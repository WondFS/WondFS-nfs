mod disk;
mod fake_disk;
mod disk_manager;
mod remote_disk_manager;

use axum::{
    routing::post,
    Router,
    Json,
    Extension,
};
use std::env;
use std::net::SocketAddr;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use crate::disk_manager::*;

struct Message {
    method: u8,
    address: Option<u32>,
    value: Option<Vec<u8>>,
    channel: Option<oneshot::Sender<Vec<u8>>>,
}

impl Message {
    fn new() -> Message {
        Message {
            method: 0,
            address: None,
            value: None,
            channel: None,
        }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let port = 3010;
    let is_virtual: bool;
    let disk_path: String;
    if args.len() > 1 && args[1].parse::<String>().is_ok() {
        disk_path = args[1].parse::<String>().unwrap();
        is_virtual = false;
    } else {
        disk_path = "".to_string();
        is_virtual = true;
    }
    let (tx, mut rx) = mpsc::channel(32);
    let mut message = Message::new();
    message.method = 3;
    let _ = tx.send(message).await;
    tokio::spawn(async move {
        let app = Router::new()
        .route("/read", post(read))
        .route("/write", post(write))
        .route("/erase", post(erase))
        .layer(Extension(tx));
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        println!("listening on {}", addr);
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await
            .unwrap();
    });
    let mut disk_manager = DiskManager::new(is_virtual, disk_path);
    while let Some(message) = rx.recv().await {
        match message.method {
            0 => {
                let data = disk_manager.disk_read(message.address.unwrap());
                message.channel.unwrap().send(data.to_vec()).unwrap();
            }
            1 => {
                let mut data = [0; 4096];
                data.copy_from_slice(&message.value.unwrap());
                disk_manager.disk_write(message.address.unwrap(), &data);
            }
            2 => {
                disk_manager.disk_erase(message.address.unwrap());
            }
            _ => ()
        }
    }
}

async fn read (
    Json(payload): Json<serde_json::Value>,
    Extension(state): Extension<mpsc::Sender<Message>>,
) -> Json<Value>  {
    let address = payload.as_object().unwrap().get("address").unwrap().as_str().unwrap().to_string();
    let address = address.parse::<u32>().unwrap();
    let (tx, rx) = oneshot::channel();
    let mut message = Message::new();
    message.method = 0;
    message.address = Some(address);
    message.channel = Some(tx);
    let _ = state.send(message).await;
    let value: Vec<u8> = rx.await.unwrap();
    let value = std::str::from_utf8(&value).unwrap().to_string();
    if value != "" {
        Json(json!({ "status": 1, "data": value }))
    } else {
        Json(json!({ "status": 0, "data": value }))
    }
}

async fn write (
    Json(payload): Json<serde_json::Value>,
    Extension(state): Extension<mpsc::Sender<Message>>,
) {
    let address = payload.as_object().unwrap().get("address").unwrap().as_str().unwrap().to_string();
    let address = address.parse::<u32>().unwrap();
    let value = payload.as_object().unwrap().get("data").unwrap().as_str().unwrap().to_string();
    let mut message = Message::new();
    message.method = 1;
    message.address = Some(address);
    message.value = Some(value.as_bytes().to_vec());
    let _ = state.send(message).await;
}

async fn erase (
    Json(payload): Json<serde_json::Value>,
    Extension(state): Extension<mpsc::Sender<Message>>,
) {
    let address = payload.as_object().unwrap().get("address").unwrap().as_str().unwrap().to_string();
    let address = address.parse::<u32>().unwrap();
    let mut message = Message::new();
    message.method = 2;
    message.address = Some(address);
    let _ = state.send(message).await;
}