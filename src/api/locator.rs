use crate::{api::Frame, imp::core::Error};

/// Locator-first API, similar to Playwright Java/TypeScript.
/// This is a lightweight wrapper that reuses existing frame operations under the hood.
#[derive(Clone, Debug)]
pub struct Locator {
    frame: Frame,
    selector: String,
}

/// Options for aria role-based queries (get_by_role).
#[derive(Clone, Debug, Default)]
pub struct GetByRoleOptions<'a> {
    pub name: Option<&'a str>,
    pub exact: Option<bool>,
    pub disabled: Option<bool>,
    pub selected: Option<bool>,
    pub checked: Option<bool>,
    pub pressed: Option<bool>,
    pub expanded: Option<bool>,
    pub include_hidden: Option<bool>,
    pub level: Option<i32>,
}

impl Locator {
    pub(crate) fn new(frame: Frame, selector: String) -> Self {
        Self { frame, selector }
    }

    fn selector(&self) -> &str {
        &self.selector
    }

    pub fn nth(&self, index: i32) -> Self {
        // simple selector suffix to approximate nth; Playwright uses internal syntax
        Self::new(
            self.frame.clone(),
            format!("{} >> nth={}", self.selector, index),
        )
    }

    pub fn locator(&self, selector: &str) -> Self {
        Self::new(
            self.frame.clone(),
            format!("{} >> {}", self.selector, selector),
        )
    }

    /// First matching locator.
    pub fn first(&self) -> Self {
        self.nth(0)
    }

    /// Last matching locator (uses nth=-1 semantics supported by Playwright selectors).
    pub fn last(&self) -> Self {
        self.nth(-1)
    }

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

    fn chain_selector(&self, selector: String) -> Self {
        Locator::new(self.frame.clone(), selector)
    }

    /// Locate by ARIA role with optional attributes. Mimics Playwright getByRole semantics in Java/TS.
    pub fn get_by_role<'a>(&self, role: &str, options: Option<GetByRoleOptions<'a>>) -> Self {
        let selector = build_role_selector(role, options);
        self.chain_selector(selector)
    }

    /// Locate by visible text. If `exact` is true, matches whole text.
    pub fn get_by_text(&self, text: &str, exact: bool) -> Self {
        let selector = build_text_selector(text, exact);
        self.chain_selector(selector)
    }

    /// Locate by associated label text.
    pub fn get_by_label(&self, text: &str, exact: bool) -> Self {
        let selector = build_label_selector(text, exact);
        self.chain_selector(selector)
    }

    /// Locate by placeholder attribute.
    pub fn get_by_placeholder(&self, text: &str, exact: bool) -> Self {
        let selector = build_placeholder_selector(text, exact);
        self.chain_selector(selector)
    }

    /// Locate by alt text.
    pub fn get_by_alt_text(&self, text: &str, exact: bool) -> Self {
        let selector = build_alt_text_selector(text, exact);
        self.chain_selector(selector)
    }

    /// Locate by title attribute.
    pub fn get_by_title(&self, text: &str, exact: bool) -> Self {
        let selector = build_title_selector(text, exact);
        self.chain_selector(selector)
    }

    /// Locate by data-testid (Playwright's default test id attribute).
    pub fn get_by_test_id(&self, test_id: &str) -> Self {
        let selector = build_test_id_selector(test_id);
        self.chain_selector(selector)
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
        file: crate::imp::utils::File,
    ) -> crate::api::frame::SetInputFilesBuilder<'_> {
        self.frame.set_input_files_builder(self.selector(), file)
    }

    // Convenience async methods
    pub async fn focus(&self, timeout: Option<f64>) -> crate::imp::core::ArcResult<()> {
        self.frame.focus(self.selector(), timeout).await
    }
    pub async fn text_content(
        &self,
        timeout: Option<f64>,
    ) -> crate::imp::core::ArcResult<Option<String>> {
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
            .evaluate_on_selector_all::<_, usize>(
                self.selector(),
                "els => els.length",
                Option::<()>::None,
            )
            .await
    }

    pub async fn all_text_contents(&self) -> crate::imp::core::ArcResult<Vec<String>> {
        self.frame
            .evaluate_on_selector_all::<_, Vec<String>>(
                self.selector(),
                "els => els.map(e => e.textContent || '')",
                Option::<()>::None,
            )
            .await
    }

    pub async fn all_inner_texts(&self) -> crate::imp::core::ArcResult<Vec<String>> {
        self.frame
            .evaluate_on_selector_all::<_, Vec<String>>(
                self.selector(),
                "els => els.map(e => e.innerText)",
                Option::<()>::None,
            )
            .await
    }

    pub async fn wait_for(
        &self,
        state: Option<crate::api::frame::FrameState>,
        timeout: Option<f64>,
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
        &self,
    ) -> crate::imp::core::ArcResult<Option<crate::api::ElementHandle>> {
        self.frame.query_selector(self.selector()).await
    }
}

/// FrameLocator is approximated by chaining selectors; it reuses the underlying Frame.
#[derive(Clone, Debug)]
pub struct FrameLocator {
    frame: Frame,
    selector: String,
}

impl FrameLocator {
    pub(crate) fn new(frame: Frame, selector: String) -> Self {
        Self { frame, selector }
    }

    /// First matching frame locator.
    pub fn first(&self) -> Self {
        Self::new(self.frame.clone(), format!("{} >> nth=0", self.selector))
    }

    /// Last matching frame locator.
    pub fn last(&self) -> Self {
        Self::new(self.frame.clone(), format!("{} >> nth=-1", self.selector))
    }

    /// Nth matching frame locator.
    pub fn nth(&self, index: i32) -> Self {
        Self::new(
            self.frame.clone(),
            format!("{} >> nth={}", self.selector, index),
        )
    }

    pub fn locator(&self, selector: &str) -> Locator {
        Locator::new(
            self.frame.clone(),
            format!(
                "{} >> internal:control=enter-frame >> {}",
                self.selector, selector
            ),
        )
    }

    /// Use an existing locator (from the same frame) as the target of this frame locator.
    /// Returns an error if the locator belongs to a different frame.
    pub fn locator_from(&self, locator: &Locator) -> Result<Locator, Error> {
        if locator.frame != self.frame {
            return Err(Error::InvalidParams);
        }
        Ok(self.locator(locator.selector()))
    }

    /// Nested frame locator.
    pub fn frame_locator(&self, selector: &str) -> FrameLocator {
        FrameLocator::new(
            self.frame.clone(),
            format!(
                "{} >> internal:control=enter-frame >> {}",
                self.selector, selector
            ),
        )
    }

    pub fn get_by_role<'a>(&self, role: &str, options: Option<GetByRoleOptions<'a>>) -> Locator {
        self.locator(&build_role_selector(role, options))
    }

    pub fn get_by_text(&self, text: &str, exact: bool) -> Locator {
        self.locator(&build_text_selector(text, exact))
    }

    pub fn get_by_label(&self, text: &str, exact: bool) -> Locator {
        self.locator(&build_label_selector(text, exact))
    }

    pub fn get_by_placeholder(&self, text: &str, exact: bool) -> Locator {
        self.locator(&build_placeholder_selector(text, exact))
    }

    pub fn get_by_alt_text(&self, text: &str, exact: bool) -> Locator {
        self.locator(&build_alt_text_selector(text, exact))
    }

    pub fn get_by_title(&self, text: &str, exact: bool) -> Locator {
        self.locator(&build_title_selector(text, exact))
    }

    pub fn get_by_test_id(&self, test_id: &str) -> Locator {
        self.locator(&build_test_id_selector(test_id))
    }

    /// Returns a locator that resolves to the owning frame element.
    pub fn owner(&self) -> Locator {
        Locator::new(self.frame.clone(), self.selector.clone())
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn append_text_filter(selector: &mut String, text: &str, exact: bool) {
    let escaped = escape(text);
    if exact {
        selector.push_str(&format!(":text-is(\"{escaped}\")"));
    } else {
        selector.push_str(&format!(":has-text(\"{escaped}\")"));
    }
}

pub(crate) fn build_text_selector(text: &str, exact: bool) -> String {
    let mut selector = String::from("text=");
    if exact {
        selector.push('"');
        selector.push_str(&escape(text));
        selector.push('"');
    } else {
        selector.push_str(&escape(text));
    }
    selector
}

pub(crate) fn build_label_selector(text: &str, exact: bool) -> String {
    let escaped = escape(text);
    if exact {
        format!(
            "[aria-label=\"{e}\"], label:has-text(\"{e}\") input, label:has-text(\"{e}\") textarea, label:has-text(\"{e}\") select",
            e = escaped
        )
    } else {
        format!(
            "[aria-label*=\"{e}\"], label:has-text(\"{e}\") input, label:has-text(\"{e}\") textarea, label:has-text(\"{e}\") select",
            e = escaped
        )
    }
}

pub(crate) fn build_placeholder_selector(text: &str, exact: bool) -> String {
    let mut selector = String::from("input[placeholder");
    if exact {
        selector.push_str(&format!("=\"{}\"]", escape(text)));
    } else {
        selector.push_str(&format!("*=\"{}\"]", escape(text)));
    }
    selector
}

pub(crate) fn build_alt_text_selector(text: &str, exact: bool) -> String {
    let mut selector = String::from("[alt");
    if exact {
        selector.push_str(&format!("=\"{}\"]", escape(text)));
    } else {
        selector.push_str(&format!("*=\"{}\"]", escape(text)));
    }
    selector
}

pub(crate) fn build_title_selector(text: &str, exact: bool) -> String {
    let mut selector = String::from("[title");
    if exact {
        selector.push_str(&format!("=\"{}\"]", escape(text)));
    } else {
        selector.push_str(&format!("*=\"{}\"]", escape(text)));
    }
    selector
}

pub(crate) fn build_test_id_selector(test_id: &str) -> String {
    format!("[data-testid=\"{}\"]", escape(test_id))
}

pub(crate) fn build_role_selector<'a>(role: &str, options: Option<GetByRoleOptions<'a>>) -> String {
    let mut selector = format!("[role=\"{}\"]", role);
    if let Some(opts) = options {
        if let Some(name) = opts.name {
            append_text_filter(&mut selector, name, opts.exact.unwrap_or(false));
        }
        if let Some(true) = opts.disabled {
            selector.push_str(":disabled");
        }
        if let Some(true) = opts.selected {
            selector.push_str(":is([aria-selected=\"true\"], :selected)");
        }
        if let Some(true) = opts.checked {
            selector.push_str(":is(:checked,[aria-checked=\"true\"])");
        }
        if let Some(true) = opts.pressed {
            selector.push_str("[aria-pressed=\"true\"]");
        }
        if let Some(true) = opts.expanded {
            selector.push_str("[aria-expanded=\"true\"]");
        }
        if let Some(false) = opts.include_hidden {
            selector.push_str(":not([hidden])");
        }
        if let Some(level) = opts.level {
            selector.push_str(&format!("[aria-level=\"{level}\"]"));
        }
    }
    selector
}
