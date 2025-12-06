use std::path::Path;

use crate::imp::{
    core::*,
    prelude::*,
    tracing::{StartArgs, StartChunkArgs, Tracing as Impl}
};

#[derive(Clone)]
pub struct Tracing {
    inner: Weak<Impl>
}

impl PartialEq for Tracing {
    fn eq(&self, other: &Self) -> bool {
        let a = self.inner.upgrade();
        let b = other.inner.upgrade();
        a.and_then(|a| b.map(|b| (a, b)))
            .map(|(a, b)| a.guid() == b.guid())
            .unwrap_or_default()
    }
}

impl Tracing {
    pub(crate) fn new(inner: Weak<Impl>) -> Self { Self { inner } }

    pub async fn start(&self, options: StartOptions<'_, '_>) -> ArcResult<()> {
        upgrade(&self.inner)?.start(options.into()).await
    }

    pub async fn start_chunk(&self, options: StartChunkOptions<'_, '_>) -> ArcResult<()> {
        upgrade(&self.inner)?.start_chunk(options.into()).await
    }

    pub async fn stop(&self, options: StopOptions<'_>) -> ArcResult<()> {
        upgrade(&self.inner)?.stop(options.path).await
    }

    pub async fn stop_chunk(&self, options: StopChunkOptions<'_>) -> ArcResult<()> {
        upgrade(&self.inner)?.stop_chunk(options.path).await
    }

    /// Group trace entries for better readability in the trace viewer.
    pub async fn group(&self, name: &str, location: Option<&str>) -> ArcResult<()> {
        upgrade(&self.inner)?.group(name, location).await
    }

    pub async fn group_end(&self) -> ArcResult<()> {
        upgrade(&self.inner)?.group_end().await
    }
}

#[derive(Default)]
pub struct StartOptions<'a, 'b> {
    pub name: Option<&'a str>,
    pub title: Option<&'b str>,
    pub screenshots: Option<bool>,
    pub snapshots: Option<bool>,
    pub sources: Option<bool>
}

impl<'a, 'b> From<StartOptions<'a, 'b>> for StartArgs<'a, 'b> {
    fn from(
        StartOptions {
            name,
            title,
            screenshots,
            snapshots,
            sources
        }: StartOptions<'a, 'b>
    ) -> Self {
        StartArgs {
            name,
            title,
            screenshots,
            snapshots,
            sources
        }
    }
}

#[derive(Default)]
pub struct StartChunkOptions<'a, 'b> {
    pub name: Option<&'a str>,
    pub title: Option<&'b str>
}

impl<'a, 'b> From<StartChunkOptions<'a, 'b>> for StartChunkArgs<'a, 'b> {
    fn from(StartChunkOptions { name, title }: StartChunkOptions<'a, 'b>) -> Self {
        StartChunkArgs { name, title }
    }
}

#[derive(Default)]
pub struct StopOptions<'a> {
    pub path: Option<&'a Path>
}

#[derive(Default)]
pub struct StopChunkOptions<'a> {
    pub path: Option<&'a Path>
}
