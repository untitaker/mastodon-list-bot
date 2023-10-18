use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use anyhow::Error;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Form, Json, Router,
};
use clap::Parser;
use serde::Serialize;

mod api_cache;
mod api_client;
mod api_helpers;
mod api_models;
mod config;
mod error;
mod list_manager;
mod runner;
mod store;

use config::Server;
use config::{Cli, Subcommand};
use error::ResponseError;
use store::{AccountPk, RegisterAccount};

#[tokio::main]
async fn main() -> Result<(), Error> {
    pretty_env_logger::init();

    let cli = Cli::parse();

    match cli.subcommand {
        Subcommand::RunOnce(run_once_cli) => {
            runner::run_once(&run_once_cli.host, &run_once_cli.token).await?;
        }
        Subcommand::Serve(server) => {
            serve(server).await?;
        }
    }

    Ok(())
}

#[derive(Clone)]
struct AppState {
    store: store::Store,
}

async fn serve(server_cli: Server) -> Result<(), Error> {
    let socketaddr_str = format!("{}:{}", server_cli.addr, server_cli.port);

    let store = store::Store::new(&server_cli.database).await?;
    let cronjob_store = store.clone();

    let state = AppState { store };

    let _cronjob = tokio::spawn(async move {
        match cronjob_store.sync_all_accounts().await {
            Ok((success, failure)) => {
                log::info!("cronjob: {} success, {} failure", success, failure)
            }
            Err(e) => log::error!("failed to run cronjob: {:?}", e),
        }

        tokio::time::sleep(Duration::from_secs(3600)).await;
    });

    let app = Router::new()
        .route("/", get(|| async { Html(include_str!("index.html")) }))
        // If this line is failing compilation, you need to run 'yarn install && yarn build' to get your CSS bundle.
        .route(
            "/bundle.css",
            get(|| async {
                (
                    [("Content-Type", "text/css")],
                    include_str!("../build/bundle.css"),
                )
            }),
        )
        .route(
            "/bundle.js",
            get(|| async {
                (
                    [("Content-Type", "application/javascript")],
                    include_str!("../build/bundle.js"),
                )
            }),
        )
        .route("/register", post(register))
        .route("/sync-immediate", post(sync_immediate))
        .with_state(state);

    log::info!("listening on {}", socketaddr_str);
    let addr = SocketAddr::from_str(&socketaddr_str).expect("invalid host/port for server");

    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();

    Ok(())
}

async fn register(
    State(state): State<AppState>,
    Form(register_account): Form<RegisterAccount>,
) -> Result<Response, ResponseError> {
    let account = state.store.register(register_account).await?;
    Ok(Json(account).into_response())
}

#[derive(Serialize)]
#[serde(rename = "snake_case")]
enum SyncImmediateResult {
    Ok,
    Error(String),
    Pending,
}

async fn sync_immediate(
    State(state): State<AppState>,
    Form(account_pk): Form<AccountPk>,
) -> Result<Response, ResponseError> {
    let result = state.store.sync_immediate(account_pk).await?;
    let body = match result {
        Some(Ok(_)) => SyncImmediateResult::Ok,
        Some(Err(e)) => SyncImmediateResult::Error(e.to_string()),
        None => SyncImmediateResult::Pending,
    };

    Ok(Json(body).into_response())
}
