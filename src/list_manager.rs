use std::collections::BTreeSet;

use anyhow::Error;
use chrono::{Days, Local};
use itertools::Itertools;
use reqwest::multipart::Form;

use crate::{api_cache::ApiCache, api_client::ApiClient, api_helpers, api_models};

const UPDATE_CHUNK_SIZE: usize = 250;

enum ListManagerKind {
    LastStatus1d,
    LastStatus1w,
    Mutuals,
}

pub struct ListManager {
    list: api_models::List,
    kind: ListManagerKind,
}

impl ListManager {
    fn new(list: api_models::List, kind: ListManagerKind) -> Self {
        ListManager { list, kind }
    }

    pub fn parse(list: api_models::List) -> Option<Self> {
        match list.title.as_str() {
            "#last_status_at<1d" => Some(Self::new(list, ListManagerKind::LastStatus1d)),
            "#last_status_at<1w" => Some(Self::new(list, ListManagerKind::LastStatus1w)),
            "#mutuals" => Some(Self::new(list, ListManagerKind::Mutuals)),
            _ => None,
        }
    }

    async fn get_new_member_ids(
        &mut self,
        client: &ApiClient,
        api_cache: &mut ApiCache,
    ) -> Result<BTreeSet<String>, Error> {
        let result = match self.kind {
            ListManagerKind::LastStatus1d => api_cache
                .get_follows(client)
                .await?
                .iter()
                .filter(|account| {
                    account
                        .last_status_at
                        .map_or(true, |x| x < Local::now().date_naive() - Days::new(1))
                })
                .map(|account| account.id.clone())
                .collect(),

            ListManagerKind::LastStatus1w => api_cache
                .get_follows(client)
                .await?
                .iter()
                .filter(|account| {
                    account
                        .last_status_at
                        .map_or(true, |x| x < Local::now().date_naive() - Days::new(7))
                })
                .map(|account| account.id.clone())
                .collect(),
            ListManagerKind::Mutuals => {
                let follows = api_cache.get_follows(client).await?;
                let follow_ids = follows.iter().map(|account| account.id.clone()).collect();
                api_cache.get_relationships(client, follow_ids).await?
                    .into_iter()
                    .filter(|relationship| relationship.following && relationship.followed_by)
                    .map(|relationship| relationship.id)
                    .collect()
            }
        };

        Ok(result)
    }

    pub async fn sync_list(
        &mut self,
        client: &ApiClient,
        api_cache: &mut ApiCache,
    ) -> Result<(), Error> {
        log::info!("syncing list {} ({})", self.list.id, self.list.title);

        let mut new_member_ids = self.get_new_member_ids(client, api_cache).await?;

        let mut url_opt = Some(format!("/api/v1/lists/{}/accounts", self.list.id));

        let mut to_delete = Vec::new();

        let mut num_old_accounts = 0usize;
        let num_new_accounts = new_member_ids.len();

        while let Some(url) = url_opt.clone() {
            let res = client.get(&url).await.send().await?.error_for_status()?;

            let next_url = api_helpers::get_next_link(&res);
            let accounts: Vec<api_models::Account> = res.json().await?;

            for account in accounts {
                num_old_accounts += 1;
                // we have ensured that account is in the list, cross it off
                let was_present = new_member_ids.remove(&account.id);

                // if it turns out that new_member_ids didn't contain the account, the account
                // isn't supposed to be on the list. enqueue it for deletion.
                if !was_present {
                    to_delete.push(account.id);
                }
            }

            url_opt = next_url;
        }

        for account_chunk in &new_member_ids.iter().cloned().chunks(UPDATE_CHUNK_SIZE) {
            let account_ids = account_chunk.collect_vec();
            log::debug!(
                "syncing list {} ({}): adding accounts: {:?}",
                self.list.id,
                self.list.title,
                account_ids
            );

            let mut formdata = Form::new();
            for id in account_ids {
                formdata = formdata.text("account_ids[]", id);
            }

            client
                .post(&format!("/api/v1/lists/{}/accounts", self.list.id))
                .await
                .multipart(formdata)
                .send()
                .await?
                .error_for_status()?;
        }

        for account_chunk in &to_delete.into_iter().chunks(UPDATE_CHUNK_SIZE) {
            let account_ids = account_chunk.collect_vec();
            log::debug!(
                "syncing list {} ({}): deleting accounts: {:?}",
                self.list.id,
                self.list.title,
                account_ids
            );

            let mut formdata = Form::new();

            for id in account_ids {
                formdata = formdata.text("account_ids[]", id);
            }

            client
                .delete(&format!("/api/v1/lists/{}/accounts", self.list.id))
                .await
                .multipart(formdata)
                .send()
                .await?
                .error_for_status()?;
        }

        log::info!("done syncing, went from {} to {} members", num_old_accounts, num_new_accounts);

        Ok(())
    }
}
