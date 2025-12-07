use crate::imp::{core::*, prelude::*};
use std::path::Path;

#[derive(Debug)]
pub(crate) struct Tracing {
    channel: ChannelOwner,
}

impl Tracing {
    pub(crate) fn try_new(channel: ChannelOwner) -> Result<Self, Error> {
        Ok(Self { channel })
    }

    pub(crate) async fn start(&self, args: StartArgs<'_, '_>) -> ArcResult<()> {
        let name = args.name;
        let title = args.title;
        let _ = send_message!(self, "tracingStart", &args);
        let _ = send_message!(self, "tracingStartChunk", StartChunkArgs { name, title });
        Ok(())
    }

    pub(crate) async fn start_chunk(&self, args: StartChunkArgs<'_, '_>) -> ArcResult<()> {
        let _ = send_message!(self, "tracingStartChunk", args);
        Ok(())
    }

    pub(crate) async fn group(&self, name: &str, location: Option<&str>) -> ArcResult<()> {
        #[skip_serializing_none]
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            name: &'a str,
            #[serde(rename = "location")]
            location: Option<&'a str>,
        }
        let args = Args { name, location };
        let _ = send_message!(self, "tracingGroup", args);
        Ok(())
    }

    pub(crate) async fn group_end(&self) -> ArcResult<()> {
        let _ = send_message!(self, "tracingGroupEnd", Map::new());
        Ok(())
    }

    pub(crate) async fn stop_chunk(&self, path: Option<&Path>) -> ArcResult<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            mode: &'a str,
        }
        let mode = if path.is_some() { "archive" } else { "discard" };
        let v = send_message!(self, "tracingStopChunk", Args { mode });
        if let Some(path) = path {
            if let Some(artifact) = v.get("artifact") {
                let guid = only_guid(artifact)?;
                let artifact = get_object!(self.context()?.lock().unwrap(), guid, Artifact)?;
                let artifact = upgrade(&artifact)?;
                artifact.save_as(path).await?;
                let _ = artifact.delete().await;
            }
        }
        Ok(())
    }

    pub(crate) async fn stop(&self, path: Option<&Path>) -> ArcResult<()> {
        self.stop_chunk(path).await?;
        let _ = send_message!(self, "tracingStop", Map::new());
        Ok(())
    }
}

#[skip_serializing_none]
#[derive(Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartArgs<'a, 'b> {
    pub name: Option<&'a str>,
    pub title: Option<&'b str>,
    pub screenshots: Option<bool>,
    pub snapshots: Option<bool>,
    pub sources: Option<bool>,
}

#[skip_serializing_none]
#[derive(Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartChunkArgs<'a, 'b> {
    pub name: Option<&'a str>,
    pub title: Option<&'b str>,
}

impl RemoteObject for Tracing {
    fn channel(&self) -> &ChannelOwner {
        &self.channel
    }
    fn channel_mut(&mut self) -> &mut ChannelOwner {
        &mut self.channel
    }
}
