//! `sfs`: serves the files of the current directory over HTTP and, with a relay, lists the files of the other servers as well.
//!
//! - `config`: the settings (environment, `.env`)
//! - `confine`: keeps what is served inside the current directory
//! - `auth`: the token guard for the calls from the relay
//! - `model`: what is sent between the servers and the relay, and the check of what comes in
//! - `registry`: the files of the other servers
//! - `api`: the calls from the relay (`PUT /`, `DELETE /`)
//! - `listing`: the page of a directory
//! - `scan`, `sync`, `relay`: telling the relay which files this server has

mod api;
mod auth;
mod config;
mod confine;
mod listing;
mod model;
mod registry;
mod relay;
mod scan;
mod sync;

use auth::TokenGuard;
use config::Config;
use confine::InsideRoot;
use ntex::web::{self, middleware::DefaultHeaders, types::JsonConfig, HttpServer};
use ntex_files as fs;
use registry::Registry;
use relay::Relay;
use std::{error::Error, path::Path, sync::Arc};

/// The most a registration may carry. It also limits what the relay can make us keep in memory.
const MAX_BODY: usize = 1024 * 500;

#[ntex::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let config = Config::load().map_err(|e| format!("settings: {e}"))?;
    if config.weak_token() {
        log::warn!(
            "TOKEN is shorter than {} characters: use a long random one",
            config::MIN_TOKEN_LEN
        );
    }
    if config.relay_is_plain_http() {
        log::warn!("RELAY_URL is plain http: the token can be read on the way. Use https");
    }

    let root = InsideRoot::new(Path::new("."))?;
    let registry = Arc::new(Registry::default());
    let relay = Arc::new(Relay::new(&config)?);
    let token = TokenGuard::new(&config.token);

    let server = {
        let (registry, root) = (registry.clone(), root.clone());
        HttpServer::new(move || {
            let (list_registry, list_root) = (registry.clone(), root.clone());
            web::App::new()
                .state(registry.clone())
                .state(JsonConfig::default().limit(MAX_BODY))
                .wrap(DefaultHeaders::new().header("x-content-type-options", "nosniff"))
                .route("/", web::put().guard(token.clone()).to(api::put_origin))
                .route(
                    "/",
                    web::delete().guard(token.clone()).to(api::delete_origin),
                )
                // without the right token
                .route("/", web::put().to(api::unauthorized))
                .route("/", web::delete().to(api::unauthorized))
                .service(
                    fs::Files::new("/", list_root.root())
                        .use_guards(list_root.clone())
                        .show_files_listing()
                        .redirect_to_slash_directory()
                        .files_listing_renderer(move |dir, req| {
                            listing::directory_listing(&list_registry, &list_root, dir, req)
                        }),
                )
        })
        // we stop on our own signal handling (sync::run), after the relay has been told
        .disable_signals()
        .workers(config.workers)
        .bind((config.host.as_str(), config.port))?
        .run()
    };

    log::info!(
        "serving {} on {}:{}",
        root.root().display(),
        config.host,
        config.port
    );

    let stop = server.clone();
    tokio::select! {
        result = server => result?,
        _ = sync::run(relay, root) => stop.stop(true).await,
    }
    Ok(())
}
