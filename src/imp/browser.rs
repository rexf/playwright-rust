use crate::imp::{
    browser_context::BrowserContext,
    browser_type::{RecordHar, RecordVideo},
    core::*,
    prelude::*,
    utils::{ColorScheme, Geolocation, HttpCredentials, ProxySettings, StorageState, Viewport},
    artifact::Artifact
};
use tokio::sync::oneshot;

#[derive(Debug)]
pub(crate) struct Browser {
    channel: ChannelOwner,
    version: String,
    var: Mutex<Variable>
}

#[derive(Debug)]
enum Either<R, C> {
    Result(R),
    Context(C)
}

#[derive(Debug, Default)]
pub(crate) struct Variable {
    contexts: Vec<Weak<BrowserContext>>,
    is_remote: bool,
    pending_context: Option<oneshot::Sender<Weak<BrowserContext>>>
}

impl Browser {
    pub(crate) fn try_new(channel: ChannelOwner) -> Result<Self, Error> {
        let Initializer { version } = serde_json::from_value(channel.initializer.clone())?;
        Ok(Self {
            channel,
            version,
            var: Mutex::new(Variable {
                contexts: Vec::new(),
                is_remote: false,
                pending_context: None
            })
        })
    }
    pub(crate) fn version(&self) -> &str { &self.version }

    pub(crate) async fn close(&self) -> Result<(), Arc<Error>> {
        let _ = send_message!(self, "close", Map::new());
        Ok(())
    }

    // Responds newtype `OwnerPage` of `SinglePageBrowserContext`.
    // There are different behavior in BrowserContext::new_page
    // async fn new_page(
}

// mutable
impl Browser {
    pub(crate) fn contexts(&self) -> Vec<Weak<BrowserContext>> {
        self.var.lock().unwrap().contexts.to_owned()
    }

    pub(crate) fn push_context(&self, c: Weak<BrowserContext>) {
        let mut lock = self.var.lock().unwrap();
        lock.contexts.push(c);
        log::debug!("browser.push_context -> total {}", lock.contexts.len());
    }

    pub(crate) fn take_pending_context_sender(
        &self
    ) -> Option<oneshot::Sender<Weak<BrowserContext>>> {
        self.var.lock().unwrap().pending_context.take()
    }

    pub(crate) fn set_pending_context_sender(
        &self,
        tx: oneshot::Sender<Weak<BrowserContext>>
    ) {
        self.var.lock().unwrap().pending_context = Some(tx);
    }

    pub(super) fn remove_context(&self, c: &Weak<BrowserContext>) {
        let contexts = &mut self.var.lock().unwrap().contexts;
        contexts.remove_one(|v| v.ptr_eq(c));
    }

    pub(crate) fn is_remote(&self) -> bool { self.var.lock().unwrap().is_remote }

    pub(crate) fn set_is_remote_true(&self) { self.var.lock().unwrap().is_remote = true; }

    pub(crate) async fn new_context(
        &self,
        args: NewContextArgs<'_, '_, '_, '_, '_, '_, '_>
    ) -> Result<Weak<BrowserContext>, Arc<Error>> {
        use tokio::{select, time::{timeout, Duration}};

        // Track existing contexts so we can fall back to the newly created one even if
        // the protocol never delivers a `result` response (observed with newer drivers).
        let existing = self.contexts();

        // Manually send the request so we can time it out.
        let req = self
            .channel()
            .create_request(Str::validate("newContext".into()).unwrap())
            .set_args(args)?;
        let fut = self.channel().send_message(req).await?;

        // Listen for a BrowserContext __create__ event in parallel with the protocol
        // response so we can return promptly even if the driver never sends a result.
        let (tx, rx) = oneshot::channel::<Weak<BrowserContext>>();
        self.set_pending_context_sender(tx);

        let outcome = timeout(Duration::from_secs(30), async {
            select! {
                res = fut => Either::Result(res),
                ctx = rx => Either::Context(ctx),
            }
        })
        .await;

        // Ensure the pending sender is cleared regardless of how we exit.
        self.var.lock().unwrap().pending_context = None;

        match outcome {
            Ok(Either::Result(res)) => {
                let res = res?;
                let res = res.map_err(Error::ErrorResponded)?;
                let guid = only_guid(&*res)?;
                let c = get_object!(self.context()?.lock().unwrap(), guid, BrowserContext)?;
                self.register_new_context(c.clone())?;
                log::debug!("new_context resolved with guid {}", guid.as_str());
                Ok(c)
            }
            Ok(Either::Context(ctx)) => {
                match ctx {
                    Ok(c) => {
                        self.register_new_context(c.clone())?;
                        log::debug!("new_context resolved via __create__ event");
                        Ok(c)
                    }
                    Err(_) => {
                        // Sender dropped; fall through to the time-based fallbacks.
                        self.fallback_find_context(existing)
                    }
                }
            }
            Err(_) => {
                // Timeout: try to find a newly created context from the __create__ events.
                self.fallback_find_context(existing)
            }
        }
    }

    fn register_new_context(&self, c: Weak<BrowserContext>) -> Result<(), Arc<Error>> {
        self.push_context(c);
        // TODO: options
        // let this = get_object!(self.context()?.lock().unwrap(), &self.guid(), Browser)?;
        // let bc = upgrade(&c)?;
        // bc._options = params
        Ok(())
    }

    fn fallback_find_context(
        &self,
        existing: Vec<Weak<BrowserContext>>
    ) -> Result<Weak<BrowserContext>, Arc<Error>> {
        // First, try the contexts vector that tracks registrations.
        let after = self.contexts();
        log::warn!(
            "new_context timeout; contexts before={}, after={}",
            existing.len(),
            after.len()
        );
        if let Some(new_ctx) = after
            .iter()
            .find(|ctx| !existing.iter().any(|old| old.ptr_eq(ctx)))
        {
            self.register_new_context(new_ctx.clone())?;
            return Ok(new_ctx.clone());
        }

        // Next, inspect the browser's children added via __create__ events.
        let children = self.channel().children();
        log::debug!("new_context fallback scanning {} children", children.len());
        for child in children.into_iter().rev() {
            if let Some(RemoteArc::BrowserContext(ctx_arc)) = child.upgrade() {
                let weak = Arc::downgrade(&ctx_arc);
                self.register_new_context(weak.clone())?;
                return Ok(weak);
            }
        }

        // Finally, scan the raw connection object table for any BrowserContext whose
        // parent is this browser.
        if let Ok(ctx) = self.context() {
            let objs = ctx.lock().unwrap().list_objects();
            for obj in objs {
                if let RemoteArc::BrowserContext(bc) = obj {
                    if let Some(RemoteWeak::Browser(parent)) = bc.channel().parent.as_ref() {
                        if let Some(parent_browser) = parent.upgrade() {
                            if parent_browser.guid() == self.guid() {
                                let weak = Arc::downgrade(&bc);
                                self.register_new_context(weak.clone())?;
                                return Ok(weak);
                            }
                        }
                    }
                }
            }
        }

        Err(Arc::new(Error::Timeout))
    }
}

impl RemoteObject for Browser {
    fn channel(&self) -> &ChannelOwner { &self.channel }
    fn channel_mut(&mut self) -> &mut ChannelOwner { &mut self.channel }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Initializer {
    version: String
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NewContextArgs<'e, 'f, 'g, 'h, 'i, 'j, 'k> {
    sdk_language: &'static str,

    pub(crate) proxy: Option<ProxySettings>,

    pub(crate) viewport: Option<Option<Viewport>>,
    pub(crate) screen: Option<Viewport>,
    pub(crate) no_viewport: Option<bool>,
    #[serde(rename = "ignoreHTTPSErrors")]
    pub(crate) ignore_https_errors: Option<bool>,
    #[serde(rename = "javaScriptEnabled")]
    pub(crate) js_enabled: Option<bool>,
    #[serde(rename = "bypassCSP")]
    pub(crate) bypass_csp: Option<bool>,
    pub(crate) user_agent: Option<&'e str>,
    pub(crate) locale: Option<&'f str>,
    pub(crate) timezone_id: Option<&'g str>,
    pub(crate) geolocation: Option<Geolocation>,
    pub(crate) permissions: Option<&'h [String]>,
    #[serde(rename = "extraHTTPHeaders")]
    pub(crate) extra_http_headers: Option<HashMap<String, String>>,
    pub(crate) offline: Option<bool>,
    pub(crate) http_credentials: Option<&'i HttpCredentials>,
    pub(crate) device_scale_factor: Option<f64>,
    pub(crate) is_mobile: Option<bool>,
    pub(crate) has_touch: Option<bool>,
    pub(crate) color_scheme: Option<ColorScheme>,
    pub(crate) accept_downloads: Option<bool>,
    pub(crate) chromium_sandbox: Option<bool>,
    pub(crate) record_video: Option<RecordVideo<'j>>,
    pub(crate) record_har: Option<RecordHar<'k>>,

    pub(crate) storage_state: Option<StorageState>
}

impl<'e, 'f, 'g, 'h, 'i, 'j, 'k> Default for NewContextArgs<'e, 'f, 'g, 'h, 'i, 'j, 'k> {
    fn default() -> Self {
        Self {
            sdk_language: "javascript",
            proxy: None,
            viewport: None,
            screen: None,
            no_viewport: None,
            ignore_https_errors: None,
            js_enabled: None,
            bypass_csp: None,
            user_agent: None,
            locale: None,
            timezone_id: None,
            geolocation: None,
            permissions: None,
            extra_http_headers: None,
            offline: None,
            http_credentials: None,
            device_scale_factor: None,
            is_mobile: None,
            has_touch: None,
            color_scheme: None,
            accept_downloads: None,
            chromium_sandbox: None,
            record_video: None,
            record_har: None,
            storage_state: None
        }
    }
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartTracingArgs<'a> {
    pub(crate) page: Option<Str<Guid>>,
    pub(crate) path: Option<&'a str>,
    pub(crate) screenshots: Option<bool>,
    pub(crate) categories: Option<Vec<&'a str>>
}

impl Browser {
    pub(crate) async fn start_tracing(&self, args: StartTracingArgs<'_>) -> ArcResult<()> {
        let _ = send_message!(self, "startTracing", args);
        Ok(())
    }

    pub(crate) async fn stop_tracing(&self) -> ArcResult<Weak<Artifact>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Res {
            artifact: OnlyGuid
        }
        let v = send_message!(self, "stopTracing", Map::new());
        let Res {
            artifact: OnlyGuid { guid }
        } = serde_json::from_value((*v).clone()).map_err(Error::Serde)?;
        let artifact = get_object!(self.context()?.lock().unwrap(), &guid, Artifact)?;
        Ok(artifact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imp::{browser_type::*, playwright::Playwright};

    crate::runtime_test!(new_context, {
        let driver = Driver::install().unwrap();
        let conn = Connection::run(&driver.executable()).unwrap();
        let p = Playwright::wait_initial_object(&conn).await.unwrap();
        let p = p.upgrade().unwrap();
        let chromium = p.chromium().upgrade().unwrap();
        let b = chromium.launch(LaunchArgs::default()).await.unwrap();
        let b = b.upgrade().unwrap();
        b.new_context(NewContextArgs::default()).await.unwrap();
    });
}
