use std::time::Duration;

use anyhow::{Result, anyhow};
use chromiumoxide::{
    browser::{Browser, BrowserConfig},
    cdp::browser_protocol::page::CaptureScreenshotFormat,
    handler::viewport::Viewport,
    page::ScreenshotParams,
};
use futures::StreamExt;
use tokio::time::sleep;

pub async fn test() -> Result<()> {
    // create a `Browser` that spawns a `chromium` process running with UI (`with_head()`, headless is default)
    // and the handler that drives the websocket etc.
    let config = BrowserConfig::builder()
        .with_head()
        .viewport(Some(Viewport {
            width: 1920,
            height: 1080,
            device_scale_factor: None,
            emulating_mobile: false,
            is_landscape: true,
            has_touch: false,
        }))
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
    eprintln!("Navigating to https://tuyen.blog/ ...");
    let page = browser.new_page("https://tuyen.blog/").await?;

    sleep(Duration::from_secs(5)).await;

    eprintln!("Taking screenshots...");
    page.save_screenshot(
        ScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Png)
            .full_page(true)
            .omit_background(true)
            .build(),
        "step-0.png",
    )
    .await?;

    sleep(Duration::from_secs(5)).await;

    // find and click the search toggle button to reveal the search bar
    eprintln!("Toggling theme...");
    page.find_element("#theme-toggle").await?.click().await?;

    sleep(Duration::from_secs(5)).await;

    eprintln!("Taking screenshots...");
    page.save_screenshot(
        ScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Png)
            .full_page(true)
            .omit_background(true)
            .build(),
        "step-1.png",
    )
    .await?;

    sleep(Duration::from_secs(5)).await;

    // find the search bar type into the search field and hit `Enter`,
    // this triggers a new navigation to the search result page
    eprintln!("Navigating to Optimizely CMS section...");
    page.find_element("#nav-main > .menu-item[data-section=\"optimizely-cms\"] > .menu-link")
        .await?
        .click()
        .await?;

    sleep(Duration::from_secs(5)).await;

    eprintln!("Retrieving page content...");
    let html = page.wait_for_navigation().await?.content().await?;
    eprint!("{}", html);

    sleep(Duration::from_secs(5)).await;

    eprintln!("Taking screenshots...");
    page.save_screenshot(
        ScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Png)
            .full_page(true)
            .omit_background(true)
            .build(),
        "step-2.png",
    )
    .await?;

    eprintln!("Closing browser...");
    browser.close().await?;

    eprintln!("Waiting for handler to finish...");
    handle.await?;

    eprintln!("Test completed successfully.");
    Ok(())
}
