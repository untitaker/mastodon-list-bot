use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use anyhow::Error;
use api_client::ApiClient;
use axum::{
    body::Body,
    debug_handler,
    extract::{Host, Query, State},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Router,
};
use clap::Parser;
use maud::Markup;
use serde::{Deserialize, Serialize};
use tower_sessions::{Expiry, Session};

mod api_cache;
mod api_client;
mod api_helpers;
mod api_models;
mod auth;
mod config;
mod error;
mod list_manager;
mod runner;
mod store;

use config::Server;
use config::{Cli, Subcommand};
use error::ResponseError;
use memory_serve::{load_assets, MemoryServe};
use store::{AccountPk, RegisterAccount, SyncImmediateResult};
use tower_sessions::{MemoryStore, SessionManagerLayer};

use crate::auth::{LoggedIn, SESSION_COOKIE_KEY};

#[tokio::main]
async fn main() -> Result<(), Error> {
    use tracing_subscriber::prelude::*;

    let _guard = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        ..sentry::ClientOptions::default()
    });

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_env("RUST_LOG"))
        .with(sentry::integrations::tracing::layer())
        .init();

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
        loop {
            match cronjob_store.sync_all_accounts().await {
                Ok((success, failure)) => {
                    tracing::info!("cronjob: {} success, {} failure", success, failure)
                }
                Err(e) => tracing::error!("failed to run cronjob: {:?}", e),
            }

            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });

    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(!cfg!(debug_assertions))
        // https://bugzilla.mozilla.org/show_bug.cgi?id=1465402
        // https://issues.chromium.org/issues/40508226#comment2
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(time::Duration::seconds(3600)));

    let static_files = MemoryServe::new(load_assets!("static")).into_router();

    let app = Router::new()
        .merge(static_files)
        .route("/", get(index))
        .route("/account/login", post(account_login))
        .route("/account/logout", post(account_logout))
        .route("/account/sync-immediate", post(sync_immediate))
        .route("/account/oauth-redirect", get(account_redirect))
        .route("/account/admin", get(account_admin))
        .layer(session_layer)
        .with_state(state);

    tracing::info!("listening on {}", socketaddr_str);
    let addr = SocketAddr::from_str(&socketaddr_str).expect("invalid host/port for server");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();

    Ok(())
}

async fn index() -> Response {
    let html = maud::html! {
        p {
            "Create programmatic lists in " a href="https://joinmastodon.org" { "Mastodon" } ". Take a look at the " a href="https://github.com/untitaker/mastodon-list-bot" { "GitHub project" } " for more information."
        }

        form action="/account/login" method="post" {
            fieldset role="group" {
                input
                    type="text"
                    required
                    name="host"
                    placeholder="e.g. mastodon.social";

                input
                    type="submit"
                    value="Sync Lists";
            }
        }
    };

    Html(with_site_chrome(html).into_string()).into_response()
}

async fn sync_immediate(
    State(state): State<AppState>,
    login: LoggedIn,
) -> Result<Response, ResponseError> {
    let account_pk = login.account()?;
    let body = state.store.sync_immediate(account_pk).await?;

    let html: maud::Markup = match body {
        SyncImmediateResult::Ok => maud::html! {
            p { "Done syncing! Refresh the page to see results. Future updates to your lists will happen automatically." }
        },
        SyncImmediateResult::Error { value } => maud::html! {
            p.red { "Error: "(value) }
        },
        SyncImmediateResult::Pending => maud::html! {
            p { "Sync ongoing." }
        },
        SyncImmediateResult::TooMany => maud::html! {
            p { "Sync has been done recently, not starting another one." }
        },
    };

    Ok(Html(html.into_string()).into_response())
}

#[derive(Deserialize)]
struct AccountRegister {
    host: String,
}

#[derive(Deserialize, Serialize)]
struct OauthState {
    client_id: String,
    client_secret: String,
    host: String,
}

fn get_service_uri(Host(self_host): Host) -> String {
    if cfg!(debug_assertions) {
        format!("http://{self_host}")
    } else {
        format!("https://{self_host}")
    }
}

async fn account_login(
    self_host: Host,
    Form(AccountRegister { host }): Form<AccountRegister>,
) -> Result<Response, ResponseError> {
    let service_uri = get_service_uri(self_host);
    let self_redirect_uri = format!("{service_uri}/account/oauth-redirect");

    let client = ApiClient::new(&host, None).unwrap();
    let scopes = "read:follows read:lists read:accounts write:lists";

    #[derive(Deserialize)]
    struct OauthAppResponse {
        client_id: String,
        client_secret: String,
    }

    let OauthAppResponse {
        client_id,
        client_secret,
    } = client
        .client
        .post(format!("https://{host}/api/v1/apps"))
        .form(&[
            ("client_name", "Mastodon List Bot"),
            ("website", &service_uri),
            ("scopes", scopes),
            ("redirect_uris", &self_redirect_uri),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let state = OauthState {
        client_id: client_id.clone(),
        client_secret: client_secret.clone(),
        host: host.clone(),
    };
    // there is no point in hiding the client secret from the user. we already create one secret
    // per user to begin with, and mastodon allows anybody to register multiple applications with
    // the same name/website/logo. backend-less SPAs already work like this, and it's a waste of
    // resources to make a separate "tokens" table mapping host -> client ID/secret
    let state = serde_json::to_string(&state).unwrap();

    let foreign_redirect_uri = format!("https://{host}/oauth/authorize?scope={scopes}&response_type=code&redirect_uri={self_redirect_uri}&client_id={client_id}&client_secret={client_secret}&state={state}");

    Ok(Response::builder()
        .status(302)
        .header("Location", foreign_redirect_uri)
        .body(Body::empty())
        .unwrap())
}

#[debug_handler]
async fn account_logout(session: Session) -> Result<Response, ResponseError> {
    session.remove::<AccountPk>(SESSION_COOKIE_KEY).await?;
    Ok(Redirect::to("/").into_response())
}

#[derive(Deserialize)]
struct OauthAccountRedirect {
    code: String,
    state: String,
}

#[debug_handler]
async fn account_redirect(
    session: Session,
    self_host: Host,
    State(state): State<AppState>,
    Query(OauthAccountRedirect {
        code,
        state: oauth_state,
    }): Query<OauthAccountRedirect>,
) -> Result<Response, ResponseError> {
    let service_uri = get_service_uri(self_host);
    let self_redirect_uri = format!("{service_uri}/account/oauth-redirect");

    let OauthState {
        client_id,
        client_secret,
        host,
    } = serde_json::from_str(&oauth_state)?;
    let client = ApiClient::new(&host, None).unwrap();

    #[derive(Deserialize)]
    struct OauthTokenResponse {
        access_token: String,
    }

    let OauthTokenResponse { access_token } = client
        .client
        .post(format!("https://{host}/oauth/token"))
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &self_redirect_uri),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let register_account = RegisterAccount {
        host,
        token: access_token,
    };

    let account = state.store.register(register_account).await?;
    session
        .insert(SESSION_COOKIE_KEY, account.primary_key())
        .await?;
    Ok(Redirect::to("/account/admin").into_response())
}

#[debug_handler]
async fn account_admin(
    State(state): State<AppState>,
    login: LoggedIn,
) -> Result<Response, ResponseError> {
    let account_pk = login.account()?;
    let account = state.store.get_account(account_pk).await?;

    let html = maud::html! {
        div {
            div.grid {
                h2 { "Hello "(account.username)"@"(account.host)"!" }
                form
                    method="post"
                    action="/account/logout" {
                        input.secondary.outline type="submit" value="Logout";
                    }
            }

            @if account.failure_count > 0 {
                p.red {
                    "We have encountered "(account.failure_count)" fatal errors when trying to sync. After 10 attempts, we will stop synchronizing."
                }
            }

            @if let Some(err) = account.last_error {
                p."pico-color-red-500" {
                    "The last error we encountered was: "(err)
                }
            }

            @if let Some(d) = account.last_success_at {
                p { "Your last successful sync was at "(d)"." }
                p { (account.list_count)" dynamic lists were found." }
            } @else {
                p { "Not synced yet." }
            }

            p {
                "Your lists will be updated once per day. Take a look at the " a href="https://github.com/untitaker/mastodon-list-bot#how-to-use" { "README" } " to see which list names are supported. After that, click Sync Now."
            }

            form
            method="post"
            action="/account/sync-immediate"
            target="_blank"
            data-hx-post="/account/sync-immediate"
            data-hx-swap="innerHTML"
            data-hx-target="#sync-result"
            data-hx-disabled-elt="#sync-now" {
                input id="sync-now" type="submit" value="Sync now";
                p id="sync-result";
            }

            script src="/htmx.js" {}
        }
    };

    Ok(Html(with_site_chrome(html).into_string()).into_response())
}

fn with_site_chrome(content: Markup) -> Markup {
    maud::html! {
        (maud::DOCTYPE)
        html lang="en" {
            head {
                title { "Mastodon List Bot" }
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                link rel="stylesheet" href="/pico.css";
                link rel="stylesheet" href="/pico.colors.css";
            }

            body {
                header.container {
                    h1 { "Mastodon List Bot" }
                }

                main.container {
                    (content)
                }
            }
        }
    }
}
