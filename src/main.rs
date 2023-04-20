use axum::{
    extract::{Query, State},
    routing::get,
    Router,
};
use prediction::{prediction_client::PredictionClient, PredictionRequest};

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

mod backend;

use backend::{start_actors, App, Params, Request};

pub mod prediction {
    tonic::include_proto!("prediction");
}

#[tokio::main]
async fn main() {
    
    //test_grpc().await;
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let queue = Arc::new(Mutex::new(vec![]));
    let response = Arc::new(Mutex::new(HashMap::default()));

    start_actors(queue.clone(), rx, response.clone());

    let shared_state = Arc::new(App {
        queue,
        tx,
        response,
    });

    let app = Router::new()
        .route("/", get(get_predict))
        .route("/get", get(get_retrieve))
        .with_state(shared_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}


/* async fn test_grpc() {
    let mut client = PredictionClient::connect("http://[::1]:8080").await.unwrap();

    let request = tonic::Request::new(PredictionRequest {
        input: "hellooo".into()
      });
      let response = client.predict(request).await.unwrap();
      println!("{}", response.into_inner().prediction);
} */

/// GET - Prediction endpoint
/// This will immediately return a task ID.
async fn get_predict(State(state): State<Arc<App>>) -> String {
    let uuid = Uuid::new_v4().to_string();

    _ = state
        .tx
        .send(Request {
            uuid: uuid.clone(),
            data: "INPUT DATA FROM USER".into(),
        })
        .await;

    uuid
}

/// GET - Retrieve a Task output given an ID. <br>
/// You can periodically poll this endpoint 
/// to check if your prediction is ready or not.
async fn get_retrieve(Query(params): Query<Params>, State(state): State<Arc<App>>) -> String {
    let id = &params.id;
    let mut sink = state.response.lock().unwrap();
    let res = match sink.get(id) {
        Some(v) => v.data.clone(),
        None => "retry".into(),
    };

    sink.remove(id);
    res
}
