use crate::imp::{
    core::{upgrade, ArcResult},
    dialog::Dialog as Impl,
    prelude::*,
};

/// `Dialog` objects are dispatched by page via the [page::Event::Dialog](crate::api::page::Event::Dialog) event.
///
/// An example of using `Dialog` class:
///
/// ```js
/// const { chromium } = require('playwright');  // Or 'firefox' or 'webkit'.
///
/// (async () => {
///  const browser = await chromium.launch();
///  const page = await browser.newPage();
///  page.on('dialog', async dialog => {
///    console.log(dialog.message());
///    await dialog.dismiss();
///  });
///  await page.evaluate(() => alert('1'));
///  await browser.close();
/// })();
/// ```
///
/// > NOTE: Dialogs are dismissed automatically, unless there is a [`event: Page.dialog`] listener. When listener is
/// present, it **must** either [`method: Dialog.accept`] or [`method: Dialog.dismiss`] the dialog - otherwise the page will
/// [freeze](https://developer.mozilla.org/en-US/docs/Web/JavaScript/EventLoop#never_blocking) waiting for the dialog, and
/// actions like click will never finish.
#[derive(Clone, Debug)]
pub struct Dialog {
    inner: Weak<Impl>,
}

impl Dialog {
    pub(crate) fn new(inner: Weak<Impl>) -> Self {
        Self { inner }
    }

    /// Returns when the dialog has been accepted. Optional prompt text if the dialog is a prompt.
    pub async fn accept(&self, prompt_text: Option<&str>) -> ArcResult<()> {
        upgrade(&self.inner)?.accept(prompt_text).await
    }

    /// Returns when the dialog has been dismissed.
    pub async fn dismiss(&self) -> ArcResult<()> {
        upgrade(&self.inner)?.dismiss().await
    }

    /// If dialog is prompt, returns default prompt value. Otherwise, returns empty string.
    pub fn default_value(&self) -> ArcResult<String> {
        Ok(upgrade(&self.inner)?.default_value().to_owned())
    }

    /// A message displayed in the dialog.
    pub fn message(&self) -> ArcResult<String> {
        Ok(upgrade(&self.inner)?.message().to_owned())
    }

    /// Returns dialog's type, can be one of `alert`, `beforeunload`, `confirm` or `prompt`.
    pub fn r#type(&self) -> ArcResult<String> {
        Ok(upgrade(&self.inner)?.r#type().to_owned())
    }
}
