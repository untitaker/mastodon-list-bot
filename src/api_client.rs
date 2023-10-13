use anyhow::{Context, Error};
use backoff::future::retry_notify;
use backoff::ExponentialBackoff;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, Method, RequestBuilder, Response, StatusCode,
};

pub struct ApiClient {
    client: Client,
    instance: String,
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

        Ok(ApiClient { client, instance })
    }

    pub async fn request(
        &self,
        method: Method,
        url: impl Into<String>,
        builder_fn: RequestBuilderFunction,
    ) -> Result<Response, reqwest::Error> {
        let mut url = url.into();
        if url.starts_with('/') {
            url = format!("{}{}", self.instance, url);
        }

        retry_notify(
            ExponentialBackoff::default(),
            || async {
                let request_builder = self.client.request(method.clone(), url.clone());

                let response = builder_fn(request_builder)
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
                log::warn!("encountered rate limit, backing off for {:?}", dur);
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

type RequestBuilderFunction = Box<dyn Fn(RequestBuilder) -> RequestBuilder>;
