use crate::imp::{artifact::Artifact as Impl, core::*, prelude::*};
use std::path::Path;

/// Artifact represents a file produced by Playwright (e.g. traces, videos, HARs).
#[derive(Clone)]
pub struct Artifact {
    inner: Weak<Impl>,
}

impl PartialEq for Artifact {
    fn eq(&self, other: &Self) -> bool {
        let a = self.inner.upgrade();
        let b = other.inner.upgrade();
        a.and_then(|a| b.map(|b| (a, b)))
            .map(|(a, b)| a.guid() == b.guid())
            .unwrap_or_default()
    }
}

impl Artifact {
    pub(crate) fn new(inner: Weak<Impl>) -> Self {
        Self { inner }
    }

    /// Path on disk once the artifact is finished. Returns `None` for remote artifacts.
    pub async fn path_after_finished(&self) -> ArcResult<Option<std::path::PathBuf>> {
        upgrade(&self.inner)?.path_after_finished().await
    }

    /// Save the artifact to the given path.
    pub async fn save_as<P: AsRef<Path>>(&self, path: P) -> ArcResult<()> {
        upgrade(&self.inner)?.save_as(path).await
    }

    /// Delete the artifact file.
    pub async fn delete(&self) -> ArcResult<()> {
        upgrade(&self.inner)?.delete().await
    }

    /// If artifact creation failed, returns an error string.
    pub async fn failure(&self) -> ArcResult<Option<String>> {
        upgrade(&self.inner)?.failure().await
    }
}
