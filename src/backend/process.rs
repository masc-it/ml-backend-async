use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};

use tokio::{sync::mpsc::Receiver, time};
use tokio_stream::{wrappers::IntervalStream, StreamExt};

use crate::backend::{
    Request, Response, QUEUE_MAX_SIZE, QUEUE_MAX_WAIT_TIME, RESPONSE_CLEANING_TIME,
};

/// Start the main actors
/// 1. Producer => Collect user requests in batches
/// 2. Consumer => Run processing on such requests
/// 3. Cleaner  => Clean-up responses after some time (TTL)
pub fn start_actors(
    req_queue: Arc<Mutex<Vec<Request>>>,
    rx: Receiver<Request>,
    response: Arc<Mutex<HashMap<String, Response>>>,
) {
    start_producer(rx, req_queue.clone(), response.clone());
    start_consumer(req_queue.clone(), response.clone());
    start_cleaner(response.clone());
}

/// Producer actor.
/// Collect user requests in a temporized and fixed-sized queue.
/// Also, act as a consumer if the queue size is excedeed after the last insertion.
fn start_producer(
    mut rx: Receiver<Request>,
    req_queue: Arc<Mutex<Vec<Request>>>,
    response: Arc<Mutex<HashMap<String, Response>>>,
) {
    tokio::spawn(async move {
        println!("SPAWNED PRODUCER THREAD");

        while let Some(data) = rx.recv().await {
            let mut _queue = req_queue.lock().unwrap();

            _queue.push(data);

            println!("{} elements in queue awaiting", _queue.len());
            let queue_len = _queue.len();

            // Flush the queue if full
            if queue_len >= QUEUE_MAX_SIZE {
                consume(_queue, response.clone());
            }
        }
    });
}

/// Consumer actor.
/// Process batched request if timeout is reached.
fn start_consumer(
    req_queue: Arc<Mutex<Vec<Request>>>,
    response: Arc<Mutex<HashMap<String, Response>>>,
) {
    tokio::spawn(async move {
        println!("SPAWNED TEMPORIZED CONSUMER");

        let mut stream = IntervalStream::new(time::interval(std::time::Duration::from_millis(
            QUEUE_MAX_WAIT_TIME,
        )));

        // Flush the queue if window time is over.
        while let Some(_ts) = stream.next().await {
            let mut _queue = req_queue.lock().unwrap();
            consume(_queue, response.clone());
        }
    });
}

/// Cleaner actor.
/// Clean up stale responses after some time (TTL)
fn start_cleaner(response: Arc<Mutex<HashMap<String, Response>>>) {
    let response_2 = response.clone();

    tokio::spawn(async move {
        println!("SPAWNED CLEANER THREAD");

        let mut stream = IntervalStream::new(time::interval(std::time::Duration::from_millis(
            RESPONSE_CLEANING_TIME,
        )));

        // Clean up the response map
        while let Some(_ts) = stream.next().await {
            let mut res_guard = response_2.lock().unwrap();

            if res_guard.len() == 0 {
                continue;
            }
            println!("cleaning...");
            let res_copy = res_guard
                .iter()
                .collect::<Vec<_>>()
                .into_iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>();

            for (k, v) in &res_copy {
                if v.produced_time.elapsed().as_millis() >= RESPONSE_CLEANING_TIME as u128 {
                    println!("removing: {:?}", k);
                    res_guard.remove(k);
                }
            }
        }
    });
}

/// Processing logic.
/// Do your custom, batched ML predictions here.
fn consume(mut batch: MutexGuard<Vec<Request>>, response: Arc<Mutex<HashMap<String, Response>>>) {
    let batch_size = batch.len();

    if batch_size == 0 {
        return;
    }

    println!("START PROCESSING {} elements.", batch_size);
    // DO the actual heavy lifting here
    for r in batch.iter().collect::<Vec<&Request>>().into_iter() {
        let r = r.clone();
        response.lock().unwrap().insert(
            r.uuid,
            Response {
                data: "hi".into(),
                produced_time: std::time::Instant::now(),
            },
        );
    }

    batch.clear();
}
