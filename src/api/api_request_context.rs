use crate::{
    api::api_response::APIResponse,
    imp::{
        api_request_context::{
            APIRequestContext as Impl, FetchArgs, FilePayload, MultipartField, NameValue,
            NewContextArgs
        },
        core::*,
        prelude::*,
        utils::{Header, HttpCredentials, ProxySettings}
    },
    Error
};
use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;

/// Wrapper over the driver-side APIRequestContext.
#[derive(Clone)]
pub struct APIRequestContext {
    pub(crate) inner: Weak<Impl>
}

impl APIRequestContext {
    pub(crate) fn new(inner: Weak<Impl>) -> Self { Self { inner } }

    pub async fn fetch(
        &self,
        url: &str,
        options: Option<RequestOptions>
    ) -> Result<APIResponse, Arc<Error>> {
        let args = options.unwrap_or_default().into_fetch_args(url);
        let payload = upgrade(&self.inner)?.fetch(args).await?;
        Ok(APIResponse::new(self.clone(), payload))
    }

    pub async fn get(
        &self,
        url: &str,
        mut options: Option<RequestOptions>
    ) -> Result<APIResponse, Arc<Error>> {
        options.get_or_insert_with(RequestOptions::default).method = Some("GET".into());
        self.fetch(url, options).await
    }

    pub async fn post(
        &self,
        url: &str,
        mut options: Option<RequestOptions>
    ) -> Result<APIResponse, Arc<Error>> {
        options.get_or_insert_with(RequestOptions::default).method = Some("POST".into());
        self.fetch(url, options).await
    }

    pub async fn put(
        &self,
        url: &str,
        mut options: Option<RequestOptions>
    ) -> Result<APIResponse, Arc<Error>> {
        options.get_or_insert_with(RequestOptions::default).method = Some("PUT".into());
        self.fetch(url, options).await
    }

    pub async fn delete(
        &self,
        url: &str,
        mut options: Option<RequestOptions>
    ) -> Result<APIResponse, Arc<Error>> {
        options.get_or_insert_with(RequestOptions::default).method = Some("DELETE".into());
        self.fetch(url, options).await
    }

    pub async fn patch(
        &self,
        url: &str,
        mut options: Option<RequestOptions>
    ) -> Result<APIResponse, Arc<Error>> {
        options.get_or_insert_with(RequestOptions::default).method = Some("PATCH".into());
        self.fetch(url, options).await
    }

    pub async fn head(
        &self,
        url: &str,
        mut options: Option<RequestOptions>
    ) -> Result<APIResponse, Arc<Error>> {
        options.get_or_insert_with(RequestOptions::default).method = Some("HEAD".into());
        self.fetch(url, options).await
    }

    pub async fn storage_state(&self) -> Result<String, Arc<Error>> {
        upgrade(&self.inner)?.storage_state().await
    }

    pub async fn dispose(&self, reason: Option<&str>) -> Result<(), Arc<Error>> {
        upgrade(&self.inner)?.dispose(reason).await
    }
}

#[derive(Clone, Default)]
pub struct RequestOptions {
    pub method: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub params: Option<HashMap<String, String>>,
    pub data: Option<RequestData>,
    pub form: Option<HashMap<String, String>>,
    pub multipart: Option<Vec<MultipartEntry>>,
    pub timeout: Option<f64>,
    pub fail_on_status_code: Option<bool>,
    pub ignore_https_errors: Option<bool>,
    pub max_redirects: Option<i32>,
    pub max_retries: Option<i32>
}

impl RequestOptions {
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.get_or_insert_with(HashMap::new).insert(key.into(), value.into());
        self
    }

    pub fn param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.get_or_insert_with(HashMap::new).insert(key.into(), value.into());
        self
    }

    pub fn data(mut self, data: RequestData) -> Self {
        self.data = Some(data);
        self
    }

    pub fn timeout(mut self, timeout: f64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn fail_on_status_code(mut self, fail: bool) -> Self {
        self.fail_on_status_code = Some(fail);
        self
    }

    pub fn ignore_https_errors(mut self, ignore: bool) -> Self {
        self.ignore_https_errors = Some(ignore);
        self
    }

    fn into_fetch_args(self, url: &str) -> FetchArgs {
        let mut args = FetchArgs {
            url: url.to_owned(),
            ..FetchArgs::default()
        };
        args.method = self.method;
        if let Some(headers) = self.headers {
            args.headers = Some(headers.into_iter().map(Header::from).collect());
        }
        if let Some(params) = self.params {
            args.params = Some(
                params
                    .into_iter()
                    .map(|(k, v)| NameValue::new(k, v))
                    .collect()
            );
        }
        if let Some(form) = self.form {
            args.form_data = Some(
                form.into_iter()
                    .map(|(k, v)| NameValue::new(k, v))
                    .collect()
            );
        }
        if let Some(multipart) = self.multipart {
            args.multipart_data = Some(
                multipart
                    .into_iter()
                    .map(|m| m.into_field())
                    .collect()
            );
        }
        match self.data {
            Some(RequestData::Json(v)) => {
                args.json_data = Some(v.to_string());
            }
            Some(RequestData::Text(s)) => {
                args.post_data = Some(general_purpose::STANDARD.encode(s.as_bytes()));
            }
            Some(RequestData::Bytes(b)) => {
                args.post_data = Some(general_purpose::STANDARD.encode(&b));
            }
            None => {}
        }
        args.fail_on_status_code = self.fail_on_status_code;
        args.ignore_https_errors = self.ignore_https_errors;
        args.max_redirects = self.max_redirects;
        args.max_retries = self.max_retries;
        args.timeout = self.timeout;
        args
    }
}

#[derive(Clone)]
pub enum RequestData {
    Bytes(Vec<u8>),
    Json(Value),
    Text(String)
}

#[derive(Clone)]
pub struct MultipartEntry {
    pub name: String,
    pub value: Option<String>,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub buffer: Option<Vec<u8>>
}

impl MultipartEntry {
    pub fn value(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: Some(value.into()),
            file_name: None,
            mime_type: None,
            buffer: None
        }
    }

    pub fn file(
        name: impl Into<String>,
        file_name: impl Into<String>,
        mime_type: Option<String>,
        buffer: Vec<u8>
    ) -> Self {
        Self {
            name: name.into(),
            value: None,
            file_name: Some(file_name.into()),
            mime_type,
            buffer: Some(buffer)
        }
    }

    fn into_field(self) -> MultipartField {
        let file = match (self.file_name, self.buffer) {
            (Some(name), Some(buffer)) => Some(FilePayload {
                name,
                mime_type: self.mime_type,
                buffer: general_purpose::STANDARD.encode(&buffer)
            }),
            _ => None
        };
        MultipartField {
            name: self.name,
            value: self.value,
            file
        }
    }
}

#[derive(Clone, Default)]
pub struct NewContextOptions {
    pub base_url: Option<String>,
    pub extra_http_headers: Option<HashMap<String, String>>,
    pub ignore_https_errors: Option<bool>,
    pub user_agent: Option<String>,
    pub timeout: Option<f64>,
    pub fail_on_status_code: Option<bool>,
    pub proxy: Option<ProxySettings>,
    pub storage_state: Option<Value>,
    pub http_credentials: Option<HttpCredentials>
}

impl NewContextOptions {
    pub fn base_url(mut self, base: impl Into<String>) -> Self {
        self.base_url = Some(base.into());
        self
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_http_headers
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
    }

    pub fn ignore_https_errors(mut self, ignore: bool) -> Self {
        self.ignore_https_errors = Some(ignore);
        self
    }

    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }

    pub fn timeout(mut self, timeout: f64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn fail_on_status_code(mut self, fail: bool) -> Self {
        self.fail_on_status_code = Some(fail);
        self
    }

    pub fn proxy(mut self, proxy: ProxySettings) -> Self {
        self.proxy = Some(proxy);
        self
    }

    pub fn storage_state(mut self, state: Value) -> Self {
        self.storage_state = Some(state);
        self
    }

    pub fn http_credentials(mut self, creds: HttpCredentials) -> Self {
        self.http_credentials = Some(creds);
        self
    }
}

impl From<NewContextOptions> for NewContextArgs {
    fn from(opts: NewContextOptions) -> Self {
        NewContextArgs {
            base_url: opts.base_url,
            extra_http_headers: opts
                .extra_http_headers
                .map(|m| m.into_iter().map(Header::from).collect()),
            ignore_https_errors: opts.ignore_https_errors,
            user_agent: opts.user_agent,
            timeout: opts.timeout,
            fail_on_status_code: opts.fail_on_status_code,
            proxy: opts.proxy,
            storage_state: opts.storage_state,
            http_credentials: opts.http_credentials
        }
    }
}
