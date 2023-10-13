use chrono::NaiveDate;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct List {
    pub id: String,
    pub title: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Account {
    pub id: String,
    pub last_status_at: Option<NaiveDate>,
}

#[derive(Deserialize, Debug)]
pub struct CredentialAccount {
    pub id: String,
}
