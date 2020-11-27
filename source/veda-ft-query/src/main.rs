#[macro_use]
extern crate log;

use ini::Ini;
use nng::{Message, Protocol, Socket};
use serde_json::value::Value as JSONValue;
use std::{env, str};
use v_api::app::ResultCode;
use v_ft_xapian::xapian_reader::XapianReader;
use v_api::app::OptAuthorize;
use v_module::module::{init_log, Module};
use v_search::common::FTQuery;

fn main() {
    init_log("FT_QUERY");

    let conf = Ini::load_from_file("veda.properties").expect("fail load veda.properties file");
    let section = conf.section(None::<String>).expect("fail parse veda.properties");

    let tarantool_addr = if let Some(p) = section.get("tarantool_url") {
        p.to_owned()
    } else {
        warn!("param [tarantool_url] not found in veda.properties");
        "".to_owned()
    };

    if !tarantool_addr.is_empty() {
        info!("tarantool addr={}", &tarantool_addr);
    }

    let mut query_url = section.get("ft_query_service_url").expect("param [search_query_url] not found in veda.properties").to_owned();

    let args: Vec<String> = env::args().collect();
    for el in args.iter() {
        if el.starts_with("--bind") {
            let p: Vec<&str> = el.split('=').collect();
            query_url = p[1].to_owned().trim().to_owned();
            info!("bind to={}", query_url);
        }
    }

    let mut module = Module::default();

    if let Some(mut xr) = XapianReader::new("russian", &mut module.storage) {
        let server = Socket::new(Protocol::Rep0).unwrap();
        if let Err(e) = server.listen(&query_url) {
            error!("fail listen {}, {:?}", query_url, e);
            return;
        }

        loop {
            if let Ok(recv_msg) = server.recv() {
                let out_msg = req_prepare(&mut module, &recv_msg, &mut xr);
                if let Err(e) = server.send(out_msg) {
                    error!("fail send answer, err={:?}", e);
                }
            }
        }
    } else {
        error!("fail init ft-query");
    }
}

const TICKET: usize = 0;
const QUERY: usize = 1;
const SORT: usize = 2;
const DATABASES: usize = 3;
//const REOPEN: usize = 4;
const TOP: usize = 5;
const LIMIT: usize = 6;
const FROM: usize = 7;

fn req_prepare(module: &mut Module, request: &Message, xr: &mut XapianReader) -> Message {
    if let Ok(s) = str::from_utf8(request.as_slice()) {
        let v: JSONValue = if let Ok(v) = serde_json::from_slice(s.as_bytes()) {
            v
        } else {
            JSONValue::Null
        };

        if let Some(a) = v.as_array() {
            let ticket_id = a.get(TICKET).unwrap().as_str().unwrap_or_default();
            let mut query = a.get(QUERY).unwrap().as_str().unwrap_or_default().to_string();

            if !(query.find("==").is_some() || query.find("&&").is_some() || query.find("||").is_some()) {
                query = "'*' == '".to_owned() + &query + "'";
            }

            let sort = a.get(SORT).unwrap().as_str().unwrap_or_default().to_string();
            let databases = a.get(DATABASES).unwrap().as_str().unwrap_or_default().to_string();

            let top = a.get(TOP).unwrap().as_i64().unwrap_or_default() as i32;
            let limit = a.get(LIMIT).unwrap().as_i64().unwrap_or_default() as i32;
            let from = a.get(FROM).unwrap().as_i64().unwrap_or_default() as i32;

            let mut user_uri = "cfg:Guest".to_owned();
            if !ticket_id.is_empty() {
                if ticket_id.starts_with("UU=") {
                    user_uri = ticket_id.trim_start_matches("UU=").to_owned();
                } else {
                    let ticket = module.get_ticket_from_db(&ticket_id);
                    if ticket.result == ResultCode::Ok {
                        user_uri = ticket.user_uri;
                    }
                }
            }

            let mut ctx = vec![];
            fn add_out_element(id: &str, ctx: &mut Vec<String>) {
                ctx.push(id.to_owned());
            }

            let request = FTQuery {
                ticket: "".to_string(),
                user: user_uri,
                query,
                sort,
                databases,
                reopen: false,
                top,
                limit,
                from,
            };

            if let Ok(mut res) = xr.query_use_collect_fn(&request, add_out_element, OptAuthorize::YES, &mut module.storage, &mut ctx) {
                res.result = ctx;
                debug!("res={:?}", res);
                if let Ok(s) = serde_json::to_string(&res) {
                    return Message::from(s.as_bytes());
                }
            }
        }
    }
    return Message::from("[]".as_bytes());
}
