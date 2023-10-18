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
    pub username: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Relationship {
    pub id: String,
    pub following: bool,
    pub followed_by: bool,
}

#[test]
fn test_deserialize_account() {
    let _account: Account = serde_json::from_str(
        r##"
    {
      "id": "23634",
      "last_status_at": "2019-11-17"
    }
    "##,
    )
    .unwrap();
}
