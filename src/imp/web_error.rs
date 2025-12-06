use crate::imp::{page::Page, prelude::*};

#[derive(Debug, Clone)]
pub(crate) struct WebError {
    page: Option<Weak<Page>>,
    error: String
}

impl WebError {
    pub(crate) fn new(page: Option<Weak<Page>>, error: String) -> Self {
        Self { page, error }
    }

    pub(crate) fn page(&self) -> Option<Weak<Page>> { self.page.clone() }

    pub(crate) fn error(&self) -> &str { &self.error }
}
