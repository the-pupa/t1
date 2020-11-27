#[macro_use]
extern crate log;

use std::{process, fs};
use std::collections::{HashMap, HashSet};

use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter};
use bincode::{deserialize_from, serialize_into};
use serde::{Deserialize, Serialize};

use clickhouse_rs::{Pool, errors::Error, Block, ClientHandle};
use chrono::prelude::*;
use chrono_tz::Tz;
use std::time::Instant;
use futures::executor::block_on;

use v_module::info::ModuleInfo;
use v_module::module::*;
use v_module::onto::load_onto;
use v_onto::individual::*;
use v_onto::onto::Onto;
use v_onto::resource::Value;
use v_onto::datatype::DataType;
use v_onto::datatype::Lang;
use v_queue::consumer::*;
use v_api::IndvOp;
use url::Url;

type TypedBatch = HashMap<String, Batch>;
type Batch = Vec<BatchElement>;
type BatchElement = (i64, Individual, i8);
type PredicateTable = (Vec<String>, Vec<DateTime<Tz>>, Vec<String>, Vec<i8>, Vec<u32>, Vec<i64>, HashMap<String, ColumnData>);
type PredicateTables = HashMap<String, PredicateTable>;
type CommittedTypesPropsOps = HashMap<String, HashMap<String, HashSet<i64>>>;

const BATCH_SIZE: u32 = 3_000_000;
const BLOCK_LIMIT: usize = 20_000;
const EXPORTED_TYPE: [&str; 1] = ["v-s:Exportable"];
const DB: &str = "veda_pt";
const BATCH_LOG_FILE_NAME: &str = "data/batch-log-index-pt";

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
    db_predicate_tables: HashMap<String, HashMap<String, String>>,
    typed_batch: TypedBatch,
    stats: Stats,
}

#[derive(Debug)]
enum ColumnData {
    Str(Vec<Vec<String>>),
    Date(Vec<Vec<DateTime<Tz>>>),
    Int(Vec<Vec<i64>>),
    Dec(Vec<Vec<f64>>),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct TypePropOps {
    type_name: String,
    predicate: String,
    ops: Vec<i64>,
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
            if let Some(types) = prev_state.get_literals("rdf:type") {
                for type_name in types {
                    if !self.onto.is_some_entered(&type_name, &EXPORTED_TYPE) {
                        continue;
                    }
                    let op_id = queue_element.get_first_integer("op_id").unwrap_or_default();
                    let mut prev_state = Individual::default();
                    get_inner_binobj_as_individual(queue_element, "prev_state", &mut prev_state);
                    if !self.typed_batch.contains_key(&type_name) {
                        let new_batch = Batch::new();
                        self.typed_batch.insert(type_name.clone(), new_batch);
                    }
                    let batch = self.typed_batch.get_mut(&type_name).unwrap();
                    batch.push((op_id, prev_state, -1));
                    //info!("op_id = {}, sign = {}, type = {}", op_id, -1, type_name);
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
            if let Some(types) = new_state.get_literals("rdf:type") {
                for type_name in types {
                    if !self.onto.is_some_entered(&type_name, &EXPORTED_TYPE) {
                        continue;
                    }
                    let op_id = queue_element.get_first_integer("op_id").unwrap_or_default();
                    let mut new_state = Individual::default();
                    get_inner_binobj_as_individual(queue_element, "new_state", &mut new_state);
                    if !self.typed_batch.contains_key(&type_name) {
                        let new_batch = Batch::new();
                        self.typed_batch.insert(type_name.clone(), new_batch);
                    }
                    let batch = self.typed_batch.get_mut(&type_name).unwrap();
                    batch.push((op_id, new_state, 1));
                    //info!("op_id = {}, sign = {}, type = {}", op_id, 1, type_name);
                }
            }
        }
    }

    async fn process_typed_batch(&mut self) -> Result<(), Error> {
        if !self.typed_batch.is_empty() {
            let now = Instant::now();
            let client = &mut self.pool.get_handle().await?;
            let db_predicate_tables = &mut self.db_predicate_tables;
            let stats = &mut self.stats;

            //Read or create batch log file
            let f = OpenOptions::new()
                            .read(true)
                            .write(true)
                            .create(true)
                            .truncate(false)
                            .open(BATCH_LOG_FILE_NAME)?;
            let batch_log_file = f.try_clone()?;
            let mut committed_types_props_ops: CommittedTypesPropsOps = HashMap::new();
            let mut reader = BufReader::new(f);
            loop {
                let res: Result<TypePropOps, _> = deserialize_from(&mut reader);
                match res {
                    Ok(type_prop_ops) => {
                        let type_name = type_prop_ops.type_name;
                        let prop = type_prop_ops.predicate;
                        let ops = type_prop_ops.ops;
                        if !committed_types_props_ops.contains_key(&type_name) {
                            committed_types_props_ops.insert(type_name.clone(), HashMap::new());
                        }
                        let type_hash = committed_types_props_ops.get_mut(&type_name).unwrap();
                        if !type_hash.contains_key(&prop) {
                            type_hash.insert(prop.clone(), HashSet::new());
                        }
                        let operations = type_hash.get_mut(&prop).unwrap();
                        for op in ops {
                            operations.insert(op);
                        }
                    }
                    Err(_) => break,
                }
            }
            if committed_types_props_ops.len() > 0 {
                info!("Found info file for uncompleted previous batch.");
            }

            for (type_name, batch) in self.typed_batch.iter_mut() {
                while batch.len() > BLOCK_LIMIT {
                    let mut slice = batch.drain(0..BLOCK_LIMIT).collect();
                    Context::process_batch(&type_name, &mut slice, client, db_predicate_tables, stats, &mut committed_types_props_ops, &batch_log_file).await?;
                }
                Context::process_batch(&type_name, batch, client, db_predicate_tables, stats, &mut committed_types_props_ops, &batch_log_file).await?;
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
        db_predicate_tables: &mut HashMap<String, HashMap<String, String>>,
        stats: &mut Stats,
        committed_types_props_ops: &mut CommittedTypesPropsOps,
        batch_log_file: &File,
    ) -> Result<(), Error> {

        let mut predicate_tables: PredicateTables = HashMap::new();

        let now = Instant::now();

        let rows = batch.len();

        info!("---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------");

        info!("Processing class batch: {}, count: {}", type_name, rows);

        for (op_id, individual, sign) in batch {
            Context::add_to_tables(individual, type_name, *sign, &mut predicate_tables, *op_id, committed_types_props_ops);
        }

        let elapsed = now.elapsed().as_millis();

        stats.total_prepare_duration += elapsed as usize;

        info!("Predicate tables prepared in {} ms", elapsed);

        let now = Instant::now();

        for (predicate, predicate_table) in predicate_tables {

            let (block, type_prop_ops) = Context::mk_block(type_name, predicate.clone(), predicate_table, client, db_predicate_tables).await?;

            //info!("`{}` block = {:?}", predicate, block);

            let table = format!("{}.`{}`", DB, predicate);

            client.insert(table, block).await?;

            serialize_into(&mut BufWriter::new(batch_log_file), &type_prop_ops).unwrap();

            stats.insert_count += 1;
        }

        let mut insert_duration = now.elapsed().as_millis();
        if insert_duration == 0 {
            insert_duration = 1;
        }

        let cps = (rows * 1000) as usize / (insert_duration as usize);

        info!("Predicate tables inserted successfully! Individuals count = {}, duration = {} ms, cps = {}", rows, insert_duration, cps);

        stats.total_insert_duration += insert_duration as usize;

        stats.total_rows += rows;

        let total_cps = stats.total_rows * 1000 / stats.total_insert_duration;

        let uptime = stats.started.elapsed();
        let uptime_ms = if uptime.as_millis() == 0 {
            1
        } else {
            uptime.as_millis()
        } as usize;

        let uptime_cps = (stats.total_rows * 1000 / uptime_ms) as f64;

        info!(
            "Total individuals inserted = {}, total prepare duration = {} ms, total insert duration = {} ms, avg. insert cps = {}, uptime = {}h {}m {}s, avg. uptime cps = {}, inserts count = {}",
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

    fn add_to_tables(
        individual: &mut Individual,
        type_name: &str,
        sign: i8,
        predicate_tables: &mut PredicateTables,
        op_id: i64,
        committed_types_props_ops: &mut CommittedTypesPropsOps,
    ) {

        let id = individual.get_id().to_owned();

        let version = individual.get_first_integer("v-s:updateCounter").unwrap_or(0) as u32;

        let created = Tz::UTC.timestamp(individual.get_first_datetime("v-s:created").unwrap_or(0), 0);

        let mut text_content: Vec<String> = Vec::new();

        if !committed_types_props_ops.contains_key(type_name) {
            committed_types_props_ops.insert(type_name.to_string(), HashMap::new());
        }
        let committed_props_ops = committed_types_props_ops.get_mut(type_name).unwrap();

        for predicate in individual.get_predicates() {
            if !committed_props_ops.contains_key(&predicate) {
                committed_props_ops.insert(predicate.to_string(), HashSet::new());
            }
            let ops = committed_props_ops.get_mut(&predicate).unwrap();
            let signed_op = op_id * (sign as i64);
            if !ops.contains(&signed_op) {
                Context::add_to_predicate_table(&id, version, sign, created, type_name, individual, &predicate, predicate_tables, &mut text_content, op_id);
                ops.insert(signed_op);
            } else {
                info!("Skip already exported, uri = {}, type= {}, predicate = {}, op_id = {}", id, type_name, predicate, signed_op);
            }
        }

        if !text_content.is_empty() {
            let text_predicate = String::from("text");
            let text = text_content.join(" ");
            individual.set_string(&text_predicate, &text, Lang::NONE);
            Context::add_to_predicate_table(&id, version, sign, created, type_name, individual, &text_predicate, predicate_tables, &mut text_content, op_id);
        }
    }

    fn add_to_predicate_table(
        id: &str,
        version: u32,
        sign: i8,
        created: DateTime<Tz>,
        type_name: &str,
        individual: &mut Individual,
        predicate: &str,
        predicate_tables: &mut PredicateTables,
        text_content: &mut Vec<String>,
        op_id: i64,
    ) {
        if let Some(resources) = individual.get_resources(predicate) {

            if !predicate_tables.contains_key(predicate) {
                let new_table = (vec![], vec![], vec![], vec![], vec![], vec![], HashMap::new());
                predicate_tables.insert(predicate.to_string(), new_table);
            }
            let predicate_table = predicate_tables.get_mut(predicate).unwrap();
            let (type_column, created_column, id_column, sign_column, version_column, op_type_column, columns) = predicate_table;
            type_column.push(type_name.to_owned());
            created_column.push(created);
            id_column.push(id.to_owned());
            sign_column.push(sign);
            version_column.push(version);
            op_type_column.push(op_id * (sign as i64));
            let rows = id_column.len() - 1;

            match &resources[0].rtype {
                DataType::Integer => {
                    let column_name = "int".to_string();
                    let column_value: Vec<i64> = resources.iter().map(|resource| resource.get_int()).collect();

                    if !columns.contains_key(&column_name) {
                        let new_column = ColumnData::Int(Vec::new());
                        columns.insert(column_name.clone(), new_column);
                    }

                    let column_data = columns.get_mut(&column_name).unwrap();
                    if let ColumnData::Int(column) = column_data {
                        let mut empty = vec![vec![0]; rows - column.len()];
                        column.append(&mut empty);
                        column.push(column_value);
                    }
                },
                DataType::String => {
                    let column_name = "str".to_string();
                    let column_value: Vec<String> = resources.iter().map(|resource| {
                        let str_value = resource.get_str();

                        text_content.push(str_value.trim().to_owned());

                        let lang = match resource.get_lang() {
                            Lang::NONE => String::from(""),
                            lang => format!("@{}", lang.to_string()),
                        };
                        format!("{}{}", str_value.replace("'", "\\'"), lang)
                    }).collect();

                    if !columns.contains_key(&column_name) {
                        let new_column = ColumnData::Str(Vec::new());
                        columns.insert(column_name.clone(), new_column);
                    }

                    let column_data = columns.get_mut(&column_name).unwrap();
                    if let ColumnData::Str(column) = column_data {
                        let mut empty = vec![vec!["".to_owned()]; rows - column.len()];
                        column.append(&mut empty);
                        column.push(column_value);
                    }
                },
                DataType::Uri => {
                    let column_name = "str".to_string();
                    let column_value: Vec<String> = resources.iter().map(|resource| resource.get_uri().to_string()).collect();

                    if !columns.contains_key(&column_name) {
                        let new_column = ColumnData::Str(Vec::new());
                        columns.insert(column_name.clone(), new_column);
                    }

                    let column_data = columns.get_mut(&column_name).unwrap();
                    if let ColumnData::Str(column) = column_data {
                        let mut empty = vec![vec!["".to_owned()]; rows - column.len()];
                        column.append(&mut empty);
                        column.push(column_value);
                    }
                },
                DataType::Boolean => {
                    let column_name = "int".to_string();
                    let column_value: Vec<i64> = resources.iter().map(|resource| {
                        match resource.value {
                            Value::Bool(true) => 1,
                            _ => 0
                        }
                    }).collect();

                    if !columns.contains_key(&column_name) {
                        let new_column = ColumnData::Int(Vec::new());
                        columns.insert(column_name.clone(), new_column);
                    }

                    let column_data = columns.get_mut(&column_name).unwrap();
                    if let ColumnData::Int(column) = column_data {
                        let mut empty = vec![vec![0]; rows - column.len()];
                        column.append(&mut empty);
                        column.push(column_value);
                    }
                },
                DataType::Decimal => {
                    let column_name = "dec".to_string();
                    let column_value: Vec<f64> = resources.iter().map(|resource| resource.get_float()).collect();

                    if !columns.contains_key(&column_name) {
                        let new_column = ColumnData::Dec(Vec::new());
                        columns.insert(column_name.clone(), new_column);
                    }

                    let column_data = columns.get_mut(&column_name).unwrap();
                    if let ColumnData::Dec(column) = column_data {
                        let mut empty = vec![vec![0 as f64]; rows - column.len()];
                        column.append(&mut empty);
                        column.push(column_value);
                    }
                },
                DataType::Datetime => {
                    let column_name = "date".to_string();
                    let column_value: Vec<DateTime<Tz>> = resources.iter().map(|resource| Tz::UTC.timestamp(resource.get_datetime(), 0)).collect();

                    if !columns.contains_key(&column_name) {
                        let new_column = ColumnData::Date(Vec::new());
                        columns.insert(column_name.clone(), new_column);
                    }

                    let column_data = columns.get_mut(&column_name).unwrap();
                    if let ColumnData::Date(column) = column_data {
                        let mut empty = vec![vec![Tz::UTC.timestamp(0, 0)]; rows - column.len()];
                        column.append(&mut empty);
                        column.push(column_value);
                    }
                },
                _ => {
                    error!("Value type is not supported");
                }
            }
        }
    }

    async fn mk_block(
        type_name: &str,
        predicate: String,
        predicate_table: PredicateTable,
        client: &mut ClientHandle,
        db_predicate_tables: &mut HashMap<String, HashMap<String, String>>
    ) -> Result<(Block, TypePropOps), Error> {

        let (type_column, created_column, id_column, sign_column, version_column, op_type_column, mut columns) = predicate_table;

        let rows = id_column.len();

        let predicate_ops = TypePropOps {
            type_name: type_name.to_string(),
            predicate: predicate.clone(),
            ops: op_type_column
        };

        let mut block = Block::new()
            .column("rdf_type_str", type_column)
            .column("v_s_created_date", created_column)
            .column("id", id_column)
            .column("sign", sign_column)
            .column("version", version_column);

        for (column_name, column_data) in columns.iter_mut() {
            let mut column_type= "Array(String)";
            if let ColumnData::Int(column) = column_data {
                column_type = "Array(Int64)";
                let column_size = column.len();
                let mut empty = vec![vec![0]; rows - column_size];
                column.append(&mut empty);
                //info!("column: {}, size: {}, {:?}", column_name, column.len(), column);
                block = block.column(&column_name, column.to_owned());
            }
            if let ColumnData::Str(column) = column_data {
                column_type = "Array(String)";
                let column_size = column.len();
                let mut empty = vec![vec!["".to_string()]; rows - column_size];
                column.append(&mut empty);
                //info!("column: {}, size: {}, {:?}", column_name, column.len(), column);
                block = block.column(&column_name, column.to_owned());
            }
            if let ColumnData::Dec(column) = column_data {
                column_type = "Array(Float64)";
                let column_size = column.len();
                let mut empty = vec![vec![0 as f64]; rows - column_size];
                column.append(&mut empty);
                //info!("column: {}, size: {}, {:?}", column_name, column.len(), column);
                block = block.column(&column_name, column.to_owned());
            }
            if let ColumnData::Date(column) = column_data {
                column_type = "Array(DateTime)";
                let column_size = column.len();
                let mut empty = vec![vec![Tz::UTC.timestamp(0, 0)]; rows - column_size];
                column.append(&mut empty);
                //info!("column: {}, size: {}, {:?}", column_name, column.len(), column);
                block = block.column(&column_name, column.to_owned());
            }
            create_predicate_value_column(&predicate, column_name, column_type, client, db_predicate_tables).await?;
        }
        Ok((block, predicate_ops))
    }

}

fn main() ->  Result<(), Error> {
    init_log("SEARCH_INDEX_PT");

    if get_info_of_module("input-onto").unwrap_or((0, 0)).0 == 0 {
        wait_module("fulltext_indexer", wait_load_ontology());
    }

    let consumer_name = "search_index_pt".to_string();

    let mut queue_consumer = Consumer::new("./data/queue", &consumer_name, "individuals-flow").expect("!!!!!!!!! FAIL QUEUE");
    let module_info = ModuleInfo::new("./data", &consumer_name, true);
    if module_info.is_err() {
        error!("{:?}", module_info.err());
        process::exit(101);
    }
    let mut module = Module::default();

    let mut pool = match connect_to_clickhouse(&Module::get_property("query_indexer_db").unwrap_or_default()) {
        Err(e) => {
            error!("Failed to connect to clickhouse: {}", e);
            process::exit(101)
        },
        Ok(pool) => pool,
    };

    block_on(init_clickhouse(&mut pool))?;

    let db_predicate_tables = block_on(read_predicate_tables(&mut pool))?;

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
        db_predicate_tables,
        typed_batch,
        stats
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
        &mut (heartbeat as fn(&mut Module, &mut ModuleInfo, &mut Context))
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

async fn create_predicate_table(predicate_name: &str, client: &mut ClientHandle, db_predicate_tables: &mut HashMap<String, HashMap<String, String>>) -> Result<(), Error> {
    if db_predicate_tables.get(predicate_name).is_some() {
        return Ok(());
    }
    let query = format!(r"
        CREATE TABLE IF NOT EXISTS {}.`{}` (
            id String,
            sign Int8 DEFAULT 1,
            version UInt32,
            `rdf_type_str` String,
            `v_s_created_date` DateTime DEFAULT toDateTime(0)
        )
        ENGINE = VersionedCollapsingMergeTree(sign, version)
        ORDER BY (`rdf_type_str`, `v_s_created_date`, id)
        PARTITION BY (`rdf_type_str`)
    ", DB, predicate_name);
    client.execute(query).await?;
    let mut table_columns: HashMap<String, String> = HashMap::new();
    table_columns.insert("id".to_owned(), "String".to_owned());
    table_columns.insert("sign".to_owned(), "Int8".to_owned());
    table_columns.insert("version".to_owned(), "UInt32".to_owned());
    table_columns.insert("rdf_type_str".to_owned(), "Array(String)".to_owned());
    table_columns.insert("v_s_created_date".to_owned(), "Array(DateTime)".to_owned());
    db_predicate_tables.insert(predicate_name.to_string(), table_columns);
    Ok(())
}

async fn create_predicate_value_column(predicate_name: &str, column_name: &str, column_type: &str, client: &mut ClientHandle, db_predicate_tables: &mut HashMap<String, HashMap<String, String>>) -> Result<(), Error> {
    if None == db_predicate_tables.get_mut(predicate_name) {
        create_predicate_table(predicate_name, client, db_predicate_tables).await?;
    }
    if let Some(table_columns) = db_predicate_tables.get_mut(predicate_name) {
        if table_columns.get(column_name).is_some() {
            return Ok(());
        } else {
            let query = format!("ALTER TABLE {}.`{}` ADD COLUMN IF NOT EXISTS `{}` {}", DB, predicate_name, column_name, column_type);
            client.execute(query).await?;
            table_columns.insert(column_name.to_owned(), column_type.to_owned());
        }
    }
    Ok(())
}

async fn read_predicate_tables(pool: &mut Pool) -> Result<HashMap<String, HashMap<String, String>>, Error> {
    let read_tables_query = format!("SELECT name from system.tables where database = '{}'", DB);
    let mut tables: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut client = pool.get_handle().await?;
    let tables_block = client.query(read_tables_query).fetch_all().await?;
    for row_table in tables_block.rows() {
        let table_name: String = row_table.get("name")?;
        let read_columns = format!("DESCRIBE {}.`{}`", DB, table_name);
        let mut table_columns: HashMap<String, String> = HashMap::new();
        let columns_block = client.query(read_columns).fetch_all().await?;
        for row_column in columns_block.rows() {
            let column_name: String      = row_column.get("name")?;
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
            return Ok(pool);
        }
        Err(e) => {
            error!("{:?}", e);
            return Err("Invalid connection url");
        }
    }
}

async fn init_clickhouse(pool: &mut Pool) -> Result<(), Error> {
    let init_veda_db = format!("CREATE DATABASE IF NOT EXISTS {}", DB);
    let mut client = pool.get_handle().await?;
    client.execute(init_veda_db).await?;
    Ok(())
}
