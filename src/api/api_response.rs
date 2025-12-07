use crate::{
    api::api_request_context::APIRequestContext,
    imp::{api_request_context::APIResponsePayload, core::*, prelude::*, utils::Header},
    Error,
};
use serde::de::DeserializeOwned;

/// Response returned from APIRequestContext.fetch().
#[derive(Clone)]
pub struct APIResponse {
    ctx: APIRequestContext,
    payload: APIResponsePayload,
}

impl APIResponse {
    pub(crate) fn new(ctx: APIRequestContext, payload: APIResponsePayload) -> Self {
        Self { ctx, payload }
    }

    pub fn status(&self) -> i32 {
        self.payload.status
    }
    pub fn status_text(&self) -> &str {
        &self.payload.status_text
    }
    pub fn url(&self) -> &str {
        &self.payload.url
    }

    pub fn headers(&self) -> Vec<(String, String)> {
        self.payload
            .headers
            .iter()
            .cloned()
            .map(Into::into)
            .collect()
    }

    pub fn headers_array(&self) -> Vec<Header> {
        self.payload.headers.clone()
    }

    pub fn ok(&self) -> bool {
        let s = self.payload.status;
        s == 0 || (200..=299).contains(&s)
    }

    pub async fn body(&self) -> Result<Vec<u8>, Arc<Error>> {
        upgrade(&self.ctx.inner)?
            .fetch_response_body(&self.payload.fetch_uid)
            .await
    }

    pub async fn text(&self) -> Result<String, Arc<Error>> {
        let bytes = self.body().await?;
        String::from_utf8(bytes).map_err(|e| Arc::new(Error::InvalidUtf8(e)))
    }

    pub async fn json<T>(&self) -> Result<T, Arc<Error>>
    where
        T: DeserializeOwned,
    {
        let bytes = self.body().await?;
        serde_json::from_slice(&bytes).map_err(|e| Arc::new(Error::Serde(e)))
    }

    pub async fn dispose(&self) -> Result<(), Arc<Error>> {
        upgrade(&self.ctx.inner)?
            .dispose_api_response(&self.payload.fetch_uid)
            .await
    }
}
