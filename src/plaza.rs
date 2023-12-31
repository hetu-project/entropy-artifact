use std::{
    sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering::SeqCst},
    time::Duration,
};

use actix_web::{
    get, post,
    web::{Bytes, Data},
    HttpResponse,
};
use bincode::Options;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

pub struct State {
    num_participant: usize,
    num_join: AtomicUsize,
    shutdown: AtomicBool,
    shutdown_server: CancellationToken,
    repair_epoch: AtomicU32,
    num_repair_finish: AtomicU32,
}

#[post("/join")]
#[tracing::instrument(skip(data))]
pub async fn join(data: Data<State>) -> HttpResponse {
    let before = data.num_join.fetch_add(1, SeqCst);
    assert!(before < data.num_participant);
    if before + 1 == data.num_participant {
        println!("ready");
    }
    HttpResponse::Ok().finish()
}

#[post("/leave")]
#[tracing::instrument(skip(data))]
pub async fn leave(data: Data<State>) -> HttpResponse {
    let before = data.num_join.fetch_sub(1, SeqCst);
    assert!(before > 0);
    if before == 1 && data.shutdown.load(SeqCst) {
        data.shutdown_server.cancel();
    }
    HttpResponse::Ok().finish()
}

#[get("/ready")]
#[tracing::instrument(skip(data))]
pub async fn poll_ready(data: Data<State>) -> HttpResponse {
    HttpResponse::Ok().json(data.num_join.load(SeqCst) == data.num_participant)
}

#[post("/shutdown")]
#[tracing::instrument(skip(data))]
pub async fn shutdown(data: Data<State>) -> HttpResponse {
    data.shutdown.store(true, SeqCst);
    if data.num_join.load(SeqCst) == 0 {
        data.shutdown_server.cancel();
    }
    HttpResponse::Ok().finish()
}

#[post("/repair")]
#[tracing::instrument(skip(data))]
pub async fn start_repair(data: Data<State>) -> HttpResponse {
    data.repair_epoch.fetch_add(1, SeqCst);
    HttpResponse::Ok().finish()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollMessage {
    pub shutdown: bool,
    pub repair: u32,
}

#[get("/status")]
#[tracing::instrument(skip(data))]
pub async fn poll_status(data: Data<State>) -> HttpResponse {
    HttpResponse::Ok().json(PollMessage {
        shutdown: data.shutdown.load(SeqCst),
        repair: data.repair_epoch.load(SeqCst),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairFinishMessage {
    pub key: [u8; 32],
    pub duration: Duration,
}

#[post("/repair/finish")]
#[tracing::instrument(skip(data))]
pub async fn repair_finish(data: Data<State>, message: Bytes) -> HttpResponse {
    let message = bincode::options()
        .deserialize::<RepairFinishMessage>(&message)
        .unwrap();
    println!(
        ",{},repair,{}",
        crate::common::hex_string(&message.key),
        message.duration.as_secs_f32()
    );
    data.num_repair_finish.fetch_add(1, SeqCst);
    HttpResponse::Ok().finish()
}

#[get("/repair/finish")]
pub async fn poll_repair_finish(data: Data<State>) -> HttpResponse {
    HttpResponse::Ok().json(data.num_repair_finish.load(SeqCst))
}

impl State {
    pub fn new(num_participant: usize, shutdown_server: CancellationToken) -> Self {
        Self {
            num_participant,
            num_join: Default::default(),
            shutdown: Default::default(),
            shutdown_server,
            repair_epoch: Default::default(),
            num_repair_finish: Default::default(),
        }
    }
}
