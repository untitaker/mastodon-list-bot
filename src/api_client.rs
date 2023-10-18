use std::sync::Arc;

use backoff::future::retry_notify;
use backoff::ExponentialBackoff;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, Method, RequestBuilder, Response, StatusCode,
};

use crate::error::ResponseError;

pub struct ApiClient {
    client: Client,
    host: String,
}

impl ApiClient {
    pub fn new(host: &str, token: &str) -> Result<Self, ResponseError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {}", token))?,
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

        Ok(ApiClient {
            client,
            host: host.to_owned(),
        })
    }

    pub async fn request(
        &self,
        method: Method,
        url: impl Into<String>,
        builder_fn: RequestBuilderFunction,
    ) -> Result<Response, reqwest::Error> {
        let mut url = url.into();
        if url.starts_with('/') {
            url = format!("https://{}{}", self.host, url);
        }

        let arc_builder_fn = Arc::new(builder_fn);

        retry_notify(
            ExponentialBackoff::default(),
            || async {
                let request_builder = self.client.request(method.clone(), url.clone());

                let response = (arc_builder_fn.clone())(request_builder)
                    .send()
                    .await
                    .map_err(backoff::Error::permanent)?;

                if response.status() == StatusCode::TOO_MANY_REQUESTS {
                    return Err(backoff::Error::transient(
                        response.error_for_status().unwrap_err(),
                    ));
                };

                Ok(response)
            },
            |_err, dur| {
                log::warn!("[{}] encountered rate limit, backing off for {:?}", self.host, dur);
            },
        )
        .await
    }

    pub async fn get(
        &self,
        route: &str,
        builder_fn: RequestBuilderFunction,
    ) -> Result<Response, reqwest::Error> {
        self.request(Method::GET, route, builder_fn).await
    }

    pub async fn post(
        &self,
        route: &str,
        builder_fn: RequestBuilderFunction,
    ) -> Result<Response, reqwest::Error> {
        self.request(Method::POST, route, builder_fn).await
    }

    pub async fn delete(
        &self,
        route: &str,
        builder_fn: RequestBuilderFunction,
    ) -> Result<Response, reqwest::Error> {
        self.request(Method::DELETE, route, builder_fn).await
    }
}

type RequestBuilderFunction = Box<dyn Send + Sync + Fn(RequestBuilder) -> RequestBuilder>;
