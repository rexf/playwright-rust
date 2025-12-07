use crate::imp::{
    cdp_session::{CDPSession as Impl, Evt},
    core::*,
    prelude::*,
};

#[derive(Clone)]
pub struct CDPSession {
    inner: Weak<Impl>,
}

impl PartialEq for CDPSession {
    fn eq(&self, other: &Self) -> bool {
        let a = self.inner.upgrade();
        let b = other.inner.upgrade();
        a.and_then(|a| b.map(|b| (a, b)))
            .map(|(a, b)| a.guid() == b.guid())
            .unwrap_or_default()
    }
}

impl CDPSession {
    pub(crate) fn new(inner: Weak<Impl>) -> Self {
        Self { inner }
    }

    /// Sends a raw Chrome DevTools Protocol command.
    pub async fn send(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<Option<serde_json::Value>, Arc<Error>> {
        upgrade(&self.inner)?.send(method, params).await
    }

    /// Detaches the session from its target.
    pub async fn detach(&self) -> ArcResult<()> {
        upgrade(&self.inner)?.detach().await
    }

    subscribe_event! {}
}

#[derive(Debug, Clone)]
pub struct Event {
    pub method: String,
    pub params: Option<serde_json::Value>,
}

impl From<Evt> for Event {
    fn from(Evt { method, params }: Evt) -> Self {
        Self { method, params }
    }
}
