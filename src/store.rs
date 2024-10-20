use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Error;
use chrono::{Duration, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::SqlitePool;
use sqlx::Sqlite;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::api_client::ApiClient;
use crate::api_models::CredentialAccount;
use crate::error::ResponseError;

type ImmediateSyncHandle = JoinHandle<Result<(), Error>>;

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
    immediate_syncs: Arc<Mutex<BTreeMap<AccountPk, (Account, ImmediateSyncHandle)>>>,
}

#[derive(Clone, Debug, Ord, Eq, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct AccountPk {
    host: String,
    username: String,
}

impl AccountPk {
    fn as_handle(&self) -> String {
        format!("{}@{}", self.username, self.host)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Account {
    pub host: String,
    pub username: String,
    #[serde(skip)]
    pub token: String,
    pub created_at: NaiveDateTime,
    pub last_success_at: Option<NaiveDateTime>,
    pub failure_count: i64,
    pub last_error: Option<String>,
}

#[derive(Deserialize)]
pub struct RegisterAccount {
    pub host: String,
    pub token: String,
}

impl Account {
    pub fn primary_key(&self) -> AccountPk {
        AccountPk {
            host: self.host.clone(),
            username: self.username.clone(),
        }
    }
}

impl Store {
    pub async fn new(database_url: &str) -> Result<Self, Error> {
        let _ = Sqlite::create_database(database_url).await;
        let pool = SqlitePool::connect(database_url).await?;

        sqlx::migrate!("./migrations").run(&pool).await?;
        let immediate_syncs = Arc::new(Mutex::new(BTreeMap::new()));

        Ok(Store {
            pool,
            immediate_syncs,
        })
    }

    pub async fn register(&self, account: RegisterAccount) -> Result<Account, ResponseError> {
        let client = ApiClient::new(&account.host, Some(&account.token))?;

        let res: CredentialAccount = client
            .get(
                "/api/v1/accounts/verify_credentials",
                Box::new(|builder| builder),
            )
            .await?
            .error_for_status()?
            .json()
            .await?;

        let account = Account {
            host: account.host,
            token: account.token,
            username: res.username,
            created_at: Utc::now().naive_utc(),
            last_success_at: None,
            last_error: None,
            failure_count: 0,
        };

        // XXX: ugly
        sqlx::query!(
            "insert into accounts ( host, token, username, created_at, last_success_at, last_error, failure_count ) values ( ?1, ?2, ?3, ?4, ?5, ?6, ?7 )
            on conflict do update
            set token = ?2",
            account.host, account.token, account.username, account.created_at, account.last_success_at, account.last_error, account.failure_count
        ).execute(&self.pool).await?;

        let account = sqlx::query_as!(
            Account,
            "select * from accounts where host = ?1 and username = ?2",
            account.host,
            account.username
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(account)
    }

    pub async fn sync_immediate(
        &self,
        account_pk: AccountPk,
    ) -> Result<SyncImmediateResult, ResponseError> {
        let account = sqlx::query_as!(
            Account,
            "select * from accounts where host = ?1 and username = ?2",
            account_pk.host,
            account_pk.username
        )
        .fetch_one(&self.pool)
        .await?;

        let mut immediate_syncs = self.immediate_syncs.lock().await;

        if !immediate_syncs.contains_key(&account_pk) {
            if let Some(last_success_at) = account.last_success_at {
                if last_success_at > Utc::now().naive_utc() - Duration::minutes(30) {
                    return Ok(SyncImmediateResult::TooMany);
                }
            }
        }

        let handle = &mut immediate_syncs
            .entry(account_pk.clone())
            .or_insert_with(move || {
                log::info!("immediate sync for {}", account.primary_key().as_handle());
                let slf = self.clone();
                let account2 = account.clone();
                let future = async move { slf.run_once_and_log(account).await? };
                (account2, tokio::spawn(future))
            })
            .1;

        if !handle.is_finished() {
            return Ok(SyncImmediateResult::Pending);
        }

        let result = handle.await?;
        immediate_syncs.remove(&account_pk);

        Ok(match result {
            Ok(()) => SyncImmediateResult::Ok,
            Err(e) => SyncImmediateResult::Error {
                value: e.to_string(),
            },
        })
    }

    async fn run_once_and_log(&self, account: Account) -> Result<Result<(), Error>, ResponseError> {
        match crate::runner::run_once(&account.host, &account.token).await {
            Ok(()) => {
                sqlx::query!(
                    "update accounts set
                    last_success_at = datetime('now'),
                    failure_count = 0,
                    last_error = null
                    where host = ?1 and username = ?2
                    ",
                    account.host,
                    account.username,
                )
                .execute(&self.pool)
                .await?;
                Ok(Ok(()))
            }
            Err(e) => {
                let e_str = format!("{:?}", e);
                sqlx::query!(
                    "update accounts set
                    failure_count = failure_count + 1,
                    last_error = ?3
                    where host = ?1 and username = ?2",
                    account.host,
                    account.username,
                    e_str,
                )
                .execute(&self.pool)
                .await?;
                Ok(Err(e))
            }
        }
    }

    pub async fn sync_all_accounts(&self) -> Result<(usize, usize), Error> {
        // sync all accounts that have already been synced at least once
        let results = sqlx::query_as!(
            Account,
            "select * from accounts
            where (last_success_at is not null and failure_count < 10 and last_success_at < datetime('now', '-1 days'))
            and failure_count < 10
            limit 10"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut success_count = 0;
        let mut failure_count = 0;

        for account in results {
            let account_pk = account.primary_key();
            if self.immediate_syncs.lock().await.contains_key(&account_pk) {
                log::warn!(
                    "skipping cronjob for account {:?}, found immediate sync pending",
                    account_pk
                );
                continue;
            }

            match self.run_once_and_log(account).await? {
                Ok(_) => {
                    success_count += 1;
                }
                Err(_) => {
                    failure_count += 1;
                }
            }
        }

        Ok((success_count, failure_count))
    }
}

#[derive(Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum SyncImmediateResult {
    Ok,
    Error { value: String },
    Pending,
    TooMany,
}
