use crate::imp::{core::*, prelude::*};

/// Low-level Chrome DevTools Protocol session.
#[derive(Debug)]
pub(crate) struct CDPSession {
    channel: ChannelOwner,
    tx: Mutex<Option<broadcast::Sender<Evt>>>
}

#[derive(Debug, Clone)]
pub(crate) struct Evt {
    pub method: String,
    pub params: Option<Value>
}

#[skip_serializing_none]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SendArgs<'a> {
    method: &'a str,
    params: Option<Value>
}

impl EventEmitter for CDPSession {
    type Event = Evt;

    fn tx(&self) -> Option<broadcast::Sender<Self::Event>> { self.tx.lock().unwrap().clone() }

    fn set_tx(&self, tx: broadcast::Sender<Self::Event>) { *self.tx.lock().unwrap() = Some(tx); }
}

impl IsEvent for Evt {
    type EventType = ();

    fn event_type(&self) -> Self::EventType { () }
}

impl CDPSession {
    pub(crate) fn try_new(channel: ChannelOwner) -> Result<Self, Error> {
        Ok(Self {
            channel,
            tx: Mutex::default()
        })
    }

    pub(crate) async fn send(
        &self,
        method: &str,
        params: Option<Value>
    ) -> ArcResult<Option<Value>> {
        let args = SendArgs { method, params };
        let v = send_message!(self, "send", args);
        if let Some(result) = v.get("result") {
            return Ok(Some(result.clone()));
        }
        Ok(None)
    }

    pub(crate) async fn detach(&self) -> ArcResult<()> {
        let _ = send_message!(self, "detach", Map::new());
        Ok(())
    }
}

impl RemoteObject for CDPSession {
    fn channel(&self) -> &ChannelOwner { &self.channel }
    fn channel_mut(&mut self) -> &mut ChannelOwner { &mut self.channel }

    fn handle_event(
        &self,
        _ctx: &Context,
        method: Str<Method>,
        params: Map<String, Value>
    ) -> Result<(), Error> {
        if method.as_str() == "event" {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct De {
                method: String,
                params: Option<Value>
            }
            let De { method, params } = serde_json::from_value(params.into())?;
            self.emit_event(Evt { method, params });
        }
        Ok(())
    }
}
