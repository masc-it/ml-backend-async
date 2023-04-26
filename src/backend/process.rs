use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
};

use rand::Rng;
use tokio::{sync::{mpsc::Receiver, Mutex}, time};
use tokio_stream::{wrappers::IntervalStream, StreamExt};
use tonic::transport::Channel;

use crate::{
    backend::{Request, Response, QUEUE_MAX_SIZE, QUEUE_MAX_WAIT_TIME, RESPONSE_CLEANING_TIME},
    prediction::{prediction_client::PredictionClient, PredictionRequest},
};

use super::StoreMemory;

type GrpcChannels = Arc<Vec<PredictionClient<Channel>>>;


/// Start the main actors
/// 1. Producer => Collect user requests in batches
/// 2. Consumer => Run processing on such requests
/// 3. Cleaner  => Clean-up responses after some time (TTL)
pub fn start_actors(
    req_queue: Arc<Mutex<Vec<Request>>>,
    rx: Receiver<Request>,
    store_memory: Arc<StoreMemory>,
    grpc_channels: GrpcChannels,
) {
    start_producer(rx, req_queue.clone(), store_memory.clone(), grpc_channels.clone());
    start_consumer(req_queue.clone(), store_memory.clone(), grpc_channels.clone());
    start_cleaner(store_memory.clone());
}

/// Producer actor.
/// Collect user requests in a temporized and fixed-sized queue.
/// Also, act as a consumer if the queue size is excedeed after the last insertion.
fn start_producer(
    mut rx: Receiver<Request>,
    req_queue: Arc<Mutex<Vec<Request>>>,
    store_memory: Arc<StoreMemory>,
    grpc_channels: GrpcChannels,
) {
    tokio::spawn(async move {
        println!("SPAWNED PRODUCER THREAD");

        while let Some(data) = rx.recv().await {
            
            let mut _queue = req_queue.lock().await;
            _queue.push(data);
            let queue_len = _queue.len();
        
            //println!("{} elements in queue awaiting", &queue_len);

            // Flush the queue if full
            if queue_len >= QUEUE_MAX_SIZE {
                consume(req_queue.clone(), store_memory.clone(), grpc_channels.clone());
            }
            
        }
    });
}

/// Consumer actor.
/// Process batched request if timeout is reached.
fn start_consumer(
    req_queue: Arc<Mutex<Vec<Request>>>,
    store_memory: Arc<StoreMemory>,
    grpc_channels: GrpcChannels,

) {
    tokio::spawn(async move {
        println!("SPAWNED TEMPORIZED CONSUMER");

        let mut stream = IntervalStream::new(time::interval(std::time::Duration::from_millis(
            QUEUE_MAX_WAIT_TIME,
        )));

        // Flush the queue if window time is over.
        while let Some(_ts) = stream.next().await {
            //let mut _queue = req_queue.lock().unwrap();
            consume(req_queue.clone(), store_memory.clone(), grpc_channels.clone());
        }
    });
}

/// Cleaner actor.
/// Clean up stale responses after some time (TTL)
fn start_cleaner(store_memory: Arc<StoreMemory>,) {

    tokio::spawn(async move {
        println!("SPAWNED CLEANER THREAD");

        let mut stream = IntervalStream::new(time::interval(std::time::Duration::from_millis(
            RESPONSE_CLEANING_TIME,
        )));

        // Clean up the response map
        while let Some(_ts) = stream.next().await {
            let mut res_guard = store_memory.response_map.lock().await;

            if res_guard.len() == 0 {
                continue;
            }

            let num_el_before = res_guard.len();
            res_guard.retain(|_, v| {
                v.produced_time.elapsed().as_millis() < RESPONSE_CLEANING_TIME as u128
            });

            let num_el_after = res_guard.len();

            println!("Removed {} elapsed elements from store memory.", num_el_after - num_el_before);
        }
    });
}


/// Processing logic.
/// Do your custom, batched ML predictions here.
fn consume(request_queue: Arc<Mutex<Vec<Request>>>, store_memory: Arc<StoreMemory>, grpc_channels: GrpcChannels) {
    
    tokio::spawn(async move {
        
        let mut batch = request_queue.lock().await;
        let batch_size = batch.len();

        if batch_size == 0 {
            return;
        }

        // get random GRPC predictor        
        let i = rand::thread_rng().gen_range(0..grpc_channels.len());
        let mut client = grpc_channels[i].clone();

        println!("START PROCESSING {} elements.", batch_size);
        
        // Prepare payload for grpc request
        let mut inputs = vec![];
        let mut uuids = vec![];
        
        for req in batch.iter() {
            inputs.push(req.data.clone());
            uuids.push(req.uuid.clone());
        }
        batch.clear();
        drop(batch);
        
        let t0 = std::time::Instant::now();
        let real_batch_size = inputs.len();
        let pred_request = PredictionRequest { input: inputs, uuid: uuids };
        // unroll batch
        let request = tonic::Request::new(pred_request);
        // send request
        let prediction_response = client.predict(request).await;

        match prediction_response {
            Ok(prediction_response) => {

                let prediction_response = prediction_response.into_inner();
                // iterate and add responses
                //while let Some(el) = preds.next().await {
                let predictions = prediction_response.prediction.iter();
                let uuids = prediction_response.uuid.iter();
                
                let mut response_map = store_memory.response_map.lock().await;
                for (prediction, request_id) in predictions.zip(uuids) {

                    response_map.insert(
                        request_id.to_owned(),
                        Response {
                            data: prediction.to_owned(),
                            produced_time: std::time::Instant::now(),
                        },
                    );
                }

                store_memory.response_map_size.fetch_add(real_batch_size, Ordering::SeqCst);
                println!("got response in {}", t0.elapsed().as_millis());
                
            }
            Err(e) => {
                println!("err: {}", e);
            },
        }
            
    });
}
