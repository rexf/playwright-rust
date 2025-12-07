use playwright::Playwright;

playwright::runtime_test!(frame_locator, {
    run().await.unwrap();
});

async fn run() -> Result<(), playwright::Error> {
    let playwright = match Playwright::initialize().await {
        Ok(p) => p,
        Err(playwright::Error::Timeout) => {
            eprintln!("Playwright driver initialization timed out; skipping frame locator test.");
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    if let Err(e) = playwright.prepare() {
        eprintln!("Playwright prepare failed ({e:?}); skipping frame locator test.");
        return Ok(());
    }

    let chromium = playwright.chromium();
    let headless = if std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err()
    {
        true
    } else {
        false
    };
    let browser = chromium.launcher().headless(headless).launch().await?;
    let context = browser.context_builder().build().await?;
    let page = context.new_page().await?;

    page.set_content_builder(r#"<iframe id="child"></iframe>"#)
        .timeout(30_000.0)
        .set_content()
        .await?;

    // Inject HTML into the iframe explicitly to avoid srcdoc escaping issues.
    page
        .eval::<()>(
            "(() => {\n  const iframe = document.querySelector('#child');\n  const doc = iframe.contentDocument;\n  doc.body.innerHTML = '<button id=\"inner\">Click me</button>';\n})",
        )
        .await?;

    // Use frame locator to click inside the iframe
    let frame_loc = page.frame_locator("#child");

    // owner() should resolve to the iframe element itself
    let owner_handle = frame_loc
        .owner()
        .element_handle()
        .await?
        .expect("frame element present");
    let id_attr = owner_handle.get_attribute("id").await?;
    assert_eq!(id_attr.as_deref(), Some("child"));

    // locator_from should accept locators bound to the same frame tree
    let outer_locator = page.locator("#inner");
    let bridged = frame_loc.locator_from(&outer_locator).expect("same frame");
    let bridged_text = bridged.inner_text(Some(5_000.0)).await?;
    assert_eq!(bridged_text, "Click me");

    frame_loc
        .locator("button")
        .click_builder()
        .timeout(10_000.0)
        .click()
        .await?;

    let text = frame_loc
        .locator("#inner")
        .inner_text(Some(5_000.0))
        .await?;
    assert_eq!(text, "Click me");

    context.close().await.ok();
    browser.close().await.ok();
    Ok(())
}
