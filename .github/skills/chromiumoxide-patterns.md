# Skill: chromiumoxide CDP Patterns

## When to Use

Use this skill when:

- Working with `chromiumoxide` API for browser automation
- Need to understand CDP (Chrome DevTools Protocol) commands available
- Implementing actions that need low-level browser control
- Debugging browser interaction issues

## chromiumoxide Basics

`chromiumoxide` is a Rust CDP client. It provides:

- `Browser` — launches and manages Chromium processes
- `Page` — represents a browser tab/page
- `Element` — represents a DOM element found via selector

### Launching a Browser

```rust
use chromiumoxide::{Browser, BrowserConfig};

let config = BrowserConfig::builder()
    .chrome_executable("/path/to/chrome")
    .headless_mode(HeadlessMode::New)
    .window_size(1920, 1080)
    .viewport(Some(Viewport { width: 1920, height: 1080, .. }))
    .args(["--no-sandbox", "--disable-setuid-sandbox"])
    .build()?;

let (browser, mut handler) = Browser::launch(config).await?;

// Handler must be polled in background
tokio::spawn(async move {
    while let Some(h) = handler.next().await {
        if h.is_err() { break; }
    }
});
```

### Page Operations

```rust
// Navigate
let page = browser.new_page("https://example.com").await?;
page.goto("https://other.com").await?;

// Wait for navigation
page.wait_for_navigation().await?;

// Get URL
let url: Option<String> = page.url().await?;

// Execute JavaScript
let result = page.evaluate("document.title").await?;
let title: String = result.into_value()?;

// Execute JavaScript with complex return types
let result = page.evaluate("JSON.stringify({a:1})").await?;
let json: serde_json::Value = serde_json::from_str(&result.into_value::<String>()?)?;

// Screenshot
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::page::ScreenshotParams;

let params = ScreenshotParams::builder()
    .format(CaptureScreenshotFormat::Png)
    .full_page(true)
    .omit_background(true)
    .build();
let png_bytes = page.screenshot(params).await?;

// Get cookies
let cookies = page.get_cookies().await?;

// Set cookies
use chromiumoxide::cdp::browser_protocol::network::CookieParam;
let cookie = CookieParam::builder()
    .name("session")
    .value("abc123")
    .domain("example.com")
    .build()?;
page.set_cookie(cookie).await?;
```

### Element Operations

```rust
// Find element (fails immediately if not found)
let element = page.find_element("css selector").await?;

// Find multiple elements
let elements = page.find_elements("css selector").await?;

// Click
element.click().await?;

// Type text (simulates keypresses)
element.type_str("hello world").await?;

// Focus
element.focus().await?;

// Hover
element.hover().await?;

// Press key
element.press_key("Enter").await?;

// Scroll into view
element.scroll_into_view().await?;

// Call JS function on element
element.call_js_fn("function() { this.value = ''; }", false).await?;

// Get property
let node_id = element.node_id;
```

### Raw CDP Commands

For operations not directly exposed by the high-level API:

```rust
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchMouseEventParams, MouseButton, DispatchMouseEventType,
    DispatchKeyEventParams, DispatchKeyEventType,
};

// Mouse move
let cmd = DispatchMouseEventParams::builder()
    .r#type(DispatchMouseEventType::MouseMoved)
    .x(100.0)
    .y(200.0)
    .build()?;
page.execute(cmd).await?;

// Mouse click (press + release)
let press = DispatchMouseEventParams::builder()
    .r#type(DispatchMouseEventType::MousePressed)
    .x(100.0)
    .y(200.0)
    .button(MouseButton::Left)
    .click_count(1)
    .build()?;
page.execute(press).await?;

let release = DispatchMouseEventParams::builder()
    .r#type(DispatchMouseEventType::MouseReleased)
    .x(100.0)
    .y(200.0)
    .button(MouseButton::Left)
    .click_count(1)
    .build()?;
page.execute(release).await?;

// Key event
let key_down = DispatchKeyEventParams::builder()
    .r#type(DispatchKeyEventType::KeyDown)
    .key("Enter")
    .code("Enter")
    .windows_virtual_key_code(13)
    .build()?;
page.execute(key_down).await?;

// File upload via CDP
use chromiumoxide::cdp::browser_protocol::dom::SetFileInputFilesParams;
let cmd = SetFileInputFilesParams::builder()
    .files(vec!["/absolute/path/to/file.txt".to_string()])
    .node_id(element.node_id)
    .build()?;
page.execute(cmd).await?;
```

### Getting Element Bounding Box

chromiumoxide doesn't have a built-in bounding box method, so use JS:

```rust
let script = format!(
    r#"(() => {{
        const el = document.querySelector('{}');
        if (!el) return null;
        const rect = el.getBoundingClientRect();
        return {{ x: rect.x, y: rect.y, width: rect.width, height: rect.height }};
    }})()"#,
    selector.replace('\'', "\\'")
);
let result = page.evaluate(script).await?;
// Parse result as { x, y, width, height }
```

### Frame Handling

chromiumoxide doesn't have native frame navigation like Playwright. Use JS:

```rust
// Build a JS expression that traverses into nested frames
fn build_frame_document_js(selectors: &[String]) -> String {
    let mut js = "document".to_string();
    for sel in selectors {
        js = format!("{}.querySelector('{}').contentDocument", js, sel.replace('\'', "\\'"));
    }
    js
}

// Then use it to query elements inside frames:
let doc = build_frame_document_js(&frame_selectors);
let script = format!("{}.querySelector('{}')", doc, element_selector);
page.evaluate(script).await?;
```

**Limitation:** For CDP-level actions (mouse events) inside frames, you need the frame's position offset in the parent to calculate correct coordinates.

## Common Pitfalls

1. **`find_element` doesn't wait** — Unlike Playwright's `waitForSelector`, this fails immediately.
   Implement your own polling loop.

2. **`element.click()` uses JS** — chromiumoxide's `.click()` calls JS `el.click()`,
   which bypasses overlay detection. For real user simulation, use CDP `Input.dispatchMouseEvent`.

3. **`element.hover()` behavior** — Verify if this uses CDP mouse events or JS dispatch.
   May need to use raw CDP for proper CSS `:hover` activation.

4. **No built-in actionability checks** — Unlike Playwright, chromiumoxide doesn't check
   visibility, stability, or interactability before actions. You must implement these.

5. **Handler must be polled** — The `handler` stream must be consumed or the browser will freeze.
   Always spawn it in a background task.

6. **Serialization of CDP types** — Use the builder pattern for CDP command params.
   Don't construct them manually.
