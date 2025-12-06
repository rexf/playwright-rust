macro_rules! setter {
    (
        $(
            $(#[$meta:ident $($args:tt)*])*
            $field:ident :  Option<$t:ty>
        ),+
    ) => {
        $(
            paste::paste! {
                #[allow(clippy::wrong_self_convention)]
                $(#[$meta $($args)*])*
                pub fn [<$field>](mut self, x:$t) -> Self {
                    self.args.$field = Some(x);
                    self
                }
            }
        )*
        $(
            paste::paste! {
                pub fn [<clear_$field>](mut self) -> Self {
                    self.args.$field = None;
                    self
                }
            }
        )*
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! subscribe_event {
    () => {
        // TODO: FusedStream + Sink
        pub fn subscribe_event(
            &self
        ) -> Result<
            impl futures::stream::Stream<
                Item = Result<Event, tokio_stream::wrappers::errors::BroadcastStreamRecvError>
            >,
            Error
        > {
            use futures::stream::StreamExt;
            use tokio_stream::wrappers::BroadcastStream;
            let stream = BroadcastStream::new(upgrade(&self.inner)?.subscribe_event());
            Ok(stream.map(|e| e.map(Event::from)))
        }
    };
}

pub mod input_device;
pub mod playwright;
pub mod api_request;
pub mod api_request_context;
pub mod api_response;

pub mod accessibility;
pub mod browser;
pub mod browser_context;
pub mod browser_type;
pub mod console_message;
pub mod dialog;
pub mod download;
pub mod artifact;
pub mod cdp_session;
pub mod element_handle;
pub mod locator;
pub mod file_chooser;
pub mod frame;
pub mod js_handle;
pub mod page;
pub mod request;
pub mod response;
pub mod route;
pub mod selectors;
pub mod tracing;
pub mod video;
pub mod web_error;
pub mod websocket;
pub mod websocket_route;
pub mod worker;

pub use crate::imp::{core::DateTime, utils::*};

pub use self::playwright::Playwright;
pub use artifact::Artifact;
pub use accessibility::Accessibility;
pub use browser::Browser;
pub use browser_context::BrowserContext;
pub use browser_type::BrowserType;
pub use console_message::ConsoleMessage;
pub use cdp_session::CDPSession;
pub use api_request::APIRequest;
pub use api_request_context::{APIRequestContext, RequestOptions, NewContextOptions, RequestData};
pub use api_response::APIResponse;
pub use dialog::Dialog;
pub use download::Download;
pub use locator::{Locator, FrameLocator};
pub use element_handle::ElementHandle;
pub use file_chooser::FileChooser;
pub use frame::Frame;
pub use input_device::{Keyboard, Mouse, TouchScreen};
pub use js_handle::JsHandle;
pub use page::Page;
pub use request::Request;
pub use response::Response;
pub use route::Route;
pub use selectors::Selectors;
pub use tracing::{
    Tracing,
    StartOptions as TracingStartOptions,
    StartChunkOptions as TracingStartChunkOptions,
    StopOptions as TracingStopOptions,
    StopChunkOptions as TracingStopChunkOptions
};
pub use web_error::WebError;
pub use video::Video;
pub use websocket::WebSocket;
pub use websocket_route::{WebSocketRoute, Side as WebSocketRouteSide, Event as WebSocketRouteEvent};
pub use worker::Worker;

// Artifact
// BindingCall
// Stream

// Android
// AndroidDevice
// androidinput
// androidsocket
// androidwebview
// browserserver
// cdpsession
// coverage
// electron
// electronapplication
// logger
// websocketframe
