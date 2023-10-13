use anyhow::Error;

use crate::api_helpers;
use crate::api_models::{Account, CredentialAccount};

#[derive(Default)]
pub struct ApiCache {
    follows: Option<Vec<Account>>,
}

impl ApiCache {
    pub async fn get_follows(
        &mut self,
        instance: &str,
        client: &reqwest::Client,
    ) -> Result<&[Account], Error> {
        if let Some(ref follows) = self.follows {
            return Ok(&follows);
        }

        log::info!("fetching all your follows");

        // TODO: cache that too
        let res: CredentialAccount = client
            .get(format!("{}/api/v1/accounts/verify_credentials", instance))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let mut url_opt = Some(format!("{}/api/v1/accounts/{}/following", instance, res.id));

        let mut result = Vec::new();

        while let Some(url) = url_opt.clone() {
            let res = client.get(url).send().await?.error_for_status()?;

            let next_url = api_helpers::get_next_link(&res);
            let accounts: Vec<Account> = res.json().await?;

            result.extend(accounts);

            url_opt = next_url;
        }

        self.follows = Some(result);
        Ok(self.follows.as_ref().unwrap())
    }
}
