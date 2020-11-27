#[macro_use]
extern crate log;

use futures::io::Error;
use ini::Ini;
use nng::{Message, Protocol, Socket};
use serde_json::value::Value as JSONValue;
use std::time::*;
use std::{str, thread};
use v_api::app::{ResultCode, OptAuthorize};
use v_module::module::{init_log, Module};
use v_search::clickhouse_client::*;

#[tokio::main]
async fn main() -> Result<(), Error> {
    init_log("SEARCH_QUERY");

    let conf = Ini::load_from_file("veda.properties").expect("fail load veda.properties file");
    let section = conf.section(None::<String>).expect("fail parse veda.properties");
    let query_search_db = section.get("query_search_db").expect("param [query_search_db_url] not found in veda.properties");

    let query_url = section.get("search_query_url").expect("param [search_query_url] not found in veda.properties");

    let mut module = Module::default();

    let mut ch_client = CHClient::new(query_search_db.to_owned());

    loop {
        if ch_client.connect() {
            break;
        }
        thread::sleep(Duration::from_millis(10000));
    }

    let server = Socket::new(Protocol::Rep0).unwrap();
    if let Err(e) = server.listen(&query_url) {
        error!("fail listen {}, {:?}", query_url, e);
        return Ok(());
    }

    loop {
        if let Ok(recv_msg) = server.recv() {
            let out_msg = req_prepare(&mut module, &recv_msg, &mut ch_client);
            if let Err(e) = server.send(out_msg) {
                error!("fail send answer, err={:?}", e);
            }
        }
    }
}

const TICKET: usize = 0;
const QUERY: usize = 1;
const TOP: usize = 5;
const LIMIT: usize = 6;
const FROM: usize = 7;

fn req_prepare(module: &mut Module, request: &Message, ch_client: &mut CHClient) -> Message {
    if let Ok(s) = str::from_utf8(request.as_slice()) {
        let v: JSONValue = if let Ok(v) = serde_json::from_slice(s.as_bytes()) {
            v
        } else {
            JSONValue::Null
        };

        if let Some(a) = v.as_array() {
            let ticket_id = a.get(TICKET).unwrap().as_str().unwrap_or_default();
            let query = a.get(QUERY).unwrap().as_str().unwrap_or_default();

            let top = a.get(TOP).unwrap().as_i64().unwrap_or_default();
            let limit = a.get(LIMIT).unwrap().as_i64().unwrap_or_default();
            let from = a.get(FROM).unwrap().as_i64().unwrap_or_default();

            let mut user_uri = "cfg:Guest".to_owned();
            if !ticket_id.is_empty() {
                let ticket = module.get_ticket_from_db(&ticket_id);
                if ticket.result == ResultCode::Ok {
                    user_uri = ticket.user_uri;
                }
            }

            let res = ch_client.select(&user_uri, query, top, limit, from, OptAuthorize::YES);

            if let Ok(s) = serde_json::to_string(&res) {
                return Message::from(s.as_bytes());
            }
        }
    }
    return Message::from("[]".as_bytes());
}
