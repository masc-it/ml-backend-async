use std::{sync::{Arc, Mutex, MutexGuard}, collections::HashMap};

use tokio::{sync::mpsc::Receiver, time};
use tokio_stream::{wrappers::IntervalStream, StreamExt};

use crate::backend::{QUEUE_MAX_SIZE, QUEUE_MAX_WAIT_TIME, Request, Response, RESPONSE_CLEANING_TIME};



pub fn start_process_thread(
    req_queue: Arc<Mutex<Vec<Request>>>,
    mut rx: Receiver<Request>,
    response: Arc<Mutex<HashMap<String, Response>>>
) {


    let req_queue_1 = req_queue.clone();
    let response_1 = response.clone();
    tokio::spawn(async move {
        println!("SPAWNED TEMPORIZED CONSUMER");

        let mut stream = IntervalStream::new(time::interval(std::time::Duration::from_millis(QUEUE_MAX_WAIT_TIME)));

        // Flush the queue if window time is over.
        while let Some(_ts) = stream.next().await {
            let mut _queue = req_queue_1.lock().unwrap();
            consume(_queue, response_1.clone());
        }
        
    });
    
    cleanup_response(response.clone());

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


fn consume(mut queue: MutexGuard<Vec<Request>>, response: Arc<Mutex<HashMap<String, Response>>>) {
    let queue_len = queue.len();

    if queue_len == 0 {
        return;
    }

    println!("START PROCESSING {} elements.", queue_len);
    for r in queue.iter().collect::<Vec<&Request>>().into_iter() {
        let r = r.clone();
        response.lock().unwrap().insert(r.uuid, Response { 
            data: "hi".into(),
            produced_time: std::time::Instant::now()
        });
    };

    queue.clear();
}

fn cleanup_response(response: Arc<Mutex<HashMap<String, Response>>>) {
        
    let response_2 = response.clone();

    tokio::spawn(async move {
        println!("SPAWNED CLEANER THREAD");

        let mut stream = IntervalStream::new(time::interval(std::time::Duration::from_millis(RESPONSE_CLEANING_TIME)));

        // Clean up the response map
        while let Some(_ts) = stream.next().await {

            let mut res_guard = response_2.lock().unwrap();
            
            if res_guard.len() == 0 {
                continue;
            }
            println!("cleaning...");
            let res_copy = res_guard.iter().collect::<Vec<_>>().into_iter().map(|(k,v)| {
                (k.clone(), v.clone())
            }).collect::<Vec<_>>();

            for (k, v) in &res_copy {
                if v.produced_time.elapsed().as_millis() >= RESPONSE_CLEANING_TIME as u128{
                    println!("removing: {:?}", k);
                    res_guard.remove(k);
                }
            }

        }
    });
}