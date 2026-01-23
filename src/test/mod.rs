use anyhow::{Result, anyhow};
use chromiumoxide::{
    browser::{Browser, BrowserConfig},
    cdp::browser_protocol::page::CaptureScreenshotFormat,
    page::ScreenshotParams,
};
use futures::StreamExt;

pub async fn test() -> Result<()> {
    // create a `Browser` that spawns a `chromium` process running with UI (`with_head()`, headless is default)
    // and the handler that drives the websocket etc.
    let config = BrowserConfig::builder()
        .with_head()
        .build()
        .map_err(|e| anyhow!("Cannot create browser config: {:?}", e))?;
    let (mut browser, mut handler) = Browser::launch(config).await?;

    // spawn a new task that continuously polls the handler
    let handle = tokio::spawn(async move {
        while let Some(h) = handler.next().await {
            if h.is_err() {
                break;
            }
        }
    });

    // create a new browser page and navigate to the url
    let page = browser.new_page("https://tuyen.blog/").await?;
    page.save_screenshot(
        ScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Png)
            .full_page(true)
            .omit_background(true)
            .build(),
        "step-0.png",
    )
    .await?;

    // find and click the search toggle button to reveal the search bar
    page.find_element("#theme-toggle").await?.click().await?;
    page.save_screenshot(
        ScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Png)
            .full_page(true)
            .omit_background(true)
            .build(),
        "step-1.png",
    )
    .await?;

    // find the search bar type into the search field and hit `Enter`,
    // this triggers a new navigation to the search result page
    page.find_element("#nav-main > .menu-item[data-section=\"optimizely-cms\"] > .menu-link")
        .await?
        .click()
        .await?;

    let html = page.wait_for_navigation().await?.content().await?;
    eprint!("{}", html);
    page.save_screenshot(
        ScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Png)
            .full_page(true)
            .omit_background(true)
            .build(),
        "step-2.png",
    )
    .await?;

    browser.close().await?;
    handle.await?;
    Ok(())
}
