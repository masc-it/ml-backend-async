use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::{sync::mpsc::Receiver, time};
use tokio_stream::{wrappers::IntervalStream, StreamExt};

use crate::{
    backend::{Request, Response, QUEUE_MAX_SIZE, QUEUE_MAX_WAIT_TIME, RESPONSE_CLEANING_TIME},
    prediction::{prediction_client::PredictionClient, PredictionRequest},
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
            let queue_len = _queue.len();
        
            //println!("{} elements in queue awaiting", &queue_len);

            // Flush the queue if full
            if queue_len >= QUEUE_MAX_SIZE {
                consume(req_queue.clone(), response.clone());
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
            //let mut _queue = req_queue.lock().unwrap();
            consume(req_queue.clone(), response.clone());
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
fn consume(request_queue: Arc<Mutex<Vec<Request>>>, response: Arc<Mutex<HashMap<String, Response>>>) {
    let batch = request_queue.clone();
    let response = response.clone();
    tokio::spawn(async move {
        let batch_size;
        
        {
            let batch = batch.lock().unwrap();
            batch_size = batch.len();

            if batch_size == 0 {
                return;
            }
        }
        
        let client = PredictionClient::connect("http://[::1]:8080").await; //.expect("wtf");

        match client {
            Ok(mut client) => {
                println!("START PROCESSING {} elements.", batch_size);
                // DO the actual heavy lifting here

                let mut inputs = vec![];
                let mut uuids = vec![];
                {
                    let mut batch = request_queue.lock().unwrap();

                    if batch.len() == 0 {
                        return;
                    }
                    for req in batch.iter() {
                        inputs.push(req.data.clone());
                        uuids.push(req.uuid.clone());
                    }

                    batch.clear();
                    //drop(batch);
                }

                let t0 = std::time::Instant::now();
                let pred_request = PredictionRequest { input: inputs, uuid: uuids };
                // unroll batch
                let request = tonic::Request::new(pred_request);
                // send request
                let prediction_response = client.predict(request).await;

                match prediction_response {
                    Ok(prediction_response) => {

                        let mut preds = prediction_response.into_inner();
                        // iterate and add responses
                        while let Some(el) = preds.next().await {
                            
                            let el = el.unwrap();

                            response.lock().unwrap().insert(
                                el.uuid,
                                Response {
                                    data: el.prediction,
                                    produced_time: std::time::Instant::now(),
                                },
                            );
                        }
                        println!("got response in {}", t0.elapsed().as_millis());
                        
                    }
                    Err(e) => {
                        println!("err: {}", e);
                    },
                }
            }
            Err(e) => {
                println!("{}", e);
            },
        }

    });
}
