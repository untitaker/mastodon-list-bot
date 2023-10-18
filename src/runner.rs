use anyhow::Error;

use crate::api_cache::ApiCache;
use crate::api_client::ApiClient;
use crate::api_helpers::get_next_link;
use crate::api_models::List;
use crate::list_manager::ListManager;

pub async fn run_once(instance: &str, token: &str) -> Result<(), Error> {
    let api_client = ApiClient::new(instance, token)?;

    log::info!("fetching all your lists");

    let mut url_opt = Some("/api/v1/lists".to_owned());

    let mut list_managers = Vec::new();

    while let Some(url) = url_opt.clone() {
        let res = api_client
            .get(&url, Box::new(|builder| builder))
            .await?
            .error_for_status()?;

        let next_url = get_next_link(&res);

        let lists: Vec<List> = res.json().await?;

        for list in lists {
            if let Some(manager) = ListManager::parse(list) {
                list_managers.push(manager);
            }
        }

        url_opt = next_url;
    }

    if list_managers.is_empty() {
        return Ok(());
    }

    let mut api_cache = ApiCache::default();

    for mut manager in list_managers {
        manager.sync_list(&api_client, &mut api_cache).await?;
    }

    Ok(())
}
