use serde::Deserialize;
use tokio::sync::Mutex;

use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicUsize},
};

pub static QUEUE_MAX_SIZE: usize = 1024;
pub static QUEUE_MAX_WAIT_TIME: u64 = 500; // ms
pub static RESPONSE_CLEANING_TIME: u64 = 1000 * 60 * 30; // clean response map every 30 minutes

#[derive(Debug, Clone)]
pub struct Request {
    pub uuid: String,
    pub data: String,
}

#[derive(Debug, Clone)]
pub struct Response {
    pub data: String,
    pub produced_time: std::time::Instant,
}

pub struct StoreMemory {
    pub response_map: Arc<Mutex<HashMap<String, Response>>>,
    pub response_map_size: AtomicUsize
}
pub struct App {
    pub queue: Arc<Mutex<Vec<Request>>>,
    pub tx: tokio::sync::mpsc::Sender<Request>,
    pub store_memory: Arc<StoreMemory>
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Params {
    pub id: String,
}