use chrono::NaiveDate;
use serde::{de, de::Error as _, Deserialize, Deserializer};

#[derive(Deserialize, Debug, Clone)]
pub struct List {
    pub id: String,
    pub title: String,
}

// https://github.com/superseriousbusiness/gotosocial/issues/3418
fn date_deserialize<'de, D>(deserializer: D) -> Result<Option<NaiveDate>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<&str> = de::Deserialize::deserialize(deserializer).map_err(D::Error::custom)?;
    let Some(s) = s else { return Ok(None) };
    let (date, _) = NaiveDate::parse_and_remainder(s, "%Y-%m-%d").map_err(D::Error::custom)?;
    Ok(Some(date))
}

#[derive(Deserialize, Debug, Clone)]
pub struct Account {
    pub id: String,
    #[serde(deserialize_with = "date_deserialize")]
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
