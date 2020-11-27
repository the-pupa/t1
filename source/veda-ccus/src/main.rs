#[macro_use]
extern crate log;

use actix::prelude::*;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use chrono::Local;
use env_logger::Builder;
use ini::Ini;
use log::LevelFilter;
use std::io::Write;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};
use v_onto::individual::*;
use v_storage::storage::{StorageMode, VStorage};

mod server;
use crate::server::CMessage;

const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(5000);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/////////////////////////////////////////////

async fn ccus_route(req: HttpRequest, stream: web::Payload, srv: web::Data<Addr<server::CCUSServer>>) -> Result<HttpResponse, Error> {
    ws::start(
        WsCCUSSession {
            id: 0,
            hb: Instant::now(),
            addr: srv.get_ref().clone(),
        },
        &req,
        stream,
    )
}

struct WsCCUSSession {
    /// unique session id
    id: usize,
    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    hb: Instant,
    /// CCUS server
    addr: Addr<server::CCUSServer>,
}

impl Drop for WsCCUSSession {
    fn drop(&mut self) {
        debug!("Drop WsCCUSSession");
        self.addr.do_send(server::Disconnect {
            id: self.id,
        });
    }
}

impl Actor for WsCCUSSession {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start.
    /// We register ws session with CCUSServer
    fn started(&mut self, ctx: &mut Self::Context) {
        // we'll start heartbeat process on session start.
        self.hb(ctx);

        // register self in ccus server. `AsyncContext::wait` register
        // future within context, but context waits until this future resolves
        // before processing any other events.
        // HttpContext::state() is instance of WsCCUSSessionState, state is shared
        // across all routes within application
        let addr = ctx.address();
        self.addr
            .send(server::Connect {
                addr: addr.recipient(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => act.id = res,
                    // something is wrong with chat server
                    _ => ctx.stop(),
                }
                actix::fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        // notify ccus server
        self.addr.do_send(server::Disconnect {
            id: self.id,
        });
        Running::Stop
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        self.addr.do_send(server::Disconnect {
            id: self.id,
        });
    }
}

/// Handle messages from ccus server, we simply send it to peer websocket
impl Handler<server::Msg> for WsCCUSSession {
    type Result = ();

    fn handle(&mut self, msg: server::Msg, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

/// WebSocket message handler
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsCCUSSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        debug!("WEBSOCKET MESSAGE: {:?}", msg);
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                let m = text.trim();

                self.addr.do_send(server::ClientMessage {
                    id: self.id,
                    msg: m.to_owned(),
                });
            }
            Ok(ws::Message::Binary(_)) => println!("Unexpected binary"),
            Ok(ws::Message::Close(_)) => {
                ctx.stop();
            }
            Ok(ws::Message::Nop) => (),
            _ => {}
        }
    }

    fn finished(&mut self, ctx: &mut Self::Context) {
        ctx.stop()
    }
}

impl WsCCUSSession {
    /// helper method that sends ping to client every second.
    /// also this method checks heartbeats from client
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                info!("Websocket Client heartbeat failed, disconnecting!");

                // notify ccus server
                act.addr.do_send(server::Disconnect {
                    id: act.id,
                });

                // stop actor
                ctx.stop();

                // don't try to send a ping
                return;
            }
            ctx.ping(b"");
            ctx.text("");
        });
    }
}

fn storage_manager(tarantool_addr: String, rx: Receiver<CMessage>) {
    info!("Start STORAGE MANAGER");

    let mut storage = if !tarantool_addr.is_empty() {
        VStorage::new_tt(tarantool_addr, "veda6", "123456")
    } else {
        VStorage::new_lmdb("./data", StorageMode::ReadOnly)
    };

    loop {
        if let Ok((msg, msg_id, sender)) = rx.recv() {
            //info!("main:recv={:?}", msg);

            let mut indv = Individual::default();

            storage.get_individual(&msg, &mut indv);
            let out_counter = indv.get_first_integer("v-s:updateCounter").unwrap_or_default();

            info!("main: {:?}->{}", indv.get_id(), out_counter);

            if let Err(e) = sender.send((out_counter, msg_id)) {
                error!("NOT SEND RESPONSE, err={}", e);
            }
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let env_var = "RUST_LOG";
    match std::env::var_os(env_var) {
        Some(val) => println!("use env var: {}: {:?}", env_var, val.to_str()),
        None => std::env::set_var(env_var, "info,actix_server=info,actix_web=info"),
    }

    Builder::new()
        .format(|buf, record| writeln!(buf, "{} [{}] - {}", Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"), record.level(), record.args()))
        .filter(None, LevelFilter::Info)
        .init();

    let conf = Ini::load_from_file("veda.properties").expect("fail load [veda.properties] file");
    let section = conf.section(None::<String>).expect("fail parse veda.properties");
    let ccus_port = section.get("ccus_port").expect("param [ccus_port] not found in veda.properties").clone();

    let tarantool_addr = if let Some(p) = section.get("tarantool_url") {
        p.to_owned()
    } else {
        warn!("param [tarantool_url] not found in veda.properties");
        "".to_owned()
    };

    info!("CCUS PORT={:?}, tarantool addr={:?}", ccus_port, tarantool_addr);

    // создадим канал приема и передачи с нитью storage_manager
    let (sbscr_tx, sbscr_rx): (Sender<CMessage>, Receiver<CMessage>) = mpsc::channel();
    thread::spawn(move || storage_manager(tarantool_addr.clone(), sbscr_rx));

    // Start ccus server actor
    let server = server::CCUSServer::new(sbscr_tx.clone()).start();

    // Create Http server with websocket support
    HttpServer::new(move || {
        App::new()
            .data(server.clone())
            // websocket
            .service(web::resource("/ccus").to(ccus_route))
    })
    .bind("[::]:".to_owned() + &ccus_port)?
    //.keep_alive(75)
    .run()
    .await
}
