#[macro_use]
extern crate log;

#[macro_use]
extern crate lazy_static;

use bincode::{deserialize_from, serialize_into};
use chrono::prelude::*;
use chrono_tz::Tz;
use clickhouse_rs::{errors::Error, Block, ClientHandle, Pool};
use futures::executor::block_on;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter};
use std::time::Instant;
use std::{fs, process};
use url::Url;
use v_module::info::ModuleInfo;
use v_module::module::*;
use v_module::onto::load_onto;
use v_onto::datatype::DataType;
use v_onto::datatype::Lang;
use v_onto::individual::*;
use v_onto::onto::Onto;
use v_onto::resource::Value;
use v_queue::consumer::*;
use v_api::IndvOp;

type TypedBatch = HashMap<String, Batch>;
type Batch = Vec<BatchElement>;
type BatchElement = (i64, Individual, i8);
type CommittedTypesOps = HashMap<String, HashSet<i64>>;

const BATCH_SIZE: u32 = 3_000_000;
const BLOCK_LIMIT: usize = 20_000;
const BATCH_LOG_FILE_NAME: &str = "data/batch-log-index-tt";

pub struct Stats {
    total_prepare_duration: usize,
    total_insert_duration: usize,
    total_rows: usize,
    insert_count: usize,
    started: Instant,
    last: Instant,
}

pub struct Context {
    onto: Onto,
    pool: Pool,
    db_type_tables: HashMap<String, HashMap<String, String>>,
    typed_batch: TypedBatch,
    stats: Stats,
}

enum ColumnData {
    Str(Vec<Vec<String>>),
    Date(Vec<Vec<DateTime<Tz>>>),
    Int(Vec<Vec<i64>>),
    Dec(Vec<Vec<f64>>),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct TypeOps {
    type_name: String,
    ops: Vec<i64>
}

impl Context {
    fn add_to_typed_batch(&mut self, queue_element: &mut Individual) {
        let cmd = get_cmd(queue_element);

        if cmd.is_none() {
            error!("Queue element cmd is none. Skip element.");
            return;
        }

        let mut new_state = Individual::default();
        get_inner_binobj_as_individual(queue_element, "new_state", &mut new_state);

        let id = new_state.get_id().to_string();
        let actual_version = new_state.get_first_literal("v-s:actualVersion").unwrap_or_default();
        if !actual_version.is_empty() && actual_version != id {
            info!("Skip not actual version. {}.v-s:actualVersion {} != {}", id, &actual_version, id);
            return;
        }

        let mut prev_state = Individual::default();
        let is_new = !get_inner_binobj_as_individual(queue_element, "prev_state", &mut prev_state);
        if !is_new {
            if let Some(type_resources) = prev_state.get_resources("rdf:type") {
                for type_resource in type_resources {
                    if let Value::Uri(type_name) = type_resource.value {
                        let op_id = queue_element.get_first_integer("op_id").unwrap_or_default();
                        let mut prev_state = Individual::default();
                        get_inner_binobj_as_individual(queue_element, "prev_state", &mut prev_state);
                        if !self.typed_batch.contains_key(&type_name) {
                            let new_batch = Batch::new();
                            self.typed_batch.insert(type_name.clone(), new_batch);
                        }
                        let batch = self.typed_batch.get_mut(&type_name).unwrap();
                        batch.push((op_id, prev_state, -1));
                    }
                }
            }
        }

        let is_remove: bool;
        if cmd == Some(IndvOp::Remove) {
            is_remove = true;
        } else {
            is_remove = false;
        }
        if !is_remove {
            if let Some(type_resources) = new_state.get_resources("rdf:type") {
                for type_resource in type_resources {
                    if let Value::Uri(type_name) = type_resource.value {
                        let op_id = queue_element.get_first_integer("op_id").unwrap_or_default();
                        let mut new_state = Individual::default();
                        get_inner_binobj_as_individual(queue_element, "new_state", &mut new_state);
                        if !self.typed_batch.contains_key(&type_name) {
                            let new_batch = Batch::new();
                            self.typed_batch.insert(type_name.clone(), new_batch);
                        }
                        let batch = self.typed_batch.get_mut(&type_name).unwrap();
                        batch.push((op_id, new_state, 1));
                    }
                }
            }
        }
    }

    async fn process_typed_batch(&mut self) -> Result<(), Error> {
        if !self.typed_batch.is_empty() {
            let now = Instant::now();
            let client = &mut self.pool.get_handle().await?;
            let db_type_tables = &mut self.db_type_tables;
            let stats = &mut self.stats;

            let f = OpenOptions::new()
                            .read(true)
                            .write(true)
                            .create(true)
                            .truncate(false)
                            .open(BATCH_LOG_FILE_NAME)?;
            let batch_log_file= f.try_clone()?;
            let mut reader = BufReader::new(f);
            let mut committed_types_ops: CommittedTypesOps = HashMap::new();
            loop {
                let res: Result<TypeOps, _> = deserialize_from(&mut reader);
                match res {
                    Ok(type_ops) => {
                        let type_name = type_ops.type_name;
                        let ops = type_ops.ops;
                        if !committed_types_ops.contains_key(&type_name) {
                            committed_types_ops.insert(type_name.clone(), HashSet::new());
                        }
                        let committed_ops = committed_types_ops.get_mut(&type_name).unwrap();
                        for op in ops {
                            committed_ops.insert(op);
                        }
                    }
                    Err(_) => break,
                }
            }
            if committed_types_ops.len() > 0 {
                info!("Found {:#?} committed individuals", committed_types_ops.len());
            }

            for (type_name, batch) in self.typed_batch.iter_mut() {
                while batch.len() > BLOCK_LIMIT {
                    let mut slice = batch.drain(0..BLOCK_LIMIT).collect();
                    Context::process_batch(&type_name, &mut slice, client, db_type_tables, stats, &mut committed_types_ops, &batch_log_file).await?;
                }
                Context::process_batch(&type_name, batch, client, db_type_tables, stats, &mut committed_types_ops, &batch_log_file).await?;
            }
            fs::remove_file(BATCH_LOG_FILE_NAME)?;
            self.typed_batch.clear();
            stats.last = Instant::now();
            info!("Batch processed in {} ms", now.elapsed().as_millis());
        }
        Ok(())
    }

    async fn process_batch(
        type_name: &str,
        batch: &mut Batch,
        client: &mut ClientHandle,
        db_type_tables: &mut HashMap<String, HashMap<String, String>>,
        stats: &mut Stats,
        committed_types_ops: &mut CommittedTypesOps,
        batch_log_file: &File,
    ) -> Result<(), Error> {
        let now = Instant::now();

        info!("---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------");

        info!("Processing class batch: {}, count: {}", type_name, batch.len().clone());

        let mut id_column: Vec<String> = Vec::new();

        let mut sign_column: Vec<i8> = Vec::new();

        let mut version_column: Vec<u32> = Vec::new();

        let mut text_column: Vec<String> = Vec::new();

        let mut columns: HashMap<String, ColumnData> = HashMap::new();

        let mut type_ops = TypeOps {
            type_name: type_name.to_string(),
            ops: Vec::new(),
        };

        if !committed_types_ops.contains_key(type_name) {
            committed_types_ops.insert(type_name.to_string(), HashSet::new());
        }
        let committed_ops = committed_types_ops.get_mut(type_name).unwrap();

        for (op_id, individual, sign) in batch {
            let signed_op = (*op_id) * (*sign as i64);
            if !committed_ops.contains(&signed_op) {
                Context::add_to_table(individual, *sign, &mut id_column, &mut sign_column, &mut version_column, &mut text_column, &mut columns);
                type_ops.ops.push(signed_op);
                committed_ops.insert(signed_op);
            } else {
                info!("Skip already exported individual, uri = {}, type = {}, op_id = {}", individual.get_id(), type_name, signed_op);
            }
        }

        info!("Batch prepared in {} us", now.elapsed().as_micros());

        stats.total_prepare_duration += now.elapsed().as_millis() as usize;

        let now = Instant::now();

        let rows = id_column.len();

        let block = Context::mk_block(type_name, id_column, sign_column, version_column, text_column, &mut columns, client, db_type_tables).await?;

        if block.row_count() == 0 {
            info!("Block is empty! Nothing to insert");
            return Ok(());
        }

        let table = format!("veda_tt.`{}`", type_name);

        client.insert(table, block).await?;

        stats.insert_count += 1;

        serialize_into(&mut BufWriter::new(batch_log_file), &type_ops).unwrap();

        let mut insert_duration = now.elapsed().as_millis() as usize;
        if insert_duration == 0 {
            insert_duration = 1;
        }

        let cps = (rows * 1000 / insert_duration) as f64;

        info!("Block inserted successfully! Rows = {}, columns = {}, duration = {} ms, cps = {}", rows, columns.keys().len() + 2, insert_duration, cps);

        stats.total_insert_duration += insert_duration;

        stats.total_rows += rows;

        let total_cps = (stats.total_rows * 1000 / stats.total_insert_duration) as f64;

        let uptime = stats.started.elapsed();
        let uptime_ms = if uptime.as_millis() == 0 {
            1
        } else {
            uptime.as_millis()
        } as usize;

        let uptime_cps = (stats.total_rows * 1000 / uptime_ms) as f64;

        info!(
            "Total rows inserted = {}, total prepare duration = {} ms, total insert duration = {} ms, avg. insert cps = {}, uptime = {}h {}m {}s, avg. uptime cps = {}, inserts count = {}",
            stats.total_rows,
            stats.total_prepare_duration,
            stats.total_insert_duration,
            total_cps,
            (uptime_ms / 1000) / 3600,
            (uptime_ms / 1000) % 3600 / 60,
            (uptime_ms / 1000) % 3600 % 60,
            uptime_cps,
            stats.insert_count
        );

        Ok(())
    }

    fn add_to_table(
        individual: &mut Individual,
        sign: i8,
        id_column: &mut Vec<String>,
        sign_column: &mut Vec<i8>,
        version_column: &mut Vec<u32>,
        text_column: &mut Vec<String>,
        columns: &mut HashMap<String, ColumnData>,
    ) {
        let rows = id_column.len();

        let id = individual.get_id().to_owned();

        let version = individual.get_first_integer("v-s:updateCounter").unwrap_or(0) as u32;

        id_column.push(id);

        version_column.push(version);

        sign_column.push(sign);

        let mut text_content: Vec<String> = Vec::new();

        for predicate in individual.get_predicates() {
            if let Some(resources) = individual.get_resources(&predicate) {
                lazy_static! {
                    static ref RE: Regex = Regex::new("[^a-zA-Z0-9]").unwrap();
                }
                let mut column_name = RE.replace_all(&predicate, "_").into_owned();
                match &resources[0].rtype {
                    DataType::Integer => {
                        column_name.push_str("_int");
                        let column_value: Vec<i64> = resources.iter().map(|resource| resource.get_int()).collect();

                        if !columns.contains_key(&column_name) {
                            let new_column = ColumnData::Int(Vec::new());
                            columns.insert(column_name.clone(), new_column);
                        }

                        let column_data = columns.get_mut(&column_name).unwrap();
                        if let ColumnData::Int(column) = column_data {
                            let column_size = column.len();
                            let mut empty = vec![vec![0]; rows - column_size];
                            column.append(&mut empty);
                            column.push(column_value);
                        }
                    }
                    DataType::String => {
                        column_name.push_str("_str");
                        let column_value: Vec<String> = resources
                            .iter()
                            .map(|resource| {
                                let str_value = resource.get_str();

                                text_content.push(str_value.trim().to_owned());

                                let lang = match resource.get_lang() {
                                    Lang::NONE => String::from(""),
                                    lang => format!("@{}", lang.to_string()),
                                };
                                format!("{}{}", str_value.replace("'", "\\'"), lang)
                            })
                            .collect();

                        if !columns.contains_key(&column_name) {
                            let new_column = ColumnData::Str(Vec::new());
                            columns.insert(column_name.clone(), new_column);
                        }

                        let column_data = columns.get_mut(&column_name).unwrap();
                        if let ColumnData::Str(column) = column_data {
                            let column_size = column.len();
                            let mut empty = vec![vec!["".to_owned()]; rows - column_size];
                            column.append(&mut empty);
                            column.push(column_value);
                        }
                    }
                    DataType::Uri => {
                        column_name.push_str("_str");
                        let column_value: Vec<String> = resources.iter().map(|resource| resource.get_uri().to_string()).collect();

                        if !columns.contains_key(&column_name) {
                            let new_column = ColumnData::Str(Vec::new());
                            columns.insert(column_name.clone(), new_column);
                        }

                        let column_data = columns.get_mut(&column_name).unwrap();
                        if let ColumnData::Str(column) = column_data {
                            let column_size = column.len();
                            let mut empty = vec![vec!["".to_owned()]; rows - column_size];
                            column.append(&mut empty);
                            column.push(column_value);
                        }
                    }
                    DataType::Boolean => {
                        column_name.push_str("_int");
                        let column_value: Vec<i64> = resources
                            .iter()
                            .map(|resource| match resource.value {
                                Value::Bool(true) => 1,
                                _ => 0,
                            })
                            .collect();

                        if !columns.contains_key(&column_name) {
                            let new_column = ColumnData::Int(Vec::new());
                            columns.insert(column_name.clone(), new_column);
                        }

                        let column_data = columns.get_mut(&column_name).unwrap();
                        if let ColumnData::Int(column) = column_data {
                            let column_size = column.len();
                            let mut empty = vec![vec![0]; rows - column_size];
                            column.append(&mut empty);
                            column.push(column_value);
                        }
                    }
                    DataType::Decimal => {
                        column_name.push_str("_dec");
                        let column_value: Vec<f64> = resources.iter().map(|resource| resource.get_float()).collect();

                        if !columns.contains_key(&column_name) {
                            let new_column = ColumnData::Dec(Vec::new());
                            columns.insert(column_name.clone(), new_column);
                        }

                        let column_data = columns.get_mut(&column_name).unwrap();
                        if let ColumnData::Dec(column) = column_data {
                            let column_size = column.len();
                            let mut empty = vec![vec![0 as f64]; rows - column_size];
                            column.append(&mut empty);
                            column.push(column_value);
                        }
                    }
                    DataType::Datetime => {
                        column_name.push_str("_date");
                        let column_value: Vec<DateTime<Tz>> = resources.iter().map(|resource| Tz::UTC.timestamp(resource.get_datetime(), 0)).collect();

                        if !columns.contains_key(&column_name) {
                            let new_column = ColumnData::Date(Vec::new());
                            columns.insert(column_name.clone(), new_column);
                        }

                        let column_data = columns.get_mut(&column_name).unwrap();
                        if let ColumnData::Date(column) = column_data {
                            let column_size = column.len();
                            let mut empty = vec![vec![Tz::UTC.timestamp(0, 0)]; rows - column_size];
                            column.append(&mut empty);
                            column.push(column_value);
                        }
                    }
                    _ => {
                        error!("Value type is not supported");
                    }
                }
            }
        }

        text_column.push(text_content.join(" "));
    }

    async fn mk_block(
        type_name: &str,
        id_column: Vec<String>,
        sign_column: Vec<i8>,
        version_column: Vec<u32>,
        text_column: Vec<String>,
        columns: &mut HashMap<String, ColumnData>,
        client: &mut ClientHandle,
        db_type_tables: &mut HashMap<String, HashMap<String, String>>,
    ) -> Result<Block, Error> {
        let rows = id_column.len();

        let mut block = Block::new()
            .column("id", id_column)
            .column("sign", sign_column)
            .column("version", version_column)
            .column("text", text_column);

        for (column_name, column_data) in columns.iter_mut() {
            let mut column_type = "Array(String)";
            if let ColumnData::Int(column) = column_data {
                column_type = "Array(Int64)";
                let mut empty = vec![vec![0]; rows - column.len()];
                column.append(&mut empty);
                //info!("column: {}, size: {}, {:?}", column_name, column.len(), column);
                block = block.column(&column_name, column.to_owned());
            }
            if let ColumnData::Str(column) = column_data {
                column_type = "Array(String)";
                let mut empty = vec![vec!["".to_string()]; rows - column.len()];
                column.append(&mut empty);
                //info!("column: {}, size: {}, {:?}", column_name, column.len(), column);
                block = block.column(&column_name, column.to_owned());
            }
            if let ColumnData::Dec(column) = column_data {
                column_type = "Array(Float64)";
                let mut empty = vec![vec![0 as f64]; rows - column.len()];
                column.append(&mut empty);
                //info!("column: {}, size: {}, {:?}", column_name, column.len(), column);
                block = block.column(&column_name, column.to_owned());
            }
            if let ColumnData::Date(column) = column_data {
                column_type = "Array(DateTime)";
                let mut empty = vec![vec![Tz::UTC.timestamp(0, 0)]; rows - column.len()];
                column.append(&mut empty);
                //info!("column: {}, size: {}, {:?}", column_name, column.len(), column);
                block = block.column(&column_name, column.to_owned());
            }
            create_type_predicate_column(type_name, &column_name, &column_type, client, db_type_tables).await?;
        }
        Ok(block)
    }
}

fn main() -> Result<(), Error> {
    init_log("SEARCH_INDEX_TT");

    //return test().await;

    if get_info_of_module("input-onto").unwrap_or((0, 0)).0 == 0 {
        wait_module("fulltext_indexer", wait_load_ontology());
    }

    let mut queue_consumer = Consumer::new("./data/queue", "search_index_tt", "individuals-flow").expect("!!!!!!!!! FAIL QUEUE");
    let module_info = ModuleInfo::new("./data", "search_index_tt", true);
    if module_info.is_err() {
        error!("{:?}", module_info.err());
        process::exit(101);
    }
    let mut module = Module::default();

    let mut pool = match connect_to_clickhouse(&Module::get_property("query_indexer_db").unwrap_or_default()) {
        Err(e) => {
            error!("Failed to connect to clickhouse: {}", e);
            process::exit(101)
        }
        Ok(pool) => pool,
    };

    block_on(init_clickhouse(&mut pool))?;

    let db_type_tables = block_on(read_type_tables(&mut pool))?;

    info!("Tables: {:?}", db_type_tables);

    let typed_batch: TypedBatch = HashMap::new();
    let stats = Stats {
        total_prepare_duration: 0,
        total_insert_duration: 0,
        total_rows: 0,
        insert_count: 0,
        started: Instant::now(),
        last: Instant::now(),
    };

    let mut ctx = Context {
        onto: Onto::default(),
        pool,
        db_type_tables,
        typed_batch,
        stats,
    };

    load_onto(&mut module.storage, &mut ctx.onto);

    info!("Rusty search-index: start listening to queue");

    module.listen_queue(
        &mut queue_consumer,
        &mut module_info.unwrap(),
        &mut ctx,
        &mut (before as fn(&mut Module, &mut Context, u32) -> Option<u32>),
        &mut (process as fn(&mut Module, &mut ModuleInfo, &mut Context, &mut Individual, my_consumer: &Consumer) -> Result<bool, PrepareError>),
        &mut (after as fn(&mut Module, &mut ModuleInfo, &mut Context, u32) -> bool),
        &mut (heartbeat as fn(&mut Module, &mut ModuleInfo, &mut Context)),
    );
    Ok(())
}

fn heartbeat(_module: &mut Module, _module_info: &mut ModuleInfo, _ctx: &mut Context) {}

fn before(_module: &mut Module, _ctx: &mut Context, _batch_size: u32) -> Option<u32> {
    Some(BATCH_SIZE)
}

fn after(_module: &mut Module, _module_info: &mut ModuleInfo, ctx: &mut Context, _processed_batch_size: u32) -> bool {
    if let Err(e) = block_on(ctx.process_typed_batch()) {
        error!("Error processing batch: {}", e);
        process::exit(101);
    }
    true
}

fn process(_module: &mut Module, module_info: &mut ModuleInfo, ctx: &mut Context, queue_element: &mut Individual, _my_consumer: &Consumer) -> Result<bool, PrepareError> {
    let op_id = queue_element.get_first_integer("op_id").unwrap_or_default();
    if let Err(e) = module_info.put_info(op_id, op_id) {
        error!("Failed to write module_info, op_id={}, err={:?}", op_id, e);
    }
    ctx.add_to_typed_batch(queue_element);
    Ok(false)
}

async fn create_type_predicate_column(
    type_name: &str,
    column_name: &str,
    column_type: &str,
    client: &mut ClientHandle,
    db_type_tables: &mut HashMap<String, HashMap<String, String>>,
) -> Result<(), Error> {
    if None == db_type_tables.get_mut(type_name) {
        create_type_table(type_name, client, db_type_tables).await?;
    }
    if let Some(table_columns) = db_type_tables.get_mut(type_name) {
        if table_columns.get(column_name).is_some() {
            return Ok(());
        } else {
            let query = format!("ALTER TABLE veda_tt.`{}` ADD COLUMN IF NOT EXISTS `{}` {}", type_name, column_name, column_type);
            client.execute(query).await?;
            table_columns.insert(column_name.to_string(), column_type.to_string());
        }
    }
    Ok(())
}

async fn create_type_table(type_name: &str, client: &mut ClientHandle, db_type_tables: &mut HashMap<String, HashMap<String, String>>) -> Result<(), Error> {
    if db_type_tables.get(type_name).is_some() {
        return Ok(());
    }
    let query = format!(
        r"
        CREATE TABLE IF NOT EXISTS veda_tt.`{}` (
            id String,
            sign Int8 DEFAULT 1,
            version UInt32,
            text String,
            `v_s_created_date` Array(DateTime) DEFAULT [toDateTime(0)],
            `v_s_deleted_int` Array(Int64) DEFAULT [0]
        )
        ENGINE = VersionedCollapsingMergeTree(sign, version)
        ORDER BY (`v_s_created_date`[1], id)
        PARTITION BY (toYear(`v_s_created_date`[1]))
    ",
        type_name
    );
    client.execute(query).await?;
    let mut table_columns: HashMap<String, String> = HashMap::new();
    table_columns.insert("id".to_owned(), "String".to_owned());
    table_columns.insert("sign".to_owned(), "Int8".to_owned());
    table_columns.insert("version".to_owned(), "UInt32".to_owned());
    table_columns.insert("text".to_owned(), "String".to_owned());
    table_columns.insert("v_s_created_date".to_owned(), "Array(DateTime)".to_owned());
    table_columns.insert("v_s_deleted_int".to_owned(), "Array(Int64)".to_owned());
    db_type_tables.insert(type_name.to_string(), table_columns);
    Ok(())
}

async fn read_type_tables(pool: &mut Pool) -> Result<HashMap<String, HashMap<String, String>>, Error> {
    let read_tables_query = "SELECT name from system.tables where database = 'veda_tt'";
    let mut tables: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut client = pool.get_handle().await?;
    let tables_block = client.query(read_tables_query).fetch_all().await?;
    for row_table in tables_block.rows() {
        let table_name: String = row_table.get("name")?;
        let read_columns = format!("DESCRIBE veda_tt.`{}`", table_name);
        let mut table_columns: HashMap<String, String> = HashMap::new();
        let columns_block = client.query(read_columns).fetch_all().await?;
        for row_column in columns_block.rows() {
            let column_name: String = row_column.get("name")?;
            let data_type: String = row_column.get("type")?;
            table_columns.insert(column_name, data_type);
        }
        tables.insert(table_name, table_columns);
    }
    Ok(tables)
}

fn connect_to_clickhouse(query_db_url: &str) -> Result<Pool, &'static str> {
    info!("Configuration to connect to Clickhouse: {}", query_db_url);
    match Url::parse(query_db_url) {
        Ok(url) => {
            let host = url.host_str().unwrap_or("127.0.0.1");
            let port = url.port().unwrap_or(9000);
            let user = url.username();
            let pass = url.password().unwrap_or("123");
            let url = format!("tcp://{}:{}@{}:{}/", user, pass, host, port);
            info!("Trying to connect to Clickhouse, host: {}, port: {}, user: {}, password: {}", host, port, user, pass);
            info!("Connection url: {}", url);
            let pool = Pool::new(url);
            Ok(pool)
        }
        Err(e) => {
            error!("{:?}", e);
            Err("Invalid connection url")
        }
    }
}

async fn init_clickhouse(pool: &mut Pool) -> Result<(), Error> {
    let init_veda_db = "CREATE DATABASE IF NOT EXISTS veda_tt";
    let mut client = pool.get_handle().await?;
    client.execute(init_veda_db).await?;
    Ok(())
}
