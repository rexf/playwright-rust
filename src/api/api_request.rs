use crate::{
    api::{api_request_context::NewContextOptions, api_request_context::APIRequestContext},
    imp::{
        api_request_context::NewContextArgs,
        core::*,
        playwright::Playwright as Impl,
        prelude::*
    },
    Error
};

/// Entry point for Web API testing (Playwright.request()).
#[derive(Clone)]
pub struct APIRequest {
    inner: Weak<Impl>
}

impl APIRequest {
    pub(crate) fn new(inner: Weak<Impl>) -> Self { Self { inner } }

    /// Creates a new isolated APIRequestContext.
    pub async fn new_context(
        &self,
        options: Option<NewContextOptions>
    ) -> Result<APIRequestContext, Arc<Error>> {
        let opts = options.unwrap_or_default();
        let args: NewContextArgs = opts.into();
        let inner = upgrade::<Impl>(&self.inner)?
            .new_api_request_context(args)
            .await?;
        Ok(APIRequestContext::new(inner))
    }
}
