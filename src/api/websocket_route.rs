use crate::imp::{
    core::*,
    prelude::*,
    websocket::Buffer,
    websocket_route::{Evt as ImplEvt, WebSocketRoute as Impl},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Page,
    Server,
}

#[derive(Clone)]
pub struct WebSocketRoute {
    inner: Weak<Impl>,
    side: Side,
}

impl PartialEq for WebSocketRoute {
    fn eq(&self, other: &Self) -> bool {
        let a = self.inner.upgrade();
        let b = other.inner.upgrade();
        a.and_then(|a| b.map(|b| (a, b)))
            .map(|(a, b)| a.guid() == b.guid() && self.side == other.side)
            .unwrap_or_default()
    }
}

impl WebSocketRoute {
    pub(crate) fn new(inner: Weak<Impl>, side: Side) -> Self {
        Self { inner, side }
    }

    pub fn url(&self) -> Result<String, Error> {
        Ok(upgrade(&self.inner)?.url().to_owned())
    }

    pub async fn connect_to_server(&self) -> ArcResult<WebSocketRoute> {
        if self.side == Side::Server {
            return Err(Arc::new(Error::InvalidParams));
        }
        upgrade(&self.inner)?.connect_to_server().await?;
        Ok(WebSocketRoute::new(self.inner.clone(), Side::Server))
    }

    pub async fn send_text(&self, message: &str) -> ArcResult<()> {
        let inner = upgrade(&self.inner)?;
        match self.side {
            Side::Page => inner.send_to_page_text(message).await,
            Side::Server => inner.send_to_server_text(message).await,
        }
    }

    pub async fn send_bytes(&self, bytes: &[u8]) -> ArcResult<()> {
        let inner = upgrade(&self.inner)?;
        match self.side {
            Side::Page => inner.send_to_page_bytes(bytes).await,
            Side::Server => inner.send_to_server_bytes(bytes).await,
        }
    }

    pub async fn close(&self, code: Option<i32>, reason: Option<&str>) -> ArcResult<()> {
        let inner = upgrade(&self.inner)?;
        match self.side {
            Side::Page => inner.close_page(code, reason).await,
            Side::Server => inner.close_server(code, reason).await,
        }
    }

    subscribe_event! {}
}

#[derive(Debug, Clone)]
pub enum Event {
    MessageFromPage(Buffer),
    MessageFromServer(Buffer),
    CloseFromPage {
        code: i32,
        reason: String,
        was_clean: bool,
    },
    CloseFromServer {
        code: i32,
        reason: String,
        was_clean: bool,
    },
}

impl From<ImplEvt> for Event {
    fn from(e: ImplEvt) -> Self {
        match e {
            ImplEvt::MessageFromPage(b) => Event::MessageFromPage(b),
            ImplEvt::MessageFromServer(b) => Event::MessageFromServer(b),
            ImplEvt::CloseFromPage {
                code,
                reason,
                was_clean,
            } => Event::CloseFromPage {
                code,
                reason,
                was_clean,
            },
            ImplEvt::CloseFromServer {
                code,
                reason,
                was_clean,
            } => Event::CloseFromServer {
                code,
                reason,
                was_clean,
            },
        }
    }
}
