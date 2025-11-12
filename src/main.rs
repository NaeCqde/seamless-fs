mod data;
mod env;
mod handler;

use data::STATE;
use handler::{delete_origin, handle_delete_origin, handle_insert_origin, handle_update_notify};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use ntex::{
    http::header,
    web::{self, guard, types, HttpServer},
};
use ntex_files as fs;
use std::{error::Error, path::Path, process::exit, time::Duration};
use tokio::{signal, sync::mpsc, task::JoinSet, time::interval};

#[ntex::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let mut set = JoinSet::new();
    let server = HttpServer::new(|| {
        web::App::new()
            .state(
                // change json extractor configuration
                types::JsonConfig::default().limit(1024 * 500),
            )
            .route(
                "/",
                web::put()
                    .guard(guard::Header(header::AUTHORIZATION.as_str(), &STATE.token))
                    .to(handle_insert_origin),
            )
            .route(
                "/",
                web::delete()
                    .guard(guard::Header(header::AUTHORIZATION.as_str(), &STATE.token))
                    .to(handle_delete_origin),
            )
            .service(
                fs::Files::new("/", ".")
                    .show_files_listing()
                    .redirect_to_slash_directory()
                    .files_listing_renderer(handler::directory_listing),
            )
    })
    .bind((STATE.host.to_owned(), STATE.port))
    .expect("failed to bind port");

    set.spawn(server.run());

    delete_origin().await;
    handle_update_notify(true).await;

    // Tokio MPSC チャンネルで通知を受け取る
    let (tx, mut rx) = mpsc::channel::<Event>(1);

    // ファイル監視用の watcher
    let mut watcher: RecommendedWatcher = Watcher::new(
        move |res| {
            // イベントを MPSC チャンネルに送信
            if let Ok(event) = res {
                let _ = tx.blocking_send(event);
            }
        },
        Config::default(),
    )
    .expect("failed to init watcher");

    // カレントディレクトリ以下を再帰監視
    watcher
        .watch(Path::new("."), RecursiveMode::Recursive)
        .expect("failed to watch current directory");

    let mut ticker = interval(Duration::from_secs(10));
    let mut changed = false;

    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                match &event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                        if !changed {
                            changed = true
                        }
                    }
                    _ => {}
                }
            },
            _ = ticker.tick() => {
                if changed {
                    changed = false;

                    handle_update_notify(false).await;
                }
            }
            _ = signal::ctrl_c()=>{
                delete_origin().await;
                set.shutdown().await;

                exit(0);
            },
        }
    }
}
