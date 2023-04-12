use axum::{routing::get, Router, extract::{State, Query}};

use std::{net::SocketAddr, sync::{Arc, Mutex}, collections::HashMap};
use uuid::Uuid;

mod backend;

use backend::{Params, Request, App, start_actors};

#[tokio::main]
async fn main() {

    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let queue = Arc::new(Mutex::new(vec![]));
    let response = Arc::new(Mutex::new(HashMap::default()));

    start_actors(queue.clone(), rx, response.clone());
    
    let shared_state = Arc::new( 
        App {
            queue,
            tx,
            response
        }
    );

    let app = Router::new()
        .route("/", get(get_queue))
        .route("/get", get(get_retrieve))
        .with_state(shared_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn get_queue(State(state): State<Arc<App>>,) -> String {
    let uuid = Uuid::new_v4().to_string();

    _ = state.tx.send(Request{
        uuid: uuid.clone(),
        data: "HI".into()
    }).await;

    uuid
}

async fn get_retrieve( Query(params): Query<Params>, State(state): State<Arc<App>>,) -> String {
    
    let id = &params.id;
    let mut sink = state.response.lock().unwrap();
    let res = match sink.get(id) {
        Some(v) => v.data.clone(),
        None => "retry".into()
        };
    
    sink.remove(id);
    res
}