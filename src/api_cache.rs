use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Error};
use itertools::Itertools;

use crate::api_client::ApiClient;
use crate::api_helpers;
use crate::api_models::{Account, CredentialAccount, Relationship};

const RELATIONSHIP_FETCH_CHUNK_SIZE: usize = 40;

#[derive(Default)]
pub struct ApiCache {
    follows: Option<Vec<Account>>,
    relationships: BTreeMap<String, Relationship>,
}

impl ApiCache {
    pub async fn get_relationships(
        &mut self,
        client: &ApiClient,
        mut account_ids: BTreeSet<String>,
    ) -> Result<Vec<Relationship>, Error> {
        let mut result = Vec::new();

        account_ids.retain(|id| {
            if let Some(item) = self.relationships.get(id) {
                result.push(item.clone());
                false
            } else {
                true
            }
        });

        for account_chunk in account_ids
            .into_iter()
            .collect_vec()
            .chunks(RELATIONSHIP_FETCH_CHUNK_SIZE)
        {
            let account_ids = account_chunk.to_vec();

            log::debug!("fetching relationships: {:?}", account_ids);

            let chunk_result: Vec<Relationship> = client
                .get(
                    "/api/v1/accounts/relationships",
                    Box::new(move |builder| {
                        builder
                            .header("content-type", "application/x-www-form-urlencoded")
                            .query(&account_ids.iter().map(|id| ("id[]", id)).collect_vec())
                    }),
                )
                .await
                .context("failed to get relationships")?
                .error_for_status()
                .context("failed to get relationships")?
                .json()
                .await
                .context("failed to parse relationships")?;

            for relationship in chunk_result {
                self.relationships
                    .insert(relationship.id.clone(), relationship.clone());
                result.push(relationship);
            }
        }

        Ok(result)
    }

    pub async fn get_follows(&mut self, client: &ApiClient) -> Result<&[Account], Error> {
        if let Some(ref follows) = self.follows {
            return Ok(follows);
        }

        log::info!("fetching all your follows");

        // TODO: cache that too
        let res: CredentialAccount = client
            .get(
                "/api/v1/accounts/verify_credentials",
                Box::new(|builder| builder),
            )
            .await
            .context("failed to get CredentialAccount")?
            .error_for_status()
            .context("failed to get CredentialAccount")?
            .json()
            .await
            .context("failed to get CredentialAccount")?;

        let mut url_opt = Some(format!("/api/v1/accounts/{}/following", res.id));

        let mut result = Vec::new();

        while let Some(url) = url_opt.clone() {
            let res = client
                .get(&url, Box::new(|builder| builder))
                .await
                .context("failed to get follows")?
                .error_for_status()
                .context("failed to get follows")?;

            let next_url = api_helpers::get_next_link(&res);
            let accounts: Vec<Account> =
                res.json().await.context("failed to parse follows result")?;

            result.extend(accounts);

            url_opt = next_url;
        }

        self.follows = Some(result);
        Ok(self.follows.as_ref().unwrap())
    }
}
