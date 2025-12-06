use crate::api::Frame;

/// Locator-first API, similar to Playwright Java/TypeScript.
/// This is a lightweight wrapper that reuses existing frame operations under the hood.
#[derive(Clone, Debug)]
pub struct Locator {
    frame: Frame,
    selector: String
}

impl Locator {
    pub(crate) fn new(frame: Frame, selector: String) -> Self {
        Self { frame, selector }
    }

    fn selector(&self) -> &str { &self.selector }

    pub fn nth(&self, index: i32) -> Self {
        // simple selector suffix to approximate nth; Playwright uses internal syntax
        Self::new(self.frame.clone(), format!("{} >> nth={}", self.selector, index))
    }

    pub fn locator(&self, selector: &str) -> Self {
        Self::new(
            self.frame.clone(),
            format!("{} >> {}", self.selector, selector)
        )
    }

    /// First matching locator.
    pub fn first(&self) -> Self { self.nth(0) }

    /// Last matching locator (uses nth=-1 semantics supported by Playwright selectors).
    pub fn last(&self) -> Self { self.nth(-1) }

    /// Filter this locator using Playwright selector extensions.
    pub fn filter(&self, has: Option<&Locator>, has_text: Option<&str>) -> Self {
        let mut selector = self.selector.clone();
        if let Some(has_locator) = has {
            selector = format!("{selector}:has({})", has_locator.selector);
        }
        if let Some(text) = has_text {
            let escaped = text.replace('"', "\\\"");
            selector = format!("{selector}:has-text(\"{escaped}\")");
        }
        Locator::new(self.frame.clone(), selector)
    }

    // Action builders
    pub fn click_builder(&self) -> crate::api::frame::ClickBuilder<'_> {
        self.frame.click_builder(self.selector())
    }
    pub fn dblclick_builder(&self) -> crate::api::frame::DblClickBuilder<'_> {
        self.frame.dblclick_builder(self.selector())
    }
    pub fn hover_builder(&self) -> crate::api::frame::HoverBuilder<'_> {
        self.frame.hover_builder(self.selector())
    }
    pub fn tap_builder(&self) -> crate::api::frame::TapBuilder<'_> {
        self.frame.tap_builder(self.selector())
    }
    pub fn fill_builder<'a>(&'a self, value: &'a str) -> crate::api::frame::FillBuilder<'a, 'a> {
        self.frame.fill_builder(self.selector(), value)
    }
    pub fn type_builder<'a>(&'a self, text: &'a str) -> crate::api::frame::TypeBuilder<'a, 'a> {
        self.frame.type_builder(self.selector(), text)
    }
    pub fn press_builder<'a>(&'a self, key: &'a str) -> crate::api::frame::PressBuilder<'a, 'a> {
        self.frame.press_builder(self.selector(), key)
    }
    pub fn check_builder(&self) -> crate::api::frame::CheckBuilder<'_> {
        self.frame.check_builder(self.selector())
    }
    pub fn uncheck_builder(&self) -> crate::api::frame::UncheckBuilder<'_> {
        self.frame.uncheck_builder(self.selector())
    }
    pub fn set_input_files_builder(
        &self,
        file: crate::imp::utils::File
    ) -> crate::api::frame::SetInputFilesBuilder<'_> {
        self.frame.set_input_files_builder(self.selector(), file)
    }

    // Convenience async methods
    pub async fn focus(&self, timeout: Option<f64>) -> crate::imp::core::ArcResult<()> {
        self.frame.focus(self.selector(), timeout).await
    }
    pub async fn text_content(&self, timeout: Option<f64>) -> crate::imp::core::ArcResult<Option<String>> {
        self.frame.text_content(self.selector(), timeout).await
    }
    pub async fn inner_text(&self, timeout: Option<f64>) -> crate::imp::core::ArcResult<String> {
        self.frame.inner_text(self.selector(), timeout).await
    }
    pub async fn inner_html(&self, timeout: Option<f64>) -> crate::imp::core::ArcResult<String> {
        self.frame.inner_html(self.selector(), timeout).await
    }
    pub async fn is_visible(&self, timeout: Option<f64>) -> crate::imp::core::ArcResult<bool> {
        self.frame.is_visible(self.selector(), timeout).await
    }
    pub async fn is_enabled(&self, timeout: Option<f64>) -> crate::imp::core::ArcResult<bool> {
        self.frame.is_enabled(self.selector(), timeout).await
    }
    pub async fn is_disabled(&self, timeout: Option<f64>) -> crate::imp::core::ArcResult<bool> {
        self.frame.is_disabled(self.selector(), timeout).await
    }
    pub async fn is_checked(&self, timeout: Option<f64>) -> crate::imp::core::ArcResult<bool> {
        self.frame.is_checked(self.selector(), timeout).await
    }

    pub async fn count(&self) -> crate::imp::core::ArcResult<usize> {
        self.frame
            .evaluate_on_selector_all::<_, usize>(self.selector(), "els => els.length", Option::<()>::None)
            .await
    }

    pub async fn all_text_contents(&self) -> crate::imp::core::ArcResult<Vec<String>> {
        self.frame
            .evaluate_on_selector_all::<_, Vec<String>>(
                self.selector(),
                "els => els.map(e => e.textContent || '')",
                Option::<()>::None
            )
            .await
    }

    pub async fn all_inner_texts(&self) -> crate::imp::core::ArcResult<Vec<String>> {
        self.frame
            .evaluate_on_selector_all::<_, Vec<String>>(
                self.selector(),
                "els => els.map(e => e.innerText)",
                Option::<()>::None
            )
            .await
    }

    pub async fn wait_for(
        &self,
        state: Option<crate::api::frame::FrameState>,
        timeout: Option<f64>
    ) -> crate::imp::core::ArcResult<()> {
        let mut b = self.frame.wait_for_selector_builder(self.selector());
        if let Some(s) = state {
            b = b.state(s);
        }
        if let Some(t) = timeout {
            b = b.timeout(t);
        }
        b.wait_for_selector().await?;
        Ok(())
    }

    pub async fn element_handle(
        &self
    ) -> crate::imp::core::ArcResult<Option<crate::api::ElementHandle>> {
        self.frame.query_selector(self.selector()).await
    }
}

/// FrameLocator is approximated by chaining selectors; it reuses the underlying Frame.
#[derive(Clone, Debug)]
pub struct FrameLocator {
    frame: Frame,
    selector: String
}

impl FrameLocator {
    pub(crate) fn new(frame: Frame, selector: String) -> Self {
        Self { frame, selector }
    }

    pub fn locator(&self, selector: &str) -> Locator {
        Locator::new(
            self.frame.clone(),
            format!("{} >> {}", self.selector, selector)
        )
    }
}
