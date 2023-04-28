use axum::{
    extract::{Query, State},
    routing::get,
    Router,
};
use prediction::prediction_client::PredictionClient;
use sync_cow::SyncCow;
use tokio::sync::Mutex;

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc},
};
use uuid::Uuid;

mod backend;

use backend::{start_actors, App, Params, Request};

use crate::backend::StoreMemory;

pub mod prediction {
    tonic::include_proto!("prediction");
}

#[tokio::main]
async fn main() {
    
    let (tx, rx) = tokio::sync::mpsc::channel(1000);
    let request_queue = Arc::new(Mutex::new(vec![]));
    let response_map = Arc::new(SyncCow::new(HashMap::default()));

    let store_memory = Arc::new(StoreMemory {
        response_map,
        response_map_size: 0.into()
    });

    let mut clients = vec![];
    for _i in 0..10 {
        let client = PredictionClient::connect("http://[::1]:8080").await;

        if let Ok(pred_client) = client {
            clients.push(pred_client);
        }
    }

    println!("We have {} grpc channels ready to use.", &clients.len());
    let clients = Arc::new(clients);

    start_actors(request_queue.clone(), rx, store_memory.clone(), clients);

    let shared_state = Arc::new(App {
        queue: request_queue,
        tx,
        store_memory
    });

    let app = Router::new()
        .route("/", get(get_predict))
        .route("/get", get(get_retrieve))
        .route("/size", get(get_size))
        .with_state(shared_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

/// GET - Prediction endpoint
/// This will immediately return a task ID.
async fn get_predict(State(state): State<Arc<App>>) -> String {
    let uuid = Uuid::new_v4().to_string();

    _ = state
        .tx
        .send(Request {
            uuid: uuid.clone(),
            data: "Mary works at Netflix in California.".into(),
        })
        .await;

    uuid
}

/// GET - Retrieve a Task output given an ID. <br>
/// You can periodically poll this endpoint 
/// to check if your prediction is ready or not.
async fn get_retrieve(Query(params): Query<Params>, State(state): State<Arc<App>>) -> String {
    let id = &params.id;
    let response_map = &*state.store_memory.response_map.read();
    let res = match response_map.get(id) {
        Some(v) => v.data.clone(),
        None => "retry".into(),
    };

    //response_map.remove(id);
    res
}

async fn get_size(State(state): State<Arc<App>>) -> String {

    let response_map_size = &state.store_memory.response_map_size.load(std::sync::atomic::Ordering::Relaxed);
    return response_map_size.to_string();
}