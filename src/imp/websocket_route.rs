use crate::imp::{
    core::*,
    prelude::*,
    websocket::Buffer
};
use base64::{engine::general_purpose, Engine as _};

#[derive(Debug)]
pub(crate) struct WebSocketRoute {
    channel: ChannelOwner,
    url: String,
    var: Mutex<Variable>,
    tx: Mutex<Option<broadcast::Sender<Evt>>>
}

#[derive(Debug, Default)]
struct Variable {
    connected: bool
}

#[derive(Debug, Clone)]
pub(crate) enum Evt {
    MessageFromPage(Buffer),
    MessageFromServer(Buffer),
    CloseFromPage {
        code: i32,
        reason: String,
        was_clean: bool
    },
    CloseFromServer {
        code: i32,
        reason: String,
        was_clean: bool
    }
}

impl WebSocketRoute {
    pub(crate) fn try_new(channel: ChannelOwner) -> Result<Self, Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Init {
            url: String
        }
        let Init { url } = serde_json::from_value(channel.initializer.clone())?;
        Ok(Self {
            channel,
            url,
            var: Mutex::default(),
            tx: Mutex::default()
        })
    }

    pub(crate) fn url(&self) -> &str { &self.url }

    pub(crate) async fn connect_to_server(&self) -> ArcResult<()> {
        {
            let mut var = self.var.lock().unwrap();
            if var.connected {
                return Err(Arc::new(Error::InvalidParams));
            }
            var.connected = true;
        }
        let _ = send_message!(self, "connect", Map::new());
        Ok(())
    }

    pub(crate) async fn send_to_page_text(&self, message: &str) -> ArcResult<()> {
        let mut args = Map::new();
        args.insert("message".into(), message.into());
        args.insert("isBase64".into(), false.into());
        let _ = send_message!(self, "sendToPage", args);
        Ok(())
    }

    pub(crate) async fn send_to_page_bytes(&self, bytes: &[u8]) -> ArcResult<()> {
        let base64 = general_purpose::STANDARD.encode(bytes);
        let mut args = Map::new();
        args.insert("message".into(), base64.into());
        args.insert("isBase64".into(), true.into());
        let _ = send_message!(self, "sendToPage", args);
        Ok(())
    }

    pub(crate) async fn send_to_server_text(&self, message: &str) -> ArcResult<()> {
        let mut args = Map::new();
        args.insert("message".into(), message.into());
        args.insert("isBase64".into(), false.into());
        let _ = send_message!(self, "sendToServer", args);
        Ok(())
    }

    pub(crate) async fn send_to_server_bytes(&self, bytes: &[u8]) -> ArcResult<()> {
        let base64 = general_purpose::STANDARD.encode(bytes);
        let mut args = Map::new();
        args.insert("message".into(), base64.into());
        args.insert("isBase64".into(), true.into());
        let _ = send_message!(self, "sendToServer", args);
        Ok(())
    }

    pub(crate) async fn close_page(&self, code: Option<i32>, reason: Option<&str>) -> ArcResult<()> {
        #[skip_serializing_none]
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            code: Option<i32>,
            reason: Option<&'a str>,
            was_clean: bool
        }
        let args = Args {
            code,
            reason,
            was_clean: true
        };
        let _ = send_message!(self, "closePage", args);
        Ok(())
    }

    pub(crate) async fn close_server(
        &self,
        code: Option<i32>,
        reason: Option<&str>
    ) -> ArcResult<()> {
        #[skip_serializing_none]
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            code: Option<i32>,
            reason: Option<&'a str>,
            was_clean: bool
        }
        let args = Args {
            code,
            reason,
            was_clean: true
        };
        let _ = send_message!(self, "closeServer", args);
        Ok(())
    }

    fn emit(&self, evt: Evt) { self.emit_event(evt); }
}

impl RemoteObject for WebSocketRoute {
    fn channel(&self) -> &ChannelOwner { &self.channel }
    fn channel_mut(&mut self) -> &mut ChannelOwner { &mut self.channel }

    fn handle_event(
        &self,
        _ctx: &Context,
        method: Str<Method>,
        params: Map<String, Value>
    ) -> Result<(), Error> {
        match method.as_str() {
            "messageFromPage" => {
                let message = params.get("message").and_then(|v| v.as_str()).unwrap_or_default();
                let is_base64 = params.get("isBase64").and_then(|v| v.as_bool()).unwrap_or(false);
                let buffer = if is_base64 {
                    let bytes =
                        general_purpose::STANDARD.decode(message).map_err(Error::InvalidBase64)?;
                    Buffer::Bytes(bytes)
                } else {
                    Buffer::String(message.to_owned())
                };
                self.emit(Evt::MessageFromPage(buffer));
            }
            "messageFromServer" => {
                let message = params.get("message").and_then(|v| v.as_str()).unwrap_or_default();
                let is_base64 = params.get("isBase64").and_then(|v| v.as_bool()).unwrap_or(false);
                let buffer = if is_base64 {
                    let bytes =
                        general_purpose::STANDARD.decode(message).map_err(Error::InvalidBase64)?;
                    Buffer::Bytes(bytes)
                } else {
                    Buffer::String(message.to_owned())
                };
                self.emit(Evt::MessageFromServer(buffer));
            }
            "closePage" => {
                let code = params.get("code").and_then(|v| v.as_i64()).unwrap_or(1005) as i32;
                let reason = params
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned();
                let was_clean = params.get("wasClean").and_then(|v| v.as_bool()).unwrap_or(true);
                self.emit(Evt::CloseFromPage {
                    code,
                    reason,
                    was_clean
                });
            }
            "closeServer" => {
                let code = params.get("code").and_then(|v| v.as_i64()).unwrap_or(1005) as i32;
                let reason = params
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned();
                let was_clean = params.get("wasClean").and_then(|v| v.as_bool()).unwrap_or(true);
                self.emit(Evt::CloseFromServer {
                    code,
                    reason,
                    was_clean
                });
            }
            _ => {}
        }
        Ok(())
    }
}

impl EventEmitter for WebSocketRoute {
    type Event = Evt;

    fn tx(&self) -> Option<broadcast::Sender<Self::Event>> { self.tx.lock().unwrap().clone() }

    fn set_tx(&self, tx: broadcast::Sender<Self::Event>) { *self.tx.lock().unwrap() = Some(tx); }
}
