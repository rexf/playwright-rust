use crate::imp::{
    api_request_context::APIRequestContext,
    browser::Browser,
    cdp_session::CDPSession,
    console_message::ConsoleMessage,
    core::*,
    frame::Frame,
    page::Page,
    prelude::*,
    request::Request,
    response::Response,
    route::Route,
    tracing::Tracing,
    utils::{Cookie, Geolocation, Header, StorageState},
    web_error::WebError,
    websocket_route::WebSocketRoute,
};
use futures::future::BoxFuture;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;

pub(crate) type RouteHandler =
    Arc<dyn Fn(Arc<Route>) -> BoxFuture<'static, ()> + Send + Sync + 'static>;
pub(crate) type WebSocketRouteHandler =
    Arc<dyn Fn(Arc<WebSocketRoute>) -> BoxFuture<'static, ()> + Send + Sync + 'static>;
#[derive(Clone)]
enum RoutePattern {
    Glob(String),
    Regex(String, String), // source, flags
}

#[derive(Clone)]
enum WebSocketRoutePattern {
    Glob(String),
    Regex(String, String),
}

#[derive(Clone)]
struct RouteEntry {
    pattern: RoutePattern,
    handler: RouteHandler,
    times: Option<u32>,
}

#[derive(Clone)]
struct WebSocketRouteEntry {
    pattern: WebSocketRoutePattern,
    handler: WebSocketRouteHandler,
}
pub(crate) struct BrowserContext {
    channel: ChannelOwner,
    var: Mutex<Variable>,
    tx: Mutex<Option<broadcast::Sender<Evt>>>,
}

#[derive(Default)]
pub(crate) struct Variable {
    browser: Option<Weak<Browser>>,
    pages: Vec<Weak<Page>>,
    timeout: Option<u32>,
    navigation_timeout: Option<u32>,
    routes: Vec<RouteEntry>,
    websocket_routes: Vec<WebSocketRouteEntry>,
    tracing: Option<Weak<Tracing>>,
    request_context: Option<Weak<APIRequestContext>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SerializedError {
    error: Option<InnerError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InnerError {
    name: Option<String>,
    message: Option<String>,
    stack: Option<String>,
}

fn guid_from_keys(params: &Map<String, Value>, keys: &[&str]) -> Result<OnlyGuid, Error> {
    for key in keys {
        if let Some(v) = params.get(*key) {
            return serde_json::from_value(v.clone()).map_err(|_| Error::InvalidParams);
        }
    }

    if params.len() == 1 {
        if let Some(first) = first_object(params) {
            return serde_json::from_value((*first).clone()).map_err(|_| Error::InvalidParams);
        }
    }

    Err(Error::InvalidParams)
}

fn format_error_value(v: &Value) -> Result<String, Error> {
    let SerializedError { error } = serde_json::from_value(v.clone())?;
    if let Some(InnerError {
        name,
        message,
        stack,
    }) = error
    {
        let mut s = String::new();
        if let Some(name) = name {
            s.push_str(&name);
        }
        if let Some(message) = message {
            if !s.is_empty() {
                s.push_str(": ");
            }
            s.push_str(&message);
        }
        if let Some(stack) = stack {
            if !stack.is_empty() {
                s.push('\n');
                s.push_str(&stack);
            }
        }
        Ok(s)
    } else {
        Ok(String::new())
    }
}

impl BrowserContext {
    const DEFAULT_TIMEOUT: u32 = 30000;

    pub(crate) fn try_new(channel: ChannelOwner) -> Result<Self, Error> {
        log::trace!(
            "BrowserContext::try_new guid={} parent={:?}",
            channel.guid.as_str(),
            channel.parent.as_ref().map(|p| format!("{:?}", p))
        );
        let Initializer {
            tracing,
            request_context,
        } = serde_json::from_value(channel.initializer.clone())?;
        let browser = match &channel.parent {
            Some(RemoteWeak::Browser(b)) => Some(b.clone()),
            _ => None,
        };
        let var = Mutex::new(Variable {
            browser,
            tracing: tracing.and_then(|OnlyGuid { guid }| {
                let ctx = upgrade(&channel.ctx).ok()?;
                let ctx_locked = match ctx.try_lock() {
                    Ok(l) => l,
                    Err(_) => return None,
                };
                get_object!(ctx_locked, &guid, Tracing).ok()
            }),
            request_context: request_context.and_then(|OnlyGuid { guid }| {
                let ctx = upgrade(&channel.ctx).ok()?;
                let ctx_locked = match ctx.try_lock() {
                    Ok(l) => l,
                    Err(_) => return None,
                };
                get_object!(ctx_locked, &guid, APIRequestContext).ok()
            }),
            ..Variable::default()
        });
        let ctx = Self {
            channel,
            var,
            tx: Mutex::default(),
        };
        log::trace!(
            "BrowserContext::try_new completed guid={}",
            ctx.channel.guid.as_str()
        );
        Ok(ctx)
    }

    pub(crate) async fn new_page(&self) -> Result<Weak<Page>, Arc<Error>> {
        let res = send_message!(self, "newPage", Map::new());
        let guid = only_guid(&res)?;
        let p = get_object!(self.context()?.lock().unwrap(), guid, Page)?;
        Ok(p)
    }

    pub(crate) async fn new_cdp_session_with_page(
        &self,
        page: Weak<Page>,
    ) -> ArcResult<Weak<CDPSession>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            page: OnlyGuid,
        }
        let page = upgrade(&page)?;
        let args = Args {
            page: OnlyGuid {
                guid: page.guid().to_owned(),
            },
        };
        let res = send_message!(self, "newCDPSession", args);
        let session = res.get("session").ok_or(Error::InvalidParams)?;
        let guid = only_guid(session)?;
        let session = get_object!(self.context()?.lock().unwrap(), guid, CDPSession)?;
        Ok(session)
    }

    pub(crate) async fn new_cdp_session_with_frame(
        &self,
        frame: Weak<Frame>,
    ) -> ArcResult<Weak<CDPSession>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            frame: OnlyGuid,
        }
        let frame = upgrade(&frame)?;
        let args = Args {
            frame: OnlyGuid {
                guid: frame.guid().to_owned(),
            },
        };
        let res = send_message!(self, "newCDPSession", args);
        let session = res.get("session").ok_or(Error::InvalidParams)?;
        let guid = only_guid(session)?;
        let session = get_object!(self.context()?.lock().unwrap(), guid, CDPSession)?;
        Ok(session)
    }

    pub(crate) async fn close(&self) -> Result<(), Arc<Error>> {
        if let Some(rc) = self.request_context() {
            if let Some(rc) = rc.upgrade() {
                let _ = rc.dispose(None).await;
            }
        }
        let _ = send_message!(self, "close", Map::new());
        Ok(())
    }

    pub(crate) async fn storage_state(&self) -> ArcResult<StorageState> {
        let v = send_message!(self, "storageState", Map::new());
        let s = serde_json::from_value((*v).clone()).map_err(Error::Serde)?;
        Ok(s)
    }

    pub(crate) async fn clear_cookies(&self) -> ArcResult<()> {
        let _ = send_message!(self, "clearCookies", Map::new());
        Ok(())
    }

    pub(crate) async fn cookies(&self, urls: &[String]) -> ArcResult<Vec<Cookie>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            urls: &'a [String],
        }
        let args = Args { urls };
        let v = send_message!(self, "cookies", args);
        let cookies = first(&v).ok_or(Error::InvalidParams)?;
        let cs: Vec<Cookie> = serde_json::from_value((*cookies).clone()).map_err(Error::Serde)?;
        Ok(cs)
    }

    pub(crate) async fn add_cookies(&self, cookies: &[Cookie]) -> ArcResult<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            cookies: &'a [Cookie],
        }
        let args = Args { cookies };
        let _ = send_message!(self, "addCookies", args);
        Ok(())
    }

    pub(crate) async fn grant_permissions(
        &self,
        permissions: &[String],
        origin: Option<&str>,
    ) -> ArcResult<()> {
        #[skip_serializing_none]
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a, 'b> {
            permissions: &'a [String],
            origin: Option<&'b str>,
        }
        let args = Args {
            permissions,
            origin,
        };
        let _ = send_message!(self, "grantPermissions", args);
        Ok(())
    }

    pub(crate) async fn clear_permissions(&self) -> ArcResult<()> {
        let _ = send_message!(self, "clearPermissions", Map::new());
        Ok(())
    }

    pub(crate) async fn route(&self, glob: &str, handler: RouteHandler) -> ArcResult<()> {
        {
            let mut var = self.var.lock().unwrap();
            var.routes.push(RouteEntry {
                pattern: RoutePattern::Glob(glob.to_owned()),
                handler,
                times: None,
            });
        }
        let patterns = self.route_patterns();
        self.set_network_interception_patterns(&patterns).await
    }

    pub(crate) async fn route_regex(
        &self,
        regex_source: &str,
        regex_flags: &str,
        handler: RouteHandler,
    ) -> ArcResult<()> {
        {
            let mut var = self.var.lock().unwrap();
            var.routes.push(RouteEntry {
                pattern: RoutePattern::Regex(regex_source.to_owned(), regex_flags.to_owned()),
                handler,
                times: None,
            });
        }
        let patterns = self.route_patterns();
        self.set_network_interception_patterns(&patterns).await
    }

    pub(crate) async fn route_with_times_glob(
        &self,
        glob: &str,
        times: u32,
        handler: RouteHandler,
    ) -> ArcResult<()> {
        {
            let mut var = self.var.lock().unwrap();
            var.routes.push(RouteEntry {
                pattern: RoutePattern::Glob(glob.to_owned()),
                handler,
                times: Some(times),
            });
        }
        let patterns = self.route_patterns();
        self.set_network_interception_patterns(&patterns).await
    }

    pub(crate) async fn unroute(&self, glob: Option<&str>) -> ArcResult<()> {
        {
            let mut var = self.var.lock().unwrap();
            if let Some(g) = glob {
                var.routes.retain(|r| match &r.pattern {
                    RoutePattern::Glob(s) => s != g,
                    RoutePattern::Regex(src, _) => src != g,
                });
            } else {
                var.routes.clear();
            }
        }
        let patterns = self.route_patterns();
        self.set_network_interception_patterns(&patterns).await
    }

    pub(crate) async fn route_web_socket(
        &self,
        glob: &str,
        handler: WebSocketRouteHandler,
    ) -> ArcResult<()> {
        {
            let mut var = self.var.lock().unwrap();
            var.websocket_routes.push(WebSocketRouteEntry {
                pattern: WebSocketRoutePattern::Glob(glob.to_owned()),
                handler,
            });
        }
        let patterns = self.websocket_route_patterns();
        self.set_web_socket_interception_patterns(&patterns).await
    }

    pub(crate) async fn route_web_socket_regex(
        &self,
        regex_source: &str,
        regex_flags: &str,
        handler: WebSocketRouteHandler,
    ) -> ArcResult<()> {
        {
            let mut var = self.var.lock().unwrap();
            var.websocket_routes.push(WebSocketRouteEntry {
                pattern: WebSocketRoutePattern::Regex(
                    regex_source.to_owned(),
                    regex_flags.to_owned(),
                ),
                handler,
            });
        }
        let patterns = self.websocket_route_patterns();
        self.set_web_socket_interception_patterns(&patterns).await
    }

    pub(crate) async fn unroute_web_socket(&self, glob: Option<&str>) -> ArcResult<()> {
        {
            let mut var = self.var.lock().unwrap();
            if let Some(g) = glob {
                var.websocket_routes.retain(|r| match &r.pattern {
                    WebSocketRoutePattern::Glob(s) => s != g,
                    WebSocketRoutePattern::Regex(src, _) => src != g,
                });
            } else {
                var.websocket_routes.clear();
            }
        }
        let patterns = self.websocket_route_patterns();
        self.set_web_socket_interception_patterns(&patterns).await
    }

    pub(crate) async fn set_geolocation(&self, geolocation: Option<&Geolocation>) -> ArcResult<()> {
        #[skip_serializing_none]
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            geolocation: Option<&'a Geolocation>,
        }
        let args = Args { geolocation };
        let _ = send_message!(self, "setGeolocation", args);
        Ok(())
    }

    pub(crate) async fn set_offline(&self, offline: bool) -> ArcResult<()> {
        let mut args = Map::new();
        args.insert("offline".into(), offline.into());
        let _ = send_message!(self, "setOffline", args);
        Ok(())
    }

    pub(crate) async fn add_init_script(&self, script: &str) -> ArcResult<()> {
        let mut args = HashMap::new();
        args.insert("source", script);
        let _ = send_message!(self, "addInitScript", args);
        Ok(())
    }

    pub(crate) async fn set_extra_http_headers<T>(&self, headers: T) -> ArcResult<()>
    where
        T: IntoIterator<Item = (String, String)>,
    {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            headers: Vec<Header>,
        }
        let args = Args {
            headers: headers.into_iter().map(Header::from).collect(),
        };
        let _ = send_message!(self, "setExtraHTTPHeaders", args);
        Ok(())
    }

    // async def expose_binding(
    // async def expose_function(self, name: str, callback: Callable) -> None:
    // async def route(self, url: URLMatch, handler: RouteHandler) -> None:
    // async def unroute(

    // async fn pause(&self) -> ArcResult<()> {
    //    let _ = send_message!(self, "pause", Map::new());
    //    Ok(())
    //}
}

// mutable
impl BrowserContext {
    pub(crate) fn browser(&self) -> Option<Weak<Browser>> {
        self.var.lock().unwrap().browser.clone()
    }

    pub(crate) fn tracing(&self) -> Option<Weak<Tracing>> {
        self.var.lock().unwrap().tracing.clone()
    }

    pub(crate) fn request_context(&self) -> Option<Weak<APIRequestContext>> {
        self.var.lock().unwrap().request_context.clone()
    }

    pub(crate) fn set_browser(&self, browser: Weak<Browser>) {
        self.var.lock().unwrap().browser = Some(browser);
    }

    pub(crate) fn pages(&self) -> Vec<Weak<Page>> {
        self.var.lock().unwrap().pages.clone()
    }

    pub(super) fn push_page(&self, p: Weak<Page>) {
        self.var.lock().unwrap().pages.push(p);
    }

    pub(super) fn remove_page(&self, page: &Weak<Page>) {
        let pages = &mut self.var.lock().unwrap().pages;
        pages.remove_one(|p| p.ptr_eq(page));
    }

    pub(crate) fn default_timeout(&self) -> u32 {
        self.var
            .lock()
            .unwrap()
            .timeout
            .unwrap_or(Self::DEFAULT_TIMEOUT)
    }

    pub(crate) fn default_navigation_timeout(&self) -> u32 {
        self.var
            .lock()
            .unwrap()
            .navigation_timeout
            .unwrap_or(Self::DEFAULT_TIMEOUT)
    }

    pub(crate) async fn set_default_timeout(&self, timeout: u32) -> ArcResult<()> {
        let mut args = Map::new();
        args.insert("timeout".into(), timeout.into());
        let _ = send_message!(self, "setDefaultTimeoutNoReply", args);
        self.var.lock().unwrap().timeout = Some(timeout);
        Ok(())
    }

    pub(crate) async fn set_default_navigation_timeout(&self, timeout: u32) -> ArcResult<()> {
        let mut args = Map::new();
        args.insert("timeout".into(), timeout.into());
        let _ = send_message!(self, "setDefaultNavigationTimeoutNoReply", args);
        self.var.lock().unwrap().navigation_timeout = Some(timeout);
        Ok(())
    }

    fn route_patterns(&self) -> Vec<RoutePattern> {
        let mut patterns: Vec<RoutePattern> = self
            .var
            .lock()
            .unwrap()
            .routes
            .iter()
            .map(|r| r.pattern.clone())
            .collect();
        // dedup by serialized key
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        patterns.retain(|p| {
            let k = match p {
                RoutePattern::Glob(g) => format!("g:{}", g),
                RoutePattern::Regex(s, f) => format!("r:{}/{}", s, f),
            };
            seen.insert(k)
        });
        patterns
    }

    async fn set_network_interception_patterns(&self, patterns: &[RoutePattern]) -> ArcResult<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Pattern<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            glob: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            regex_source: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            regex_flags: Option<&'a str>,
        }
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            patterns: Vec<Pattern<'a>>,
        }
        let args = Args {
            patterns: patterns
                .iter()
                .map(|p| match p {
                    RoutePattern::Glob(g) => Pattern {
                        glob: Some(g.as_str()),
                        regex_source: None,
                        regex_flags: None,
                    },
                    RoutePattern::Regex(s, f) => Pattern {
                        glob: None,
                        regex_source: Some(s.as_str()),
                        regex_flags: Some(f.as_str()),
                    },
                })
                .collect(),
        };
        let _ = send_message!(self, "setNetworkInterceptionPatterns", args);
        Ok(())
    }

    fn websocket_route_patterns(&self) -> Vec<WebSocketRoutePattern> {
        let mut patterns: Vec<WebSocketRoutePattern> = self
            .var
            .lock()
            .unwrap()
            .websocket_routes
            .iter()
            .map(|r| r.pattern.clone())
            .collect();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        patterns.retain(|p| {
            let k = match p {
                WebSocketRoutePattern::Glob(g) => format!("g:{}", g),
                WebSocketRoutePattern::Regex(s, f) => format!("r:{}/{}", s, f),
            };
            seen.insert(k)
        });
        patterns
    }

    async fn set_web_socket_interception_patterns(
        &self,
        patterns: &[WebSocketRoutePattern],
    ) -> ArcResult<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Pattern<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            glob: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            regex_source: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            regex_flags: Option<&'a str>,
        }
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            patterns: Vec<Pattern<'a>>,
        }
        let args = Args {
            patterns: patterns
                .iter()
                .map(|p| match p {
                    WebSocketRoutePattern::Glob(g) => Pattern {
                        glob: Some(g.as_str()),
                        regex_source: None,
                        regex_flags: None,
                    },
                    WebSocketRoutePattern::Regex(s, f) => Pattern {
                        glob: None,
                        regex_source: Some(s.as_str()),
                        regex_flags: Some(f.as_str()),
                    },
                })
                .collect(),
        };
        let _ = send_message!(self, "setWebSocketInterceptionPatterns", args);
        Ok(())
    }

    fn ws_matches(pattern: &WebSocketRoutePattern, url: &str) -> bool {
        match pattern {
            WebSocketRoutePattern::Glob(g) => {
                if g == "*" || g == "**" {
                    return true;
                }
                let mut regex = String::from("^");
                for ch in g.chars() {
                    match ch {
                        '*' => regex.push_str(".*"),
                        '.' => regex.push_str("\\."),
                        '?' => regex.push('.'),
                        c => regex.push(c),
                    }
                }
                regex.push('$');
                Regex::new(&regex)
                    .map(|re| re.is_match(url))
                    .unwrap_or(false)
            }
            WebSocketRoutePattern::Regex(source, flags) => {
                let mut builder = regex::RegexBuilder::new(source);
                if flags.contains('i') {
                    builder.case_insensitive(true);
                }
                builder.build().map(|re| re.is_match(url)).unwrap_or(false)
            }
        }
    }

    fn on_close(&self, ctx: &Context) -> Result<(), Error> {
        let browser = match self.browser().and_then(|b| b.upgrade()) {
            None => return Ok(()),
            Some(b) => b,
        };
        let this = get_object!(ctx, self.guid(), BrowserContext)?;
        browser.remove_context(&this);
        self.emit_event(Evt::Close);
        Ok(())
    }

    fn on_route(&self, ctx: &Context, params: Map<String, Value>) -> Result<(), Error> {
        let OnlyGuid { guid } = guid_from_keys(&params, &["route"])?;
        let route = get_object!(ctx, &guid, Route)?;
        let mut handled = false;
        {
            // pick the most recently added handler
            let mut var = self.var.lock().unwrap();
            if let Some(entry) = var.routes.last().cloned() {
                handled = true;
                if let Some(times) = entry.times {
                    if times <= 1 {
                        var.routes.pop();
                    } else if let Some(last) = var.routes.last_mut() {
                        last.times = Some(times - 1);
                    }
                }
                let cb = entry.handler;
                let r = route.clone();
                tokio::spawn(async move {
                    if let Some(route_arc) = r.upgrade() {
                        cb(route_arc).await;
                    }
                });
            }
        }
        if !handled {
            if let Some(r) = route.upgrade() {
                tokio::spawn(async move {
                    let _ = r.fallback().await;
                });
            }
        }
        self.emit_event(Evt::Route(route));
        Ok(())
    }

    pub(crate) fn handle_route_from_page(&self, route: Weak<Route>) {
        let mut handled = false;
        {
            let mut var = self.var.lock().unwrap();
            if let Some(entry) = var.routes.last().cloned() {
                handled = true;
                if let Some(times) = entry.times {
                    if times <= 1 {
                        var.routes.pop();
                    } else if let Some(last) = var.routes.last_mut() {
                        last.times = Some(times - 1);
                    }
                }
                let cb = entry.handler;
                let r = route.clone();
                tokio::spawn(async move {
                    if let Some(route_arc) = r.upgrade() {
                        cb(route_arc).await;
                    }
                });
            }
        }
        if !handled {
            if let Some(r) = route.upgrade() {
                tokio::spawn(async move {
                    let _ = r.fallback().await;
                });
            }
        }
    }

    fn on_web_socket_route(&self, ctx: &Context, params: Map<String, Value>) -> Result<(), Error> {
        let OnlyGuid { guid } = guid_from_keys(&params, &["route"])?;
        let route = get_object!(ctx, &guid, WebSocketRoute)?;
        self.handle_web_socket_route(route);
        Ok(())
    }

    pub(crate) fn handle_web_socket_route(&self, route: Weak<WebSocketRoute>) {
        let mut handled = false;
        let url = route.upgrade().map(|r| r.url().to_owned());
        if let Some(url) = url {
            let var = self.var.lock().unwrap();
            if let Some(entry) = var
                .websocket_routes
                .iter()
                .rfind(|entry| Self::ws_matches(&entry.pattern, &url))
            {
                handled = true;
                let cb = entry.handler.clone();
                let r = route.clone();
                tokio::spawn(async move {
                    if let Some(route_arc) = r.upgrade() {
                        cb(route_arc).await;
                    }
                });
            }
        }
        if !handled {
            if let Some(r) = route.upgrade() {
                tokio::spawn(async move {
                    let _ = r.connect_to_server().await;
                });
            }
        }
    }

    fn on_console(&self, ctx: &Context, params: Map<String, Value>) -> Result<(), Error> {
        let OnlyGuid { guid } = guid_from_keys(&params, &["message", "console", "consoleMessage"])?;
        let console = get_object!(ctx, &guid, ConsoleMessage)?;
        self.emit_event(Evt::Console(console));
        Ok(())
    }

    fn on_request(&self, ctx: &Context, params: Map<String, Value>) -> Result<(), Error> {
        let OnlyGuid { guid } = guid_from_keys(&params, &["request"])?;
        let request = get_object!(ctx, &guid, Request)?;
        self.emit_event(Evt::Request(request));
        Ok(())
    }

    fn on_request_failed(&self, ctx: &Context, params: Map<String, Value>) -> Result<(), Error> {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct De {
            request: OnlyGuid,
            response_end_timing: f64,
            failure_text: Option<String>,
        }
        let De {
            request: OnlyGuid { guid },
            response_end_timing,
            failure_text,
        } = serde_json::from_value(params.into())?;
        let request = get_object!(ctx, &guid, Request)?;
        let req = upgrade(&request)?;
        req.set_failure(failure_text);
        req.set_response_end(response_end_timing);
        self.emit_event(Evt::RequestFailed(request));
        Ok(())
    }

    fn on_request_finished(&self, ctx: &Context, params: Map<String, Value>) -> Result<(), Error> {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct De {
            request: OnlyGuid,
            response_end_timing: f64,
        }
        let De {
            request: OnlyGuid { guid },
            response_end_timing,
        } = serde_json::from_value(params.into())?;
        let request = get_object!(ctx, &guid, Request)?;
        let req = upgrade(&request)?;
        req.set_response_end(response_end_timing);
        self.emit_event(Evt::RequestFinished(request));
        Ok(())
    }

    fn on_response(&self, ctx: &Context, params: Map<String, Value>) -> Result<(), Error> {
        let OnlyGuid { guid } = guid_from_keys(&params, &["response"])?;
        let response = get_object!(ctx, &guid, Response)?;
        self.emit_event(Evt::Response(response));
        Ok(())
    }
}

impl RemoteObject for BrowserContext {
    fn channel(&self) -> &ChannelOwner {
        &self.channel
    }
    fn channel_mut(&mut self) -> &mut ChannelOwner {
        &mut self.channel
    }

    fn handle_event(
        &self,
        ctx: &Context,
        method: Str<Method>,
        params: Map<String, Value>,
    ) -> Result<(), Error> {
        match method.as_str() {
            "page" => {
                let OnlyGuid { guid } = guid_from_keys(&params, &["page"])?;
                let p = get_object!(ctx, &guid, Page)?;
                self.push_page(p.clone());
                self.emit_event(Evt::Page(p));
            }
            "close" => self.on_close(ctx)?,
            "bindingCall" => {}
            "route" => self.on_route(ctx, params)?,
            "console" => self.on_console(ctx, params)?,
            "request" => self.on_request(ctx, params)?,
            "requestfailed" => self.on_request_failed(ctx, params)?,
            "requestfinished" => self.on_request_finished(ctx, params)?,
            "response" => self.on_response(ctx, params)?,
            "pageError" | "pageerror" => {
                let page = params
                    .get("page")
                    .and_then(|v| only_guid(v).ok())
                    .and_then(|guid| get_object!(ctx, guid, Page).ok());
                let err_val = params.get("error").ok_or(Error::InvalidParams)?;
                let error = format_error_value(err_val)?;
                if let Some(page) = &page {
                    if let Some(p) = page.upgrade() {
                        p.emit_event(crate::imp::page::Evt::PageError(error.clone()));
                    }
                }
                self.emit_event(Evt::WebError(WebError::new(page, error)));
            }
            "webSocketRoute" => self.on_web_socket_route(ctx, params)?,
            _ => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Evt {
    Close,
    Page(Weak<Page>),
    Route(Weak<Route>),
    Console(Weak<ConsoleMessage>),
    Request(Weak<Request>),
    RequestFailed(Weak<Request>),
    RequestFinished(Weak<Request>),
    Response(Weak<Response>),
    WebError(WebError),
}

impl EventEmitter for BrowserContext {
    type Event = Evt;

    fn tx(&self) -> Option<broadcast::Sender<Self::Event>> {
        self.tx.lock().unwrap().clone()
    }

    fn set_tx(&self, tx: broadcast::Sender<Self::Event>) {
        *self.tx.lock().unwrap() = Some(tx);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventType {
    Close,
    Page,
    Route,
    Console,
    Request,
    RequestFailed,
    RequestFinished,
    Response,
    WebError,
}

impl IsEvent for Evt {
    type EventType = EventType;

    fn event_type(&self) -> Self::EventType {
        match self {
            Self::Close => EventType::Close,
            Self::Page(_) => EventType::Page,
            Self::Route(_) => EventType::Route,
            Self::Console(_) => EventType::Console,
            Self::Request(_) => EventType::Request,
            Self::RequestFailed(_) => EventType::RequestFailed,
            Self::RequestFinished(_) => EventType::RequestFinished,
            Self::Response(_) => EventType::Response,
            Self::WebError(_) => EventType::WebError,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Initializer {
    #[serde(default)]
    tracing: Option<OnlyGuid>,
    #[serde(default)]
    request_context: Option<OnlyGuid>,
}

impl fmt::Debug for BrowserContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserContext")
            .field("guid", &self.channel.guid)
            .finish()
    }
}

impl fmt::Debug for Variable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Variable")
            .field("pages_len", &self.pages.len())
            .field("timeout", &self.timeout)
            .field("navigation_timeout", &self.navigation_timeout)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imp::{browser::*, browser_type::*, playwright::Playwright};

    crate::runtime_test!(storage_state, {
        let driver = Driver::install().unwrap();
        let conn = Connection::run(&driver.executable()).unwrap();
        let p = Playwright::wait_initial_object(&conn).await.unwrap();
        let p = p.upgrade().unwrap();
        let chromium = p.chromium().upgrade().unwrap();
        let b = chromium.launch(LaunchArgs::default()).await.unwrap();
        let b = b.upgrade().unwrap();
        let c = b.new_context(NewContextArgs::default()).await.unwrap();
        let c = c.upgrade().unwrap();
        c.storage_state().await.unwrap();
        c.cookies(&[]).await.unwrap();
        c.set_default_timeout(30000).await.unwrap();
    });
}
