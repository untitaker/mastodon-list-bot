use std::collections::BTreeSet;

use anyhow::Error;
use chrono::{Days, Local};
use itertools::Itertools;
use reqwest::multipart::Form;

use crate::{
    api_cache::ApiCache,
    api_helpers,
    api_models,
};

const UPDATE_CHUNK_SIZE: usize = 250;

enum ListManagerKind {
    LastStatus1d,
    LastStatus1w,
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
            _ => None,
        }
    }

    pub async fn sync_list(
        &mut self,
        instance: &str,
        client: &reqwest::Client,
        api_cache: &mut ApiCache,
    ) -> Result<(), Error> {
        log::info!("syncing list {} ({})", self.list.id, self.list.title);

        let mut new_member_ids = BTreeSet::new();

        for account in api_cache.get_follows(instance, client).await? {
            let is_match = match self.kind {
                ListManagerKind::LastStatus1d => account
                    .last_status_at
                    .map_or(true, |x| x < Local::now().date_naive() - Days::new(1)),
                ListManagerKind::LastStatus1w => account
                    .last_status_at
                    .map_or(true, |x| x < Local::now().date_naive() - Days::new(7)),
            };

            if !is_match {
                continue;
            }

            new_member_ids.insert(account.id.clone());
        }

        let mut url_opt = Some(format!(
            "{}/api/v1/lists/{}/accounts",
            instance, self.list.id
        ));

        let mut to_delete = Vec::new();

        while let Some(url) = url_opt.clone() {
            let res = client.get(url).send().await?.error_for_status()?;

            let next_url = api_helpers::get_next_link(&res);
            let accounts: Vec<api_models::Account> = res.json().await?;

            for account in accounts {
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
            log::info!(
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
                .post(format!(
                    "{}/api/v1/lists/{}/accounts",
                    instance, self.list.id
                ))
                .multipart(formdata)
                .send()
                .await?
                .error_for_status()?;
        }

        for account_chunk in &to_delete.into_iter().chunks(UPDATE_CHUNK_SIZE) {
            let account_ids = account_chunk.collect_vec();
            log::info!(
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
                .delete(format!(
                    "{}/api/v1/lists/{}/accounts",
                    instance, self.list.id
                ))
                .multipart(formdata)
                .send()
                .await?
                .error_for_status()?;
        }

        Ok(())
    }
}
