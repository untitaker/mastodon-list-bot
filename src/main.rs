use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use anyhow::Error;
use api_client::ApiClient;
use axohtml::{dom::DOMTree, elements::FlowContent, html, text, unsafe_text};
use axum::{
    body::Body,
    debug_handler,
    extract::{Host, Query, State},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Form, Router,
};
use clap::Parser;
use serde::{Deserialize, Serialize};

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
use store::{AccountPk, RegisterAccount, SyncImmediateResult};

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
        loop {
            match cronjob_store.sync_all_accounts().await {
                Ok((success, failure)) => {
                    log::info!("cronjob: {} success, {} failure", success, failure)
                }
                Err(e) => log::error!("failed to run cronjob: {:?}", e),
            }

            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });

    let app = Router::new()
        .route("/", get(index))
        // If this line is failing compilation, you need to run 'npm install && npm run build' to get your CSS bundle.
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
            "/htmx.js",
            get(|| async {
                (
                    [("Content-Type", "application/javascript")],
                    include_str!("../node_modules/htmx.org/dist/htmx.min.js"),
                )
            }),
        )
        .route("/account/login", post(account_login))
        .route("/account/sync-immediate", post(sync_immediate))
        .route("/account", get(account))
        .with_state(state);

    log::info!("listening on {}", socketaddr_str);
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
    let html = html! {
      <form class="pure-form pure-form-stacked" action="/account/login" method="post">
        <label for="host">"Your instance"</label>
        <input
          type="text"
          id="host"
          class="pure-input-1"
          required=true
          name="host"
          placeholder="e.g. mastodon.social"
          pattern="[a-zA-Z0-9.:\\-]+"
          title="Something that looks like a hostname"
        />

        <input
          type="submit"
          class="pure-button pure-button-primary"
          value="Sync Lists"
        />
      </form>
    };

    Html(with_site_chrome(html).to_string()).into_response()
}

async fn sync_immediate(
    State(state): State<AppState>,
    Form(account_pk): Form<AccountPk>,
) -> Result<Response, ResponseError> {
    let body = state.store.sync_immediate(account_pk).await?;

    let html: DOMTree<String> = match body {
        SyncImmediateResult::Ok => html!(
            <p>"Done syncing! Future updates to your lists will happen automatically."</p>
        ),
        SyncImmediateResult::Error { value } => html!(
            <p class="red">{text!("Error: {}", value)}</p>
        ),
        SyncImmediateResult::Pending => html!(
            <p>"Sync ongoing."</p>
        ),
        SyncImmediateResult::TooMany => html!(
            <p>"Sync has been done recently, not starting another one."</p>
        ),
    };

    Ok(Html(html.to_string()).into_response())
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

async fn account_login(
    Host(self_host): Host,
    Form(AccountRegister { host }): Form<AccountRegister>,
) -> Result<Response, ResponseError> {
    let service_uri = format!("https://{self_host}");
    let self_redirect_uri = format!("{service_uri}/account");

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

#[derive(Deserialize)]
struct OauthAccountRedirect {
    code: String,
    state: String,
}

#[debug_handler]
async fn account(
    Host(self_host): Host,
    State(state): State<AppState>,
    Query(OauthAccountRedirect {
        code,
        state: oauth_state,
    }): Query<OauthAccountRedirect>,
) -> Result<Response, ResponseError> {
    let service_uri = format!("https://{self_host}");
    let self_redirect_uri = format!("{service_uri}/account");

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

    let html = html!(
      <div>
        // hide account credentials in query string from browser history
        <script>
        {unsafe_text!("history.replaceState({}, '', '/');")}
        </script>

        <p class="green">{text!("Hello {}@{}!", account.username, account.host)}</p>

        {if let Some(d) = account.last_success_at {
            html!(<p>{text!("Your last successful sync was at {}", d)}</p>)
        } else {
            html!(<p>"Not synced yet."</p>)
        }}

        <p>
          "Your lists will be updated once every day automatically. Take a look at the "
          <a href="https://github.com/untitaker/mastodon-list-bot#how-to-use">"README"</a>
          " to see which list names are supported. After that, click Sync Now."
        </p>

        <form
          class="pure-form"
          method="post"
          action="/account/sync-immediate"
          target="_blank"
          data-hx-post="/account/sync-immediate"
          data-hx-swap="innerHTML"
          data-hx-target="#sync-result"
          data-hx-disabled-elt="input[type=submit]"
        >
          <input type="hidden" name="host" value=account.host />
          <input type="hidden" name="username" value=account.username />
          <input type="submit" value="Sync now" />

          <p id="sync-result"></p>
        </form>

        {Some(html!(
          <p class="red">{text!("We have encountered {} fatal errors when trying to sync. After 10 attempted sync attempts, we will stop synchronizing.", account.failure_count)}</p>
        )).filter(|_| account.failure_count > 0)}

        {account.last_error.map(|err| html!(
          <p class="red">
            "The last error we encountered was: "
            <code>{text!("{}", err)}</code>
          </p>
        ))}

        <script src="/htmx.js"></script>
      </div>
    );

    Ok(Html(with_site_chrome(html).to_string()).into_response())
}

fn with_site_chrome(content: Box<dyn FlowContent<String>>) -> String {
    let html = html! {
      <html lang="en">
        <head>
          <title>"Mastodon List Bot"</title>
          <meta charset="utf-8" />
          <meta name="viewport" content="width=device-width, initial-scale=1" />
          <link rel="stylesheet" href="/bundle.css" />
        </head>

        <body>
          <div class="content">
            <h1>"Mastodon List Bot"</h1>
            <p>"Create programmatic lists in "<a href="https://joinmastodon.org">"Mastodon"</a>". Take a look at the "<a href="https://github.com/untitaker/mastodon-list-bot">"GitHub project"</a>" for more information."</p>

            {content}
          </div>
        </body>
      </html>
    };

    format!("<!DOCTYPE html>\n{html}")
}
