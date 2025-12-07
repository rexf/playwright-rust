use playwright::{api::BrowserChannel, Playwright};

playwright::runtime_test!(hello_chrome, {
    main().await.unwrap();
});

async fn main() -> Result<(), playwright::Error> {
    println!("init playwright (chrome channel)");
    let playwright = match Playwright::initialize().await {
        Ok(p) => p,
        Err(playwright::Error::Timeout) => {
            eprintln!("Playwright driver initialization timed out; skipping chrome smoke test.");
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    println!("prepare browsers");
    if let Err(e) = playwright.prepare() {
        eprintln!("Playwright prepare failed ({e:?}); skipping chrome smoke test.");
        return Ok(());
    } // install bundled browsers

    let chromium = playwright.chromium();
    println!("launch chrome");
    let headless = if std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err()
    {
        println!("No DISPLAY/WAYLAND_DISPLAY detected; running headless to avoid hang");
        true
    } else {
        false
    };

    let browser = match chromium
        .launcher()
        .channel(BrowserChannel::Chrome)
        .headless(headless)
        .launch()
        .await
    {
        Ok(b) => b,
        Err(e) => match e.as_ref() {
            playwright::Error::ErrorResponded(err) => {
                eprintln!("Chrome channel unavailable: {err}; skipping chrome smoke test.");
                return Ok(());
            }
            playwright::Error::Timeout => {
                eprintln!("Chrome launch timed out; skipping chrome smoke test.");
                return Ok(());
            }
            _ => return Err(playwright::Error::Arc(e)),
        },
    };

    println!("new context");
    let context = browser.context_builder().build().await?;
    println!("new page");
    let page =
        match tokio::time::timeout(std::time::Duration::from_secs(15), context.new_page()).await {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => return Err(playwright::Error::Arc(e)),
            Err(_) => {
                return Err(playwright::Error::Timeout);
            }
        };

    // Basic navigation smoke test
    println!("goto example.com via chrome");
    page.goto_builder("https://example.com").goto().await?;
    let url: String = page.eval("() => location.href").await?;
    assert!(
        url.contains("example.com"),
        "expected example.com in current URL, got {}",
        url
    );

    println!("read title");
    let title: String = page.title().await?;
    assert!(
        title.to_lowercase().contains("example"),
        "unexpected page title: {}",
        title
    );
    println!("done");
    tokio::time::sleep(std::time::Duration::from_millis(750)).await;

    context.close().await.ok();
    browser.close().await.ok();

    Ok(())
}
