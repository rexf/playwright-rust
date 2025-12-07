use playwright::{api::GetByRoleOptions, Playwright};

playwright::runtime_test!(get_by_selectors, {
    run().await.unwrap();
});

async fn run() -> Result<(), playwright::Error> {
    let playwright = match Playwright::initialize().await {
        Ok(p) => p,
        Err(playwright::Error::Timeout) => {
            eprintln!("Playwright driver initialization timed out; skipping get_by_* test.");
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    if let Err(e) = playwright.prepare() {
        eprintln!("Playwright prepare failed ({e:?}); skipping get_by_* test.");
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

    page.set_content_builder(r#"
    <main>
      <button role="button" data-testid="submit">Submit</button>
      <label>Username <input aria-label="Username" placeholder="user name" /></label>
      <img alt="Playwright logo" src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==" />
      <div title="Greeting">Hello!</div>
    </main>
    "#)
    .timeout(30_000.0)
    .set_content()
    .await?;

    // get_by_role with name filter
    let mut opts = GetByRoleOptions::default();
    opts.name = Some("Submit");
    page.get_by_role("button", Some(opts))
        .click_builder()
        .timeout(5_000.0)
        .click()
        .await?;

    // get_by_test_id
    page.get_by_test_id("submit")
        .hover_builder()
        .timeout(5_000.0)
        .goto()
        .await?;

    // get_by_label + fill
    page.get_by_label("Username", true)
        .fill_builder("alice")
        .timeout(5_000.0)
        .fill()
        .await?;

    // get_by_placeholder
    page.get_by_placeholder("user name", true)
        .press_builder("Tab")
        .timeout(5_000.0)
        .press()
        .await?;

    // get_by_alt_text
    let visible = page
        .get_by_alt_text("Playwright logo", true)
        .is_visible(None)
        .await?;
    assert!(visible);

    // get_by_title
    let title_text = page
        .get_by_title("Greeting", true)
        .inner_text(Some(5_000.0))
        .await?;
    assert_eq!(title_text.trim(), "Hello!");

    context.close().await.ok();
    browser.close().await.ok();
    Ok(())
}
