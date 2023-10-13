use anyhow::{Context, Error};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};
use serde::Deserialize;

mod api_helpers;
mod api_models;
mod list_manager;

#[derive(Deserialize, Debug)]
struct CredentialAccount {
    id: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    pretty_env_logger::init();

    let instance = std::env::var("MASTODON_INSTANCE").expect("missing MASTODON_INSTANCE envvar");
    let token = std::env::var("MASTODON_TOKEN").expect("missing MASTODON_TOKEN envvar");

    let mut headers = HeaderMap::new();
    headers.insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).context("invalid bearer token")?,
    );

    headers.insert(
        "User-Agent",
        HeaderValue::from_str(&format!("mastodon-link-bot/{}", env!("CARGO_PKG_VERSION"))).unwrap(),
    );

    let client = Client::builder()
        .use_rustls_tls()
        .default_headers(headers)
        .build()?;

    log::info!("fetching all your lists");

    let mut url_opt = Some(format!("{}/api/v1/lists", instance));

    let mut list_managers = Vec::new();

    while let Some(url) = url_opt.clone() {
        let res = client.get(url).send().await?.error_for_status()?;

        let next_url = api_helpers::get_next_link(&res);

        let lists: Vec<api_models::List> = res.json().await?;

        for list in lists {
            if let Some(manager) = list_manager::ListManager::parse(list) {
                list_managers.push(manager);
            }
        }

        url_opt = next_url;
    }

    if list_managers.is_empty() {
        return Ok(());
    }

    let res: CredentialAccount = client
        .get(format!("{}/api/v1/accounts/verify_credentials", instance))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    log::info!("fetching all your follows");

    let mut url_opt = Some(format!("{}/api/v1/accounts/{}/following", instance, res.id));

    while let Some(url) = url_opt.clone() {
        let res = client.get(url).send().await?.error_for_status()?;

        let next_url = api_helpers::get_next_link(&res);
        let accounts: Vec<api_models::Account> = res.json().await?;

        for account in accounts {
            for list_manager in &mut list_managers {
                list_manager.consider_as_member(&account);
            }
        }

        url_opt = next_url;
    }

    for mut manager in list_managers {
        manager.sync_list(&instance, &client).await?;
    }

    Ok(())
}

#[test]
fn test_deserialize_account() {
    let account: Account = serde_json::from_str(
        r##"
    {
      "id": "23634",
      "last_status_at": "2019-11-17"
    }
    "##,
    )
    .unwrap();
}
