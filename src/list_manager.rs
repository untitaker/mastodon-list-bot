use std::collections::BTreeSet;
use std::str::FromStr;

use anyhow::{Context, Error};
use chrono::{Days, Local};
use itertools::Itertools;

use crate::{api_cache::ApiCache, api_client::ApiClient, api_helpers, api_models};

const UPDATE_CHUNK_SIZE: usize = 250;

#[derive(Debug, Clone, Eq, PartialEq)]
enum ListManagerTerm {
    LastStatus(Days),
    Mutuals,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ListManagerTerms(Vec<ListManagerTerm>); // and

impl FromStr for ListManagerTerms {
    type Err = pom::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use pom::parser::*;

        let whitespace = || sym(b' ').repeat(0..).discard();
        let duration_days =
            (one_of(b"123456789").repeat(1..2) + one_of(b"dwm")).map(|(number, unit)| {
                let unit = match unit {
                    b'd' => 1,
                    b'w' => 7,
                    b'm' => 30,
                    _ => unreachable!(),
                };

                let number = String::from_utf8(number).unwrap().parse::<u64>().unwrap();
                Days::new(number * unit)
            });

        let last_status_at =
            seq(b"last_status_at>") * duration_days.map(ListManagerTerm::LastStatus);
        let mutuals = seq(b"mutuals").map(|_| ListManagerTerm::Mutuals);
        let term = last_status_at | mutuals;
        let and_symbol = whitespace() + sym(b'&').repeat(1..) + whitespace();
        let and_term = list(term, and_symbol).convert(|terms| match terms.len() {
            0 => Err("empty filter"),
            _ => Ok(ListManagerTerms(terms)),
        });
        let complex_term = and_term;
        let parser = none_of(b"#").repeat(0..).discard() * sym(b'#') * complex_term;
        parser.parse(s.as_bytes())
    }
}

pub struct ListManager {
    list: api_models::List,
    terms: ListManagerTerms,
}

impl ListManager {
    fn new(list: api_models::List, terms: ListManagerTerms) -> Self {
        ListManager { list, terms }
    }

    pub fn parse(list: api_models::List) -> Option<Self> {
        let parsed = list.title.parse().ok()?;
        Some(Self::new(list, parsed))
    }

    async fn get_new_member_ids(
        &mut self,
        client: &ApiClient,
        api_cache: &mut ApiCache,
    ) -> Result<BTreeSet<String>, Error> {
        let mut first = true;
        let mut result = BTreeSet::new();

        for term in &self.terms.0 {
            let term_result = match term {
                ListManagerTerm::LastStatus(days) => api_cache
                    .get_follows(client)
                    .await?
                    .iter()
                    .filter(|account| {
                        account
                            .last_status_at
                            .map_or(true, |x| x < Local::now().date_naive() - *days)
                    })
                    .map(|account| account.id.clone())
                    .collect(),

                ListManagerTerm::Mutuals => {
                    let follows = api_cache.get_follows(client).await?;
                    let follow_ids = follows.iter().map(|account| account.id.clone()).collect();
                    api_cache
                        .get_relationships(client, follow_ids)
                        .await?
                        .into_iter()
                        .filter(|relationship| relationship.following && relationship.followed_by)
                        .map(|relationship| relationship.id)
                        .collect()
                }
            };

            if first {
                result = term_result;
                first = false;
            } else {
                result.retain(|x| term_result.contains(x));
            }
        }

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
            let res = client
                .get(&url, Box::new(|builder| builder))
                .await?
                .error_for_status()?;

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

        for account_chunk in new_member_ids
            .into_iter()
            .collect_vec()
            .chunks(UPDATE_CHUNK_SIZE)
        {
            let account_ids = account_chunk.to_vec();
            log::debug!(
                "syncing list {} ({}): adding accounts: {:?}",
                self.list.id,
                self.list.title,
                account_ids
            );

            client
                .post(
                    &format!("/api/v1/lists/{}/accounts", self.list.id),
                    Box::new(move |builder| {
                        builder.form(
                            &account_ids
                                .iter()
                                .map(|id| ("account_ids[]", id))
                                .collect_vec(),
                        )
                    }),
                )
                .await
                .context("failed to add members")?
                .error_for_status()
                .context("failed to add members")?;
        }

        for account_chunk in to_delete.chunks(UPDATE_CHUNK_SIZE) {
            let account_ids = account_chunk.to_vec();
            log::debug!(
                "syncing list {} ({}): deleting accounts: {:?}",
                self.list.id,
                self.list.title,
                account_ids
            );

            client
                .delete(
                    &format!("/api/v1/lists/{}/accounts", self.list.id),
                    Box::new(move |builder| {
                        builder.form(
                            &account_ids
                                .iter()
                                .map(|id| ("account_ids[]", id))
                                .collect_vec(),
                        )
                    }),
                )
                .await
                .context("failed to remove members")?
                .error_for_status()
                .context("failed to remove members")?;
        }

        log::info!(
            "done syncing, went from {} to {} members",
            num_old_accounts,
            num_new_accounts
        );

        Ok(())
    }
}

#[test]
fn parsing() {
    assert_eq!(
        ListManagerTerms::from_str("#mutuals"),
        Ok(ListManagerTerms(vec![ListManagerTerm::Mutuals]))
    );
    assert_eq!(
        ListManagerTerms::from_str("#last_status_at>2d"),
        Ok(ListManagerTerms(vec![ListManagerTerm::LastStatus(
            Days::new(2)
        )]))
    );
    assert_eq!(
        ListManagerTerms::from_str("#last_status_at>1w"),
        Ok(ListManagerTerms(vec![ListManagerTerm::LastStatus(
            Days::new(7)
        )]))
    );
    assert_eq!(
        ListManagerTerms::from_str("#last_status_at>1m"),
        Ok(ListManagerTerms(vec![ListManagerTerm::LastStatus(
            Days::new(30)
        )]))
    );
    assert_eq!(
        ListManagerTerms::from_str("hello #last_status_at>1m"),
        Ok(ListManagerTerms(vec![ListManagerTerm::LastStatus(
            Days::new(30)
        )]))
    );
}

#[test]
fn parsing_and() {
    assert_eq!(ListManagerTerms::from_str("hello #").is_err(), true,);
    assert_eq!(
        ListManagerTerms::from_str("hello #last_status_at>1m&mutuals"),
        Ok(ListManagerTerms(vec![
            ListManagerTerm::LastStatus(Days::new(30)),
            ListManagerTerm::Mutuals
        ]))
    );
    assert_eq!(
        ListManagerTerms::from_str("hello #last_status_at>1m & mutuals"),
        Ok(ListManagerTerms(vec![
            ListManagerTerm::LastStatus(Days::new(30)),
            ListManagerTerm::Mutuals
        ]))
    );

    assert_eq!(
        ListManagerTerms::from_str("hello #last_status_at>1m && mutuals"),
        Ok(ListManagerTerms(vec![
            ListManagerTerm::LastStatus(Days::new(30)),
            ListManagerTerm::Mutuals
        ]))
    );
}
