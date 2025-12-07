use crate::{api::Page, imp::web_error::WebError as Impl};

#[derive(Clone, Debug)]
pub struct WebError {
    page: Option<Page>,
    error: String,
}

impl WebError {
    pub(crate) fn new(inner: Impl) -> Self {
        let page = inner.page().map(Page::new);
        Self {
            page,
            error: inner.error().to_owned(),
        }
    }

    pub fn page(&self) -> Option<&Page> {
        self.page.as_ref()
    }

    pub fn error(&self) -> &str {
        &self.error
    }
}
