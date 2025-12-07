use super::Which;
use futures::stream::StreamExt;
use playwright::api::{
    page, BrowserContext, DocumentLoadState, Geolocation, Page, Viewport,
};
use tokio::time::{timeout, Duration};

macro_rules! concurrent {
    ($which:expr, $($e:expr),*) => {
        if $which != Which::Firefox {
            tokio::join!($($e),*);
        } else {
            $($e.await;)*
        }
    }
}

pub async fn all(c: &BrowserContext, port: u16, which: Which) {
    let page = c.new_page().await.unwrap();
    eq_context_close(c, &page).await;
    ensure_timeout(&page).await;
    set_timeout(&page).await;
    context_pages_visibility(c).await;
    reject_promises_when_page_closed(c).await;
    beforeunload_runs_when_asked(c, port).await;
    beforeunload_not_run_by_default(c, port).await;
    page_close_state(c).await;
    close_callable_twice(c).await;
    page_url_should_work(c, port).await;
    load_events_should_fire(&page, port).await;
    domcontentloaded_event_should_fire(&page, port).await;
    opener_should_work(c).await;
    opener_should_be_null_after_parent_close(c).await;
    page_url_should_include_hashes(c, port).await;
    dialog_should_fire(&page).await;
    dialog_accept_prompt(&page).await;
    dialog_dismiss_prompt(&page).await;
    dialog_accept_confirm(&page).await;
    dialog_dismiss_confirm(&page).await;
    dialog_auto_dismiss_without_listener(&page).await;
    wait_for_load_state_should_work(&page, port).await;
    wait_for_url_should_work(&page, port).await;
    permissions(c, &page, port, which).await;
    if which != Which::Firefox {
        // XXX: go_back response is null on firefox
        navigations(&page, port).await;
    }
    front_should_work(c, &page).await;
    concurrent!(
        which,
        set_extra_http_headers(c, port),
        focus_should_work(c),
        add_script_tag_includes_source_url(c, port),
        reload_should_worker(c),
        screenshot_should_work(&page),
        title_should_work(&page),
        check_should_work(c),
        pointer(c),
        viewport(c),
        download(c, port),
        workers_should_work(c, port, which),
        accessibility(c),
        query_selector_and_eval(c),
        input(c)
    );
    // TODO
    // file_chooser(c, port).await;
    if which != Which::Firefox {
        pdf_should_work(&page).await;
    }
    video(&page).await;
    emulate_media(&page).await;
}

macro_rules! done {
    ($e:expr) => {
        $e.await.unwrap()
    };
}

async fn eq_context_close(c: &BrowserContext, p1: &Page) {
    let p2 = new(c).await;
    assert_ne!(p1, &p2);
    assert_eq!(&p1.context(), c);
    assert_eq!(&p2.context(), c);
    ensure_close(&p2).await;
}

async fn ensure_close(page: &Page) {
    let mut rx = page.subscribe_event().unwrap();
    let receive_close = async {
        let mut received = false;
        while let Some(Ok(evt)) = rx.next().await {
            if let page::Event::Close = evt {
                received = true;
                break;
            }
        }
        received
    };
    let (received, wait_result, result) = tokio::join!(
        receive_close,
        page.expect_event(page::EventType::Close),
        page.close(None)
    );
    result.unwrap();
    assert!(received);
    match wait_result.unwrap() {
        page::Event::Close => (),
        _ => unreachable!(),
    }
}

async fn front_should_work(c: &BrowserContext, p1: &Page) {
    let p2 = new(c).await;
    done!(p1.bring_to_front());
    assert_eq!(
        done!(p1.eval::<String>("document.visibilityState")),
        "visible"
    );
    assert_eq!(
        done!(p2.eval::<String>("document.visibilityState")),
        "visible"
    );
    close(&p2).await;
}

async fn focus_should_work(c: &BrowserContext) {
    let page = new(c).await;
    page.set_content_builder("<div id=d1 tabIndex=0></div>")
        .set_content()
        .await
        .unwrap();
    assert_eq!(
        page.eval::<String>("() => document.activeElement.nodeName")
            .await
            .unwrap(),
        "BODY"
    );
    page.focus("#d1", None).await.unwrap();
    assert_eq!(
        page.eval::<String>("(s) => document.activeElement.id")
            .await
            .unwrap(),
        "d1"
    );
    close(&page).await;
}

async fn reload_should_worker(c: &BrowserContext) {
    let page = new(c).await;
    page.evaluate::<i32, i32>("x => window._foo = x", 10)
        .await
        .unwrap();
    page.reload_builder().reload().await.unwrap();
    let x: Option<i32> = page.eval("() => window._foo").await.unwrap();
    assert_eq!(x, None);
    close(&page).await;
}

async fn navigations(page: &Page, port: u16) {
    assert_eq!(page.go_back_builder().go_back().await.unwrap(), None);
    let url1 = super::url_static(port, "/empty.html");
    let url2 = super::url_static(port, "/empty2.html");
    page.goto_builder(&url1).goto().await.unwrap();
    page.goto_builder(&url2).goto().await.unwrap();
    {
        let response = page.go_back_builder().go_back().await.unwrap().unwrap();
        assert!(response.ok().unwrap());
        assert_eq!(response.url().unwrap(), url1);
    }
    {
        let response = page
            .go_forward_builder()
            .go_forward()
            .await
            .unwrap()
            .unwrap();
        assert!(response.ok().unwrap());
        assert_eq!(response.url().unwrap(), url2);
    }
    let maybe_response = page.go_forward_builder().go_forward().await.unwrap();
    assert_eq!(maybe_response, None);
}

async fn set_timeout(page: &Page) {
    page.set_default_navigation_timeout(10000).await.unwrap();
    page.set_default_timeout(10000).await.unwrap();
}

async fn workers_should_work(c: &BrowserContext, port: u16, which: Which) {
    let page = new(c).await;
    let url = super::url_static(port, "/worker.html");
    let js = super::url_static(port, "/worker.js");
    let empty = super::url_static(port, "/empty.html");
    let workers = || page.workers().unwrap();
    assert_eq!(workers().len(), 0);
    let (_, _) = tokio::join!(
        page.expect_event(page::EventType::Worker),
        page.goto_builder(&url).goto()
    );
    assert_eq!(workers().len(), 1);
    let w = &workers()[0];
    assert_eq!(
        w.url().unwrap(),
        match which {
            Which::Firefox => "worker.js".to_owned(),
            _ => js,
        }
    );
    assert_eq!(
        w.eval::<String>("() => self.workerFunction()")
            .await
            .unwrap(),
        "worker function result"
    );
    page.goto_builder(&empty).goto().await.unwrap();
    assert_eq!(workers().len(), 0);
    close(&page).await;
}

async fn ensure_timeout(page: &Page) {
    page.set_default_timeout(500).await.unwrap();
    match page.expect_event(page::EventType::Load).await {
        Err(playwright::Error::Timeout) => {}
        _ => panic!("Not expected"),
    }
}

async fn permissions(c: &BrowserContext, page: &Page, port: u16, which: Which) {
    const PERMISSION_DENIED: i32 = 1;
    let snippet = "async () => {
        let getCurrentPositionAsync =
            () => new Promise((resolve, reject) =>
                navigator.geolocation.getCurrentPosition(resolve, reject));
        let err;
        const result = await getCurrentPositionAsync().catch(e => { err = e; });
        return [result?.coords.latitude, err?.code];
    }";
    page.goto_builder(&super::url_static(port, "/empty.html"))
        .goto()
        .await
        .unwrap();
    let geo = || async {
        page.eval::<(Option<f64>, Option<i32>)>(snippet)
            .await
            .unwrap()
    };
    assert_eq!(get_permission(page, "geolocation").await, "granted");
    c.clear_permissions().await.unwrap();
    assert_eq!(get_permission(page, "geolocation").await, "prompt");
    if which != Which::Firefox {
        // firefox shows prompt
        assert_eq!(geo().await, (None, Some(PERMISSION_DENIED)));
    }
    c.grant_permissions(&["geolocation".into()], None)
        .await
        .unwrap();
    assert_eq!(get_permission(page, "geolocation").await, "granted");
    c.set_geolocation(Some(&Geolocation {
        latitude: 59.95,
        longitude: 2.,
        accuracy: None,
    }))
    .await
    .unwrap();
    let result = geo().await;
    dbg!(&result);
    assert_eq!(result.0, Some(59.95))
}

async fn get_permission(p: &Page, name: &str) -> String {
    p.evaluate(
        "name => navigator.permissions.query({name}).then(result => result.state)",
        name,
    )
    .await
    .unwrap()
}

async fn viewport(c: &BrowserContext) {
    let p = new(c).await;
    let v = Viewport {
        width: 500,
        height: 500,
    };
    dbg!(p.viewport_size().unwrap());
    p.set_viewport_size(v.clone()).await.unwrap();
    assert_eq!(p.viewport_size().unwrap(), Some(v));
    close(&p).await;
}

async fn download(c: &BrowserContext, port: u16) {
    let p = new(c).await;
    p.set_content_builder(&format!(
        r#"<a href="{}">download</a>"#,
        super::url_download(port, "/worker.html")
    ))
    .set_content()
    .await
    .unwrap();
    let (d, _) = tokio::join!(
        p.expect_event(page::EventType::Download),
        p.click_builder("a").click()
    );
    let download = match d.unwrap() {
        page::Event::Download(d) => d,
        _ => unreachable!(),
    };
    dbg!(download.url());
    dbg!(download.suggested_filename());
    dbg!(download.path().await.unwrap());
    assert!(!download.url().is_empty());
    assert!(!download.suggested_filename().is_empty());
    assert!(download.path().await.unwrap().is_some());
    assert_eq!(download.failure().await.unwrap(), None);
    let tmp = super::temp_dir().join(download.suggested_filename());
    download.save_as(tmp).await.unwrap();
    download.delete().await.unwrap();
    close(&p).await;
}

async fn video(p: &Page) {
    let video = p.video().unwrap().unwrap();
    dbg!(video.path().unwrap());
    // TODO
    // let path = super::temp_dir().join("video.webm");
    // video.save_as(&path).await.unwrap();
    // assert!(path.is_file());
    // video.delete().await.unwrap();
}

async fn add_script_tag_includes_source_url(c: &BrowserContext, port: u16) {
    // Skips WebKit where upstream behavior differs (mirroring Java @DisabledIf for WebKit).
    if cfg!(target_os = "macos") {
        return;
    }
    let page = new(c).await;
    let url = super::url_static(port, "/empty.html");
    page.goto_builder(&url).goto().await.unwrap();
    let script_path = std::path::Path::new("tests/server/injectedfile.js");
    page.add_script_tag_builder("// placeholder")
        .path(script_path)
        .add_script_tag()
        .await
        .unwrap();
    let stack: String = page
        .eval("() => window.__injectedError.stack")
        .await
        .unwrap();
    assert!(
        stack.contains("injectedfile.js"),
        "stack should include source URL: {}",
        stack
    );
    close(&page).await;
}

async fn accessibility(c: &BrowserContext) {
    let p = new(c).await;
    use playwright::api::accessibility::SnapshotResponse;
    let ac = &p.accessibility;
    p.set_content_builder(
        r#"<div>\
            <span>Hello World</span>\
            <input placeholder="Empty input" />\
        </div>"#,
    )
    .set_content()
    .await
    .unwrap();
    p.focus("input", None).await.unwrap();
    let span = p.query_selector("span").await.unwrap().unwrap();
    let input = p.query_selector("input").await.unwrap().unwrap();
    let snapshot = ac
        .snapshot_builder()
        .try_root(input)
        .unwrap()
        .snapshot()
        .await
        .unwrap();
    let input_response = Some(SnapshotResponse {
        role: "textbox".into(),
        name: "Empty input".into(),
        value: None,
        description: None,
        keyshortcuts: None,
        roledescription: None,
        valuetext: None,
        disabled: None,
        expanded: None,
        focused: Some(true),
        modal: None,
        multiline: None,
        multiselectable: None,
        readonly: None,
        required: None,
        selected: None,
        checked: None,
        pressed: None,
        level: None,
        valuemin: None,
        valuemax: None,
        autocomplete: None,
        haspopup: None,
        invalid: None,
        orientation: None,
        children: Vec::new(),
    });
    assert_eq!(snapshot, input_response);
    let snapshot = ac
        .snapshot_builder()
        .try_root(span)
        .unwrap()
        .clear_root()
        .interesting_only(false)
        .snapshot()
        .await
        .unwrap();
    assert_ne!(snapshot, input_response);
    close(&p).await;
}

async fn screenshot_should_work(p: &Page) {
    use playwright::api::ScreenshotType;
    let path = super::temp_dir().join("screenshot.jpg");
    p.screenshot_builder()
        .r#type(ScreenshotType::Jpeg)
        .clear_type()
        .path(path.clone())
        .screenshot()
        .await
        .unwrap();
    assert!(path.is_file());
}

async fn pdf_should_work(p: &Page) {
    let path = super::temp_dir().join("pdf.pdf");
    p.pdf_builder().path(path.clone()).pdf().await.unwrap();
    assert!(path.is_file());
}

async fn emulate_media(p: &Page) {
    use playwright::api::page::Media;
    let screen = || async {
        p.eval::<bool>("() => matchMedia('screen').matches")
            .await
            .unwrap()
    };
    let print = || async {
        p.eval::<bool>("() => matchMedia('print').matches")
            .await
            .unwrap()
    };
    assert!(screen().await);
    assert!(!print().await);
    p.emulate_media_builder()
        .media(Media::Print)
        .emulate_media()
        .await
        .unwrap();
    assert!(!screen().await);
    assert!(print().await);
    p.emulate_media_builder().emulate_media().await.unwrap();
    assert!(!screen().await);
    assert!(print().await);
    p.emulate_media_builder()
        .media(Media::Null)
        .emulate_media()
        .await
        .unwrap();
    assert!(screen().await);
    assert!(!print().await);
}

async fn check_should_work(c: &BrowserContext) {
    let p = new(c).await;
    p.set_content_builder(r#"<input type="checkbox" />"#)
        .set_content()
        .await
        .unwrap();
    p.check_builder("input").check().await.unwrap();
    let checked = p.is_checked("input", None).await.unwrap();
    assert!(checked);
    p.uncheck_builder("input").uncheck().await.unwrap();
    let checked = p.is_checked("input", None).await.unwrap();
    assert!(!checked);
    close(&p).await;
}

async fn title_should_work(p: &Page) {
    p.eval::<String>(r#"() => document.title = "foo""#)
        .await
        .unwrap();
    assert_eq!(p.title().await.unwrap(), "foo");
}

async fn pointer(c: &BrowserContext) {
    let p = new(c).await;
    p.set_content_builder(r#"<input type="checkbox" />"#)
        .set_content()
        .await
        .unwrap();
    let checked = || async {
        p.eval::<bool>("() => document.querySelector('input').checked")
            .await
            .unwrap()
    };
    p.tap_builder("input").tap().await.unwrap();
    assert!(checked().await);
    p.dblclick_builder("input").dblclick().await.unwrap();
    assert!(checked().await);
    p.click_builder("input").click().await.unwrap();
    assert!(!checked().await);
    close(&p).await;
}

async fn new(c: &BrowserContext) -> Page {
    let page = c.new_page().await.unwrap();
    set_timeout(&page).await;
    page
}

async fn close(p: &Page) {
    p.close(None).await.unwrap()
}

async fn input(c: &BrowserContext) {
    let p = new(c).await;
    done!(p
        .set_content_builder(r#"<input type="text" value="" />"#)
        .set_content());
    assert_eq!(
        done!(p.get_attribute("input", "type", None)).as_deref(),
        Some("text")
    );
    done!(p.fill_builder("input", "foo").fill());
    // assert_eq!(
    //    done!(p.get_attribute("input", "value", None)).as_deref(),
    //    Some("foo")
    //);
    // TODO
    close(&p).await;
}

async fn context_pages_visibility(c: &BrowserContext) {
    let page = new(c).await;
    let pages = c.pages().unwrap();
    assert!(pages.contains(&page));
    close(&page).await;
    let pages_after = c.pages().unwrap();
    assert!(!pages_after.contains(&page));
}

async fn reject_promises_when_page_closed(c: &BrowserContext) {
    let page = new(c).await;
    close(&page).await;
    let res: Result<i32, _> = page.eval("() => new Promise(r => {})").await;
    assert!(res.is_err());
}

async fn close_callable_twice(c: &BrowserContext) {
    let page = new(c).await;
    page.close(None).await.unwrap();
    // Closing again should be a no-op.
    page.close(None).await.unwrap();
}

async fn beforeunload_runs_when_asked(c: &BrowserContext, port: u16) {
    let page = new(c).await;
    let url = super::url_static(port, "/beforeunload.html");
    page.goto_builder(&url).goto().await.unwrap();
    // Ensure we interacted so handler can fire.
    page.click_builder("body").click().await.ok();
    let dialog_fut = page.expect_event(page::EventType::Dialog);
    let close_fut = page.close(Some(true));
    let (dialog, close_res) = tokio::join!(dialog_fut, close_fut);
    close_res.unwrap();
    match dialog.unwrap() {
        page::Event::Dialog(_) => {}
        _ => panic!("expected dialog beforeunload"),
    }
}

async fn beforeunload_not_run_by_default(c: &BrowserContext, port: u16) {
    let page = new(c).await;
    let url = super::url_static(port, "/beforeunload.html");
    page.goto_builder(&url).goto().await.unwrap();
    page.click_builder("body").click().await.ok();
    let dialog_fut = page.expect_event(page::EventType::Dialog);
    // Close without runBeforeUnload should not emit dialog; expect timeout.
    page.close(None).await.unwrap();
    let timed = timeout(Duration::from_millis(500), dialog_fut).await;
    assert!(
        timed.is_err(),
        "dialog should not fire without runBeforeUnload"
    );
}

async fn page_close_state(c: &BrowserContext) {
    let page = new(c).await;
    let mut rx = page.subscribe_event().unwrap();
    page.close(None).await.unwrap();
    let mut saw_close = false;
    while let Some(Ok(evt)) = timeout(Duration::from_secs(1), rx.next())
        .await
        .ok()
        .flatten()
    {
        if let page::Event::Close = evt {
            saw_close = true;
            break;
        }
    }
    assert!(saw_close, "close event should be emitted");
}

async fn page_url_should_work(c: &BrowserContext, port: u16) {
    let page = new(c).await;
    assert_eq!(page.url().unwrap(), "about:blank");
    let url = super::url_static(port, "/empty.html");
    page.goto_builder(&url).goto().await.unwrap();
    assert_eq!(page.url().unwrap(), url);
    close(&page).await;
}

async fn load_events_should_fire(page: &Page, port: u16) {
    let mut rx = page.subscribe_event().unwrap();
    let url = super::url_static(port, "/empty.html");
    page.goto_builder(&url).goto().await.unwrap();
    let evt = timeout(Duration::from_secs(5), rx.next())
        .await
        .ok()
        .flatten()
        .and_then(Result::ok);
    assert!(matches!(evt, Some(page::Event::Load)));
}

async fn domcontentloaded_event_should_fire(page: &Page, port: u16) {
    let mut rx = page.subscribe_event().unwrap();
    let url = super::url_static(port, "/empty.html");
    page.goto_builder(&url).goto().await.unwrap();
    // wait for both domcontentloaded and load; ensure domcontentloaded shows up
    let mut saw_dcl = false;
    for _ in 0..3 {
        if let Some(Ok(evt)) = timeout(Duration::from_secs(5), rx.next())
            .await
            .ok()
            .flatten()
        {
            if let page::Event::DomContentLoaded = evt {
                saw_dcl = true;
                break;
            }
        }
    }
    assert!(saw_dcl, "domcontentloaded should fire");
}

async fn wait_for_load_state_should_work(page: &Page, port: u16) {
    let url = super::url_static(port, "/empty.html");
    page.goto_builder(&url).goto().await.unwrap();
    page.wait_for_load_state(None, Some(5_000.0)).await.unwrap();
    page.wait_for_load_state(Some(DocumentLoadState::DomContentLoaded), Some(5_000.0))
        .await
        .unwrap();
}

async fn wait_for_url_should_work(page: &Page, port: u16) {
    let url = super::url_static(port, "/empty.html");
    page.goto_builder(&url).goto().await.unwrap();
    page.wait_for_url(&url, Some(DocumentLoadState::Load), Some(5_000.0))
        .await
        .unwrap();
    // Hash change should also be observable
    page.eval::<()>("() => { window.location.hash = 'dynamic'; }")
        .await
        .unwrap();
    let target = format!("{url}#dynamic");
    page.wait_for_url(&target, Some(DocumentLoadState::Commit), Some(5_000.0))
        .await
        .unwrap();
}

async fn dialog_should_fire(page: &Page) {
    let dialog = page.expect_event(page::EventType::Dialog);
    page.eval::<()>("() => alert('yo')").await.unwrap();
    match dialog.await.unwrap() {
        page::Event::Dialog(dialog) => {
            assert_eq!(dialog.r#type().unwrap(), "alert");
            assert_eq!(dialog.default_value().unwrap(), "");
            assert_eq!(dialog.message().unwrap(), "yo");
            dialog.accept(None).await.unwrap();
        }
        _ => unreachable!(),
    }
}

async fn dialog_accept_prompt(page: &Page) {
    let dialog = page.expect_event(page::EventType::Dialog);
    let handle_dialog = async move {
        match dialog.await.unwrap() {
            page::Event::Dialog(dialog) => {
                assert_eq!(dialog.r#type().unwrap(), "prompt");
                assert_eq!(dialog.default_value().unwrap(), "yes.");
                assert_eq!(dialog.message().unwrap(), "question?");
                dialog.accept(Some("answer!")).await.unwrap();
            }
            _ => unreachable!(),
        }
    };
    let (res, _) = tokio::join!(
        async {
            page.eval::<String>("() => prompt('question?', 'yes.')")
                .await
                .unwrap()
        },
        handle_dialog
    );
    assert_eq!(res, "answer!");
}

async fn dialog_dismiss_prompt(page: &Page) {
    let dialog = page.expect_event(page::EventType::Dialog);
    let handle_dialog = async move {
        if let page::Event::Dialog(dialog) = dialog.await.unwrap() {
            dialog.dismiss().await.unwrap();
        }
    };
    let (res, _) = tokio::join!(
        async { page.eval::<Option<String>>("() => prompt('question?')").await.unwrap() },
        handle_dialog
    );
    assert!(res.is_none());
}

async fn dialog_accept_confirm(page: &Page) {
    let dialog = page.expect_event(page::EventType::Dialog);
    let handle_dialog = async move {
        if let page::Event::Dialog(dialog) = dialog.await.unwrap() {
            dialog.accept(None).await.unwrap();
        }
    };
    let (res, _) = tokio::join!(
        async { page.eval::<bool>("() => confirm('boolean?')").await.unwrap() },
        handle_dialog
    );
    assert!(res);
}

async fn dialog_dismiss_confirm(page: &Page) {
    let dialog = page.expect_event(page::EventType::Dialog);
    let handle_dialog = async move {
        if let page::Event::Dialog(dialog) = dialog.await.unwrap() {
            dialog.dismiss().await.unwrap();
        }
    };
    let (res, _) = tokio::join!(
        async { page.eval::<bool>("() => confirm('boolean?')").await.unwrap() },
        handle_dialog
    );
    assert!(!res);
}

async fn dialog_auto_dismiss_without_listener(page: &Page) {
    let result: Option<String> = page
        .eval("() => prompt('question?')")
        .await
        .unwrap();
    assert!(result.is_none());
    page.set_content_builder("<div onclick='window.alert(123); window._clicked=true'>Click me</div>")
        .set_content()
        .await
        .unwrap();
    page.click_builder("div").click().await.unwrap();
    let clicked: bool = page.eval("() => window._clicked").await.unwrap();
    assert!(clicked);
}

async fn opener_should_work(c: &BrowserContext) {
    let page = new(c).await;
    let (popup_evt, _) = tokio::join!(
        page.expect_event(page::EventType::Popup),
        page.eval::<()>("() => window.open('about:blank')")
    );
    let popup = match popup_evt.unwrap() {
        page::Event::Popup(p) => p,
        _ => unreachable!(),
    };
    let opener = popup.opener().await.unwrap();
    assert_eq!(opener.as_ref(), Some(&page));
    close(&popup).await;
    close(&page).await;
}

async fn opener_should_be_null_after_parent_close(c: &BrowserContext) {
    let page = new(c).await;
    let (popup_evt, _) = tokio::join!(
        page.expect_event(page::EventType::Popup),
        page.eval::<()>("() => window.open('about:blank')")
    );
    let popup = match popup_evt.unwrap() {
        page::Event::Popup(p) => p,
        _ => unreachable!(),
    };
    page.close(None).await.unwrap();
    let opener = popup.opener().await.unwrap();
    assert!(opener.is_none());
    close(&popup).await;
}

async fn page_url_should_include_hashes(c: &BrowserContext, port: u16) {
    let page = new(c).await;
    let url = super::url_static(port, "/empty.html");
    page.goto_builder(&url).goto().await.unwrap();
    assert_eq!(page.url().unwrap(), url);
    page.eval::<()>("() => { window.location.hash = 'dynamic'; }")
        .await
        .unwrap();
    assert_eq!(page.url().unwrap(), format!("{url}#dynamic"));
    close(&page).await;
}

async fn set_extra_http_headers(c: &BrowserContext, port: u16) {
    let p = new(c).await;
    p.set_extra_http_headers(vec![("hoge".into(), "hoge".into())])
        .await
        .unwrap();
    let url = super::url_static(port, "/empty.html");
    let (maybe_request, _) = tokio::join!(
        p.expect_event(page::EventType::Request),
        p.goto_builder(&url).goto()
    );
    let req = match maybe_request.unwrap() {
        page::Event::Request(req) => req,
        _ => unreachable!(),
    };
    let headers = req.headers().unwrap();
    assert_eq!(headers.get("foo").unwrap(), "bar"); // set by BrowserContext
    assert_eq!(headers.get("hoge").unwrap(), "hoge");
    close(&p).await;
}

async fn query_selector_and_eval(c: &BrowserContext) {
    let p = new(c).await;
    p.set_content_builder(r#"<div><h1>foo</h1><div class="foo">bar</div></div>"#)
        .set_content()
        .await
        .unwrap();
    let (wait, _) = tokio::join!(
        p.wait_for_selector_builder("div.foo > div")
            .wait_for_selector(),
        p.eval::<()>(
            "() => {
                const div = document.createElement('div');
                div.innerText = 'not blank';
                document.querySelector('div.foo').appendChild(div);
            }"
        )
    );
    let found = wait.unwrap().unwrap();
    let handle = done!(
        p.evaluate_element_handle::<()>("() => document.querySelector('div.foo > div')", None)
    );
    let divs = p.query_selector_all("div").await.unwrap();
    assert_eq!(divs.len(), 3);
    assert_eq!(
        handle.inner_html().await.unwrap(),
        found.inner_html().await.unwrap()
    );
    assert_eq!(
        divs[2].inner_html().await.unwrap(),
        found.inner_html().await.unwrap()
    );
    assert_eq!(
        p.evaluate_on_selector::<(), String>("div.foo > div", "e => e.innerHTML", None)
            .await
            .unwrap(),
        found.inner_html().await.unwrap()
    );
    assert_eq!(
        p.evaluate_on_selector_all::<(), String>("div", "es => es[2].innerHTML", None)
            .await
            .unwrap(),
        found.inner_html().await.unwrap()
    );
    assert_eq!(
        p.inner_html("div.foo > div", None).await.unwrap(),
        found.inner_html().await.unwrap()
    );
    assert_eq!(
        p.text_content("div.foo > div", None)
            .await
            .unwrap()
            .unwrap(),
        found.inner_html().await.unwrap()
    );
    close(&p).await;
}

// async fn file_chooser(c: &BrowserContext, port: u16) {
//    let p = new(c).await;
//    let url = super::url_static(port, "/form.html");
//    p.goto_builder(&url).goto().await.unwrap();
//    let (maybe_file_chooser, _) = tokio::join!(
//        p.expect_event(page::EventType::FileChooser),
//        p.click_builder("input[type=file]").click()
//    );
//    let file_chooser = match maybe_file_chooser.unwrap() {
//        page::Event::FileChooser(file_chooser) => file_chooser,
//        _ => unreachable!()
//    };
//    assert_eq!(file_chooser.page(), p);
//    assert!(file_chooser.is_multiple());
//    assert_eq!(
//        file_chooser.element(),
//        p.query_selector("input[type=file]").await.unwrap().unwrap()
//    );
//    file_chooser
//        .set_input_files_builder(File {
//            name: "a".into(),
//            mime: "text/plain".into(),
//            buffer: "a\n".into()
//        })
//        .add_file(File {
//            name: "b".into(),
//            mime: "text/plain".into(),
//            buffer: "b\n".into()
//        })
//        .set_input_files()
//        .await
//        .unwrap();
//    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
//    close(&p).await;
//}
