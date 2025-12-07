use crate::imp::{core::*, prelude::*, utils::Header};
use base64::{engine::general_purpose, Engine as _};

/// Remote representation of Playwright APIRequestContext used for API testing.
#[derive(Debug)]
pub(crate) struct APIRequestContext {
    channel: ChannelOwner,
}

impl APIRequestContext {
    pub(crate) fn try_new(_ctx: &Context, channel: ChannelOwner) -> Result<Self, Error> {
        Ok(Self { channel })
    }

    /// Low-level fetch that mirrors the driver API.
    pub(crate) async fn fetch(&self, args: FetchArgs) -> ArcResult<APIResponsePayload> {
        let v = send_message!(self, "fetch", args);
        let response = v.get("response").ok_or(Error::InvalidParams)?.clone();
        let payload: APIResponsePayload = serde_json::from_value(response).map_err(Error::Serde)?;
        Ok(payload)
    }

    pub(crate) async fn fetch_response_body(&self, fetch_uid: &str) -> ArcResult<Vec<u8>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            fetch_uid: &'a str,
        }
        let v = send_message!(self, "fetchResponseBody", Args { fetch_uid });
        let b64 = v
            .get("binary")
            .and_then(|v| v.as_str())
            .ok_or(Error::InvalidParams)?;
        let data = general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| Arc::new(Error::InvalidBase64(e)))?;
        Ok(data)
    }

    pub(crate) async fn fetch_log(&self, fetch_uid: &str) -> ArcResult<Vec<String>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            fetch_uid: &'a str,
        }
        let v = send_message!(self, "fetchLog", Args { fetch_uid });
        let entries = v
            .get("log")
            .and_then(|v| v.as_array())
            .ok_or(Error::InvalidParams)?;
        Ok(entries
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_owned()))
            .collect())
    }

    pub(crate) async fn dispose_api_response(&self, fetch_uid: &str) -> ArcResult<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            fetch_uid: &'a str,
        }
        let _ = send_message!(self, "disposeAPIResponse", Args { fetch_uid });
        Ok(())
    }

    pub(crate) async fn storage_state(&self) -> ArcResult<String> {
        let v = send_message!(self, "storageState", Map::new());
        Ok(v.to_string())
    }

    pub(crate) async fn dispose(&self, reason: Option<&str>) -> ArcResult<()> {
        #[skip_serializing_none]
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            reason: Option<&'a str>,
        }
        let _ = send_message!(self, "dispose", Args { reason });
        Ok(())
    }
}

impl RemoteObject for APIRequestContext {
    fn channel(&self) -> &ChannelOwner {
        &self.channel
    }
    fn channel_mut(&mut self) -> &mut ChannelOwner {
        &mut self.channel
    }
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FetchArgs {
    pub url: String,
    pub params: Option<Vec<NameValue>>,
    pub method: Option<String>,
    pub headers: Option<Vec<Header>>,
    pub json_data: Option<String>,
    pub post_data: Option<String>,
    pub form_data: Option<Vec<NameValue>>,
    pub multipart_data: Option<Vec<MultipartField>>,
    pub fail_on_status_code: Option<bool>,
    pub ignore_https_errors: Option<bool>,
    pub max_redirects: Option<i32>,
    pub max_retries: Option<i32>,
    pub timeout: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct NameValue {
    pub name: String,
    pub value: String,
}

impl NameValue {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MultipartField {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<FilePayload>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilePayload {
    pub name: String,
    pub mime_type: Option<String>,
    pub buffer: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct APIResponsePayload {
    pub fetch_uid: String,
    pub url: String,
    pub status: i32,
    pub status_text: String,
    pub headers: Vec<Header>,
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NewContextArgs {
    pub base_url: Option<String>,
    pub extra_http_headers: Option<Vec<Header>>,
    pub ignore_https_errors: Option<bool>,
    pub user_agent: Option<String>,
    pub timeout: Option<f64>,
    pub fail_on_status_code: Option<bool>,
    pub proxy: Option<crate::imp::utils::ProxySettings>,
    pub storage_state: Option<Value>,
    pub http_credentials: Option<crate::imp::utils::HttpCredentials>,
}
