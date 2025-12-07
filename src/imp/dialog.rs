use crate::imp::{core::*, prelude::*};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Initializer {
    r#type: String,
    message: String,
    default_value: String,
}

#[derive(Debug)]
pub(crate) struct Dialog {
    channel: ChannelOwner,
    typ: String,
    message: String,
    default_value: String,
}

impl Dialog {
    pub(crate) fn new(channel: ChannelOwner) -> Self {
        let Initializer {
            r#type,
            message,
            default_value,
        } = serde_json::from_value(channel.initializer.clone())
            .unwrap_or(Initializer {
                r#type: String::new(),
                message: String::new(),
                default_value: String::new(),
            });
        Self {
            channel,
            typ: r#type,
            message,
            default_value,
        }
    }

    pub(crate) fn r#type(&self) -> &str {
        &self.typ
    }
    pub(crate) fn message(&self) -> &str {
        &self.message
    }
    pub(crate) fn default_value(&self) -> &str {
        &self.default_value
    }

    pub(crate) async fn accept(&self, prompt_text: Option<&str>) -> ArcResult<()> {
        #[skip_serializing_none]
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            prompt_text: Option<&'a str>,
        }
        let args = Args { prompt_text };
        let _ = send_message!(self, "accept", args);
        Ok(())
    }

    pub(crate) async fn dismiss(&self) -> ArcResult<()> {
        let _ = send_message!(self, "dismiss", Map::new());
        Ok(())
    }
}

impl RemoteObject for Dialog {
    fn channel(&self) -> &ChannelOwner {
        &self.channel
    }
    fn channel_mut(&mut self) -> &mut ChannelOwner {
        &mut self.channel
    }
}
