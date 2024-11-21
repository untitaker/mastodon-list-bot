use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::{request::Parts, StatusCode};
use tower_sessions::Session;

use crate::AccountPk;
use crate::ResponseError;

pub const SESSION_COOKIE_KEY: &str = "auth";

pub struct LoggedIn {
    pub account: Option<AccountPk>,
}

impl LoggedIn {
    pub fn account(&self) -> Result<AccountPk, ResponseError> {
        self.account.clone().ok_or(ResponseError::NeedsAuth)
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for LoggedIn
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(req: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(req, state).await?;
        let account: Option<AccountPk> = session.get(SESSION_COOKIE_KEY).await.unwrap();
        Ok(LoggedIn { account })
    }
}
