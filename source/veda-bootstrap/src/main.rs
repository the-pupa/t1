#[macro_use]
extern crate log;

use chrono::prelude::*;
use env_logger::Builder;
use log::LevelFilter;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::process::{Child, Command};
use std::time::SystemTime;
use std::{fs, io, process, thread, time};
use sysinfo::{get_current_pid, ProcessExt, ProcessStatus, SystemExt};

#[derive(Debug)]
#[repr(u8)]
pub enum ModuleError {
    Fatal = 101,
    Recoverable = 102,
}

#[derive(Debug)]
struct Module {
    name: String,
    exec_name: String,
    args: Vec<String>,
    memory_limit: Option<u64>,
    order: u32,
    is_enabled: bool,
}

struct App {
    date_changed_modules_info: Option<SystemTime>,
    app_dir: String,
    modules_info: HashMap<String, Module>,
    modules_start_order: Vec<String>,
    started_modules: Vec<(String, Child)>,
}

impl App {
    fn start_modules(&mut self) -> io::Result<()> {
        for name in self.modules_start_order.iter() {
            //info!("start {:?}", module);
            let module = self.modules_info.get(name).unwrap();
            match start_module(&module) {
                Ok(child) => {
                    info!("{} start module {}, {}, {:?}", child.id(), module.name, module.exec_name, module.args);
                    self.started_modules.push((module.name.to_owned(), child));
                }
                Err(e) => {
                    return Err(Error::new(ErrorKind::Other, format!("fail execute {}, err={:?}", module.exec_name, e)));
                }
            }
        }

        let mut sys = sysinfo::System::new();
        thread::sleep(time::Duration::from_millis(500));
        sys.refresh_processes();

        let mut success_started = 0;
        for (name, process) in self.started_modules.iter() {
            if is_ok_process(&mut sys, process.id()).0 {
                success_started += 1;
            } else {
                error!("fail start: {} {}", process.id(), name)
            }
        }

        if success_started < self.started_modules.len() {
            for (name, process) in self.started_modules.iter_mut() {
                if let Ok(_0) = process.kill() {
                    warn!("stop process {} {}", process.id(), name);
                }
            }

            return Err(Error::new(ErrorKind::Other, "fail start"));
        }

        Ok(())
    }

    fn watch_started_modules(&mut self) {
        loop {
            let mut new_config_modules = HashSet::new();

            if let Err(e) = self.get_modules_info() {
                if e.kind() != ErrorKind::NotFound {
                    error!("fail read modules info");
                }
            }

            for el in self.modules_start_order.iter() {
                new_config_modules.insert(el.to_owned());
            }

            let mut sys = sysinfo::System::new();
            sys.refresh_processes();
            for (name, process) in self.started_modules.iter_mut() {
                let (is_ok, memory) = is_ok_process(&mut sys, process.id());
                debug!("name={}, memory={}", name, memory);
                if !is_ok {
                    let exit_code = if let Ok(c) = process.wait() {
                        c.code().unwrap_or_default()
                    } else {
                        0
                    };

                    if exit_code != ModuleError::Fatal as i32 {
                        error!("found dead module {} {}, exit code = {}, restart this", process.id(), name, exit_code);
                        if let Ok(_0) = process.kill() {
                            warn!("attempt stop module {} {}", process.id(), name);
                        }

                        if let Some(module) = self.modules_info.get(name) {
                            match start_module(module) {
                                Ok(child) => {
                                    info!("{} restart module {}, {}, {:?}", child.id(), module.name, module.exec_name, module.args);
                                    *process = child;
                                }
                                Err(e) => {
                                    error!("fail execute {}, err={:?}", module.exec_name, e);
                                }
                            }
                        } else {
                            error!("? internal error, not found module {}", name);
                        }
                    }
                }
                if let Some(module) = self.modules_info.get(name) {
                    if let Some(memory_limit) = module.memory_limit {
                        if memory > memory_limit {
                            warn!("process {}, memory={} KiB, limit={} KiB", name, memory, memory_limit);
                            if let Ok(_0) = process.kill() {
                                warn!("attempt stop module {} {}", process.id(), name);
                            }
                        }
                    }
                } else {
                    info!("process {} does not exist in the configuration, it will be killed", name);
                    if let Ok(_0) = process.kill() {
                        warn!("attempt stop module {} {}", process.id(), name);
                    }
                }
                new_config_modules.remove(name);
            }

            for name in new_config_modules {
                if let Some(module) = self.modules_info.get(&name) {
                    match start_module(&module) {
                        Ok(child) => {
                            info!("{} start module {}, {}, {:?}", child.id(), module.name, module.exec_name, module.args);
                            self.started_modules.push((module.name.to_owned(), child));
                        }
                        Err(e) => {
                            error!("fail execute {}, err={:?}", module.exec_name, e);
                        }
                    }
                }
            }

            thread::sleep(time::Duration::from_millis(10000));
        }
    }

    fn get_modules_info(&mut self) -> io::Result<()> {
        let f = File::open("veda.modules")?;
        let file = &mut BufReader::new(&f);
        let cur_modifed_date = f.metadata()?.modified()?;

        if let Some(d) = self.date_changed_modules_info {
            if d == cur_modifed_date {
                return Err(Error::new(ErrorKind::NotFound, ""));
            }
        }

        info!("read modules configuration...");
        self.modules_info.clear();
        self.date_changed_modules_info = Some(cur_modifed_date);
        let mut order = 0;

        while let Some(l) = file.lines().next() {
            if let Ok(line) = l {
                if line.starts_with('#') || line.starts_with('\t') || line.starts_with('\n') || line.starts_with(' ') || line.is_empty() {
                    continue;
                }

                let mut params = HashMap::new();

                while let Some(p) = file.lines().next() {
                    if let Ok(p) = p {
                        if p.starts_with('\t') || p.starts_with(' ') {
                            info!("param={}", p);
                            if let Some(eq_pos) = p.find('=') {
                                let nm: &str = &p[0..eq_pos].trim();
                                let vl: &str = &p[eq_pos + 1..].trim();

                                params.insert(nm.to_string(), vl.to_string());
                            }
                        } else {
                            break;
                        }
                    }
                }

                let mut module = Module {
                    name: line.to_string(),
                    args: Vec::new(),
                    memory_limit: None,
                    order,
                    is_enabled: true,
                    exec_name: String::new(),
                };
                order += 1;

                if let Some(m) = params.get("args") {
                    let elements: Vec<&str> = m.split(' ').collect();
                    for el in elements {
                        module.args.push(el.to_string());
                    }
                }

                if let Some(m) = params.get("memory-limit") {
                    let elements: Vec<&str> = m.split(' ').collect();
                    if elements.len() == 2 {
                        if let Ok(meml) = elements.get(0).unwrap_or(&"").parse::<i32>() {
                            let m = match elements.get(1).unwrap_or(&"").to_uppercase().as_str() {
                                "GB" => 1024 * 1024,
                                "MB" => 1024,
                                _ => 1,
                            };

                            module.memory_limit = Some((meml * m) as u64);
                            info!("{:?} Kb", module.memory_limit);
                        }
                    }

                    if module.memory_limit.is_none() {
                        error!("fail parse param [memory-limit]");
                    }
                }

                let module_name = if let Some(m) = params.get("module") {
                    "veda-".to_string() + m
                } else {
                    "veda-".to_string() + line.trim()
                };

                let module_path = self.app_dir.to_owned() + &module_name;
                if Path::new(&module_path).exists() {
                    module.exec_name = module_path;
                    self.modules_info.insert(line, module);
                } else {
                    return Err(Error::new(ErrorKind::Other, format!("not found module [{:?}]", &module_path)));
                }
            }
        }

        let mut vmodules: Vec<&Module> = Vec::new();
        for el in self.modules_info.values() {
            vmodules.push(el);
        }
        vmodules.sort_by(|a, b| a.order.partial_cmp(&b.order).unwrap());

        self.modules_start_order.clear();
        for el in vmodules {
            self.modules_start_order.push(el.name.to_owned());
        }

        Ok(())
    }
}

fn main() {
    let env_var = "RUST_LOG";
    match std::env::var_os(env_var) {
        Some(val) => println!("use env var: {}: {:?}", env_var, val.to_str()),
        None => std::env::set_var(env_var, "info"),
    }

    let app_dir = if let Ok(s) = std::env::var("APPDIR") {
        s.as_str().to_string() + "/"
    } else {
        "./".to_string()
    };

    Builder::new()
        .format(|buf, record| writeln!(buf, "{} [{}] - {}", Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"), record.level(), record.args()))
        .filter(None, LevelFilter::Info)
        .init();

    info!("app dir={}", app_dir);
    let mut app = App {
        date_changed_modules_info: None,
        app_dir,
        modules_info: HashMap::new(),
        modules_start_order: vec![],
        started_modules: vec![],
    };

    if let Err(e) = app.get_modules_info() {
        error!("fail read modules info, err={:?}", e);
        return;
    }

    let module_full_names: Vec<String> = app.modules_info.values().map(|x| x.exec_name[2..].to_string()).collect();

    let mut sys = sysinfo::System::new();
    sys.refresh_processes();

    let current_proc = sys.get_process(get_current_pid().unwrap()).unwrap();
    let current_user = current_proc.uid;

    for (pid, proc) in sys.get_processes() {
        if *pid == current_proc.pid() || current_user != proc.uid {
            continue;
        }

        if proc.name().starts_with("veda-") && module_full_names.contains(&proc.name().to_string()) {
            error!("unable start, found other running process: pid={}, {:?} ({:?}) ", pid, proc.exe(), proc.status());
            return;
        }
    }

    let started = app.start_modules();
    if started.is_err() {
        error!("veda not started, exit. err={:?}", started.err());
        return;
    }

    if let Ok(mut file) = File::create(".pids/__".to_owned() + "bootstrap-pid") {
        if let Err(e) = file.write_all(format!("{}", process::id()).as_bytes()) {
            error!("can not create pid file for bootstrap {}, err={:?}", process::id(), e);
        }
    }

    app.watch_started_modules();
    //info!("started {:?}", started);
}

fn is_ok_process(sys: &mut sysinfo::System, pid: u32) -> (bool, u64) {
    if let Some(proc) = sys.get_process(pid as i32) {
        match proc.status() {
            ProcessStatus::Idle => (true, proc.memory()),
            ProcessStatus::Run => (true, proc.memory()),
            ProcessStatus::Sleep => (true, proc.memory()),
            _ => (false, proc.memory()),
        }
    } else {
        (false, 0)
    }
}

fn start_module(module: &Module) -> io::Result<Child> {
    let datetime: DateTime<Local> = Local::now();

    fs::create_dir_all("./logs").unwrap_or_default();

    let log_path = "./logs/veda-".to_owned() + &module.name + "-" + &datetime.format("%Y-%m-%d %H:%M:%S.%f").to_string() + ".log";
    let std_log_file = File::create(log_path.to_string());
    let err_log_file = File::create(log_path);

    let child = if module.args.is_empty() {
        Command::new(module.exec_name.to_string()).stdout(std_log_file.unwrap()).stderr(err_log_file.unwrap()).spawn()
    } else {
        Command::new(module.exec_name.to_string()).stdout(std_log_file.unwrap()).stderr(err_log_file.unwrap()).args(&module.args).spawn()
    };

    match child {
        Ok(p) => {
            info!("success started {} with args {:?}", module.exec_name.to_string(), &module.args);
            if let Ok(mut file) = File::create(".pids/__".to_owned() + &module.name + "-pid") {
                if let Err(e) = file.write_all(format!("{}", p.id()).as_bytes()) {
                    error!("can not create pid file for {} {}, err={:?}", &module.name, p.id(), e);
                }
            }
            if module.name == "mstorage" {
                thread::sleep(time::Duration::from_millis(100));
            }
            Ok(p)
        }
        Err(e) => Err(e),
    }
}
