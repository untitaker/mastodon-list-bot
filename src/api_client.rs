use std::time::Duration;

use anyhow::{Context, Error};
use async_throttle::MultiRateLimiter;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, Method, RequestBuilder,
};

pub struct ApiClient {
    client: Client,
    instance: String,
    rate_limiter: MultiRateLimiter<&'static str>,
}

impl ApiClient {
    pub fn new(instance: String, token: String) -> Result<Self, Error> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {}", token)).context("invalid bearer token")?,
        );

        headers.insert(
            "User-Agent",
            HeaderValue::from_str(&format!("mastodon-link-bot/{}", env!("CARGO_PKG_VERSION")))
                .unwrap(),
        );

        let client = Client::builder()
            .use_rustls_tls()
            .default_headers(headers)
            .build()?;

        let period = Duration::from_secs(5);

        Ok(ApiClient {
            client,
            instance,
            rate_limiter: MultiRateLimiter::new(period),
        })
    }

    pub async fn request(&self, method: Method, url: impl Into<String>) -> RequestBuilder {
        let mut url = url.into();
        if url.starts_with('/') {
            url = format!("{}{}", self.instance, url);
        }

        self.rate_limiter
            .throttle("api", || async { self.client.request(method, url) })
            .await
    }

    pub async fn get(&self, route: &str) -> RequestBuilder {
        self.request(Method::GET, route).await
    }

    pub async fn post(&self, route: &str) -> RequestBuilder {
        self.request(Method::POST, route).await
    }

    pub async fn delete(&self, route: &str) -> RequestBuilder {
        self.request(Method::DELETE, route).await
    }
}
