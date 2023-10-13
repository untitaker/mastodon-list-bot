use anyhow::Error;

mod api_cache;
mod api_client;
mod api_helpers;
mod api_models;
mod list_manager;

#[tokio::main]
async fn main() -> Result<(), Error> {
    pretty_env_logger::init();

    let instance = std::env::var("MASTODON_INSTANCE").expect("missing MASTODON_INSTANCE envvar");
    let token = std::env::var("MASTODON_TOKEN").expect("missing MASTODON_TOKEN envvar");

    let api_client = api_client::ApiClient::new(instance, token)?;

    log::info!("fetching all your lists");

    let mut url_opt = Some("/api/v1/lists".to_owned());

    let mut list_managers = Vec::new();

    while let Some(url) = url_opt.clone() {
        let res = api_client
            .get(&url)
            .await
            .send()
            .await?
            .error_for_status()?;

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

    let mut api_cache = api_cache::ApiCache::default();

    for mut manager in list_managers {
        manager.sync_list(&api_client, &mut api_cache).await?;
    }

    Ok(())
}
