use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::env::load_env;

pub static STATE: Lazy<MyState> = Lazy::new(|| {
    let env = load_env().expect("env is undefined");

    MyState {
        host: env.host,
        port: env.port,
        workers: env.workers,
        origin: env.origin,
        relay_url: env.relay_url,
        token: env.token,
        origins: Arc::new(RwLock::new(HashMap::new())),
    }
});

#[derive(Serialize, Deserialize, Clone)]
pub struct File_ {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    pub parent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub name: String,
    pub size: u64,
}

#[derive(Clone)]
pub struct MyState {
    pub host: String,
    pub port: u16,
    pub workers: usize,
    pub origin: String,
    pub relay_url: String,
    pub token: String,
    pub origins: Arc<RwLock<HashMap<String, Vec<File_>>>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Payload {
    pub origin: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<File_>>,
}
