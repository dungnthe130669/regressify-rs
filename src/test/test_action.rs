use std::path::Path;

use anyhow::{Result, anyhow};
use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::dom::SetFileInputFilesParams;

use crate::{
    common::ROOT_DIR,
    test::capture::save_screenshot,
    types::{
        CheckAction, ClickAction, FileInput, FocusAction, Frame, GotoAction, HideAction,
        HoverAction, InputAction, PersistAction, PressAction, RemoveAction, SaveAction,
        ScreenshotType, ScrollAction, SelectAction, SelectValue, TestAction, TestScenario,
        TestSuite, UncheckAction, Wait, WaitAction,
    },
};

/// Resolve the frame chain: navigate into nested iframes and return
/// a JS expression prefix that targets the innermost frame's document.
/// Returns `None` if there is no frame, meaning actions run on the main page.
fn get_frame_selectors(frame: &Option<Frame>) -> Option<Vec<String>> {
    match frame {
        Some(Frame::Single(s)) => Some(vec![s.clone()]),
        Some(Frame::Multiple(v)) => Some(v.clone()),
        None => None,
    }
}

/// Build a JS expression that navigates into nested iframes and returns
/// the target document at the deepest level.
fn build_frame_document_js(selectors: &[String]) -> String {
    let mut js = "document".to_string();
    for sel in selectors {
        js = format!(
            "{}.querySelector('{}').contentDocument",
            js,
            sel.replace('\'', "\\'")
        );
    }
    js
}

/// Resolve file paths from a FileInput, validating they are relative and exist.
fn resolve_file_paths(file: &FileInput, visual_tests_dir: &Path) -> Result<Vec<String>> {
    let files = match file {
        FileInput::Single(f) => vec![f.clone()],
        FileInput::Multiple(v) => v.clone(),
    };

    let mut paths = Vec::new();
    for f in &files {
        if Path::new(f).is_absolute() {
            anyhow::bail!("`file` must be a relative path to the \"visual_tests\" folder");
        }
        let full_path = visual_tests_dir.join(f);
        if !full_path.exists() {
            anyhow::bail!("file does not exist: {}", f);
        }
        paths.push(
            full_path
                .to_str()
                .ok_or_else(|| anyhow!("Invalid file path"))?
                .to_string(),
        );
    }
    Ok(paths)
}

pub async fn exec_test_action(
    page: &Page,
    test_suite: &TestSuite,
    scenario: &TestScenario,
    action: &TestAction,
) -> Result<()> {
    match action {
        TestAction::Check(CheckAction { check, frame }) => {
            eprintln!("    Check element: {}", check);
            if let Some(selectors) = get_frame_selectors(frame) {
                let doc = build_frame_document_js(&selectors);
                let script = format!(
                    "(() => {{ let el = {doc}.querySelector('{sel}'); el.click(); }})()",
                    doc = doc,
                    sel = check.replace('\'', "\\'")
                );
                page.evaluate(script).await?;
            } else {
                page.find_element(check).await?.click().await?;
            }
        }

        TestAction::Click(ClickAction { click, frame }) => {
            eprintln!("    Clicking element: {}", click);
            if let Some(selectors) = get_frame_selectors(frame) {
                let doc = build_frame_document_js(&selectors);
                let script = format!(
                    "(() => {{ {doc}.querySelector('{sel}').click(); }})()",
                    doc = doc,
                    sel = click.replace('\'', "\\'")
                );
                page.evaluate(script).await?;
            } else {
                page.find_element(click).await?.click().await?;
            }
        }

        TestAction::Focus(FocusAction { focus, frame }) => {
            eprintln!("    Focusing element: {}", focus);
            if let Some(selectors) = get_frame_selectors(frame) {
                let doc = build_frame_document_js(&selectors);
                let script = format!(
                    "(() => {{ {doc}.querySelector('{sel}').focus(); }})()",
                    doc = doc,
                    sel = focus.replace('\'', "\\'")
                );
                page.evaluate(script).await?;
            } else {
                page.find_element(focus).await?.focus().await?;
            }
        }

        TestAction::Goto(GotoAction { goto, .. }) => {
            eprintln!("    Navigating to URL: {}", goto);
            page.goto(goto).await?;
        }

        TestAction::Hide(HideAction { hide, frame }) => {
            eprintln!("    Hiding element: {}", hide);
            if let Some(selectors) = get_frame_selectors(frame) {
                let doc = build_frame_document_js(&selectors);
                let script = format!(
                    "(() => {{
                        var d = {doc};
                        var style = d.createElement('style');
                        style.type = 'text/css';
                        style.innerHTML = '{sel} {{ visibility: hidden !important; }}';
                        d.head.appendChild(style);
                    }})()",
                    doc = doc,
                    sel = hide.replace('\'', "\\'")
                );
                page.evaluate(script).await?;
            } else {
                let script = format!(
                    "(() => {{
                        var style = document.createElement('style');
                        style.type = 'text/css';
                        style.innerHTML = '{} {{ visibility: hidden !important; }}';
                        document.head.appendChild(style);
                    }})()",
                    hide
                );
                page.evaluate(script.as_str()).await?;
            }
        }

        TestAction::Hover(HoverAction { hover, frame }) => {
            eprintln!("    Hovering over element: {}", hover);
            if let Some(selectors) = get_frame_selectors(frame) {
                let doc = build_frame_document_js(&selectors);
                let script = format!(
                    "(() => {{
                        var el = {doc}.querySelector('{sel}');
                        var evt = new MouseEvent('mouseover', {{ bubbles: true, cancelable: true }});
                        el.dispatchEvent(evt);
                    }})()",
                    doc = doc,
                    sel = hover.replace('\'', "\\'")
                );
                page.evaluate(script).await?;
            } else {
                page.find_element(hover).await?.hover().await?;
            }
        }

        TestAction::Input(InputAction {
            input,
            value,
            append,
            file,
            use_file_chooser,
            frame,
        }) => {
            if value.is_some() && file.is_some() {
                anyhow::bail!("`input` action must not contain both `value` and `file`");
            }

            if let Some(value) = value {
                eprintln!("    Typing into element: {} value: {}", input, value);
                if let Some(selectors) = get_frame_selectors(frame) {
                    let doc = build_frame_document_js(&selectors);
                    let append = append.unwrap_or(false);
                    let clear_js = if !append {
                        format!(
                            "{doc}.querySelector('{sel}').value = '';",
                            doc = doc,
                            sel = input.replace('\'', "\\'")
                        )
                    } else {
                        String::new()
                    };
                    // For frame input, we use native input events via JS
                    let script = format!(
                        "(() => {{
                            var el = {doc}.querySelector('{sel}');
                            {clear}
                            el.focus();
                            var nativeInputValueSetter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value') || Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value');
                            if (nativeInputValueSetter) {{
                                nativeInputValueSetter.set.call(el, el.value + '{val}');
                            }} else {{
                                el.value = el.value + '{val}';
                            }}
                            el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                            el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                        }})()",
                        doc = doc,
                        sel = input.replace('\'', "\\'"),
                        clear = clear_js,
                        val = value.replace('\'', "\\'").replace('\n', "\\n")
                    );
                    page.evaluate(script).await?;
                } else {
                    let element = page.find_element(input).await?;
                    if !append.unwrap_or(false) {
                        // Clear existing value before typing
                        element
                            .call_js_fn("function() { this.value = ''; }", false)
                            .await?;
                    }
                    element.type_str(value).await?;
                }
            } else if let Some(file) = file {
                eprintln!("    Uploading file to element: {}", input);
                let root_dir = ROOT_DIR
                    .get()
                    .ok_or_else(|| anyhow!("ROOT_DIR not initialized"))?;
                let visual_tests_dir = root_dir.join("visual_tests");
                let file_paths = resolve_file_paths(file, &visual_tests_dir)?;

                let element = page.find_element(input).await?;

                if use_file_chooser.unwrap_or(false) {
                    // Click to trigger file chooser, then set files via CDP
                    element.click().await?;
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }

                // Use CDP DOM.setFileInputFiles to set files on the input
                let cmd = SetFileInputFilesParams::builder()
                    .files(file_paths)
                    .node_id(element.node_id)
                    .build()
                    .map_err(|e| anyhow!("Failed to build SetFileInputFilesParams: {}", e))?;
                page.execute(cmd).await?;
            } else {
                anyhow::bail!("`input` action must have either `value` or `file`");
            }
        }

        TestAction::Persist(PersistAction { persist, .. }) => {
            eprintln!("    Persisting state: {}", persist);
            let root_dir = ROOT_DIR
                .get()
                .ok_or_else(|| anyhow!("ROOT_DIR not initialized"))?;

            // Collect cookies
            let cookies = page.get_cookies().await?;
            let cookies_json = serde_json::to_value(&cookies)?;

            // Collect localStorage
            let local_storage_result = page
                .evaluate("JSON.stringify(Object.fromEntries(Object.entries(localStorage)))")
                .await?;
            let local_storage_str = local_storage_result
                .into_value::<String>()
                .unwrap_or_default();
            let local_storage: serde_json::Value = serde_json::from_str(&local_storage_str)
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

            let state = serde_json::json!({
                "cookies": cookies_json,
                "localStorage": local_storage,
            });

            let state_dir = root_dir.join(".regressify").join("states");
            std::fs::create_dir_all(&state_dir)?;
            let state_path = state_dir.join(format!("{}.json", persist));
            std::fs::write(&state_path, serde_json::to_string_pretty(&state)?)?;
            eprintln!("    State saved to: {}", state_path.display());
        }

        TestAction::Press(PressAction { press, key, frame }) => {
            eprintln!("    Pressing key '{}' on element: {}", key, press);
            if let Some(selectors) = get_frame_selectors(frame) {
                let doc = build_frame_document_js(&selectors);
                let script = format!(
                    "(() => {{
                        var el = {doc}.querySelector('{sel}');
                        el.focus();
                        el.dispatchEvent(new KeyboardEvent('keydown', {{ key: '{key}', bubbles: true }}));
                        el.dispatchEvent(new KeyboardEvent('keyup', {{ key: '{key}', bubbles: true }}));
                    }})()",
                    doc = doc,
                    sel = press.replace('\'', "\\'"),
                    key = key.replace('\'', "\\'")
                );
                page.evaluate(script).await?;
            } else {
                page.find_element(press).await?.press_key(key).await?;
            }
        }

        TestAction::Remove(RemoveAction { remove, frame }) => {
            eprintln!("    Removing element: {}", remove);
            if let Some(selectors) = get_frame_selectors(frame) {
                let doc = build_frame_document_js(&selectors);
                let script = format!(
                    "(() => {{
                        var d = {doc};
                        var style = d.createElement('style');
                        style.type = 'text/css';
                        style.innerHTML = '{sel} {{ display: none !important; }}';
                        d.head.appendChild(style);
                    }})()",
                    doc = doc,
                    sel = remove.replace('\'', "\\'")
                );
                page.evaluate(script).await?;
            } else {
                let script = format!(
                    "(() => {{
                        var style = document.createElement('style');
                        style.type = 'text/css';
                        style.innerHTML = '{} {{ display: none !important; }}';
                        document.head.appendChild(style);
                    }})()",
                    remove
                );
                page.evaluate(script.as_str()).await?;
            }
        }

        TestAction::Save(SaveAction { save, .. }) => {
            eprintln!("    Saving screenshot: {}", save);

            save_screenshot(
                test_suite,
                scenario,
                page,
                ScreenshotType::Step(save.to_string()),
            )
            .await?
        }

        TestAction::Scroll(ScrollAction { scroll, frame }) => {
            eprintln!("    Scrolling to element: {}", scroll);
            if let Some(selectors) = get_frame_selectors(frame) {
                let doc = build_frame_document_js(&selectors);
                let script = format!(
                    "(() => {{ {doc}.querySelector('{sel}').scrollIntoView(); }})()",
                    doc = doc,
                    sel = scroll.replace('\'', "\\'")
                );
                page.evaluate(script).await?;
            } else {
                page.find_element(scroll).await?.scroll_into_view().await?;
            }
        }

        TestAction::Select(SelectAction {
            select,
            value,
            label,
            frame,
        }) => {
            if value.is_some() && label.is_some() {
                anyhow::bail!("Select action must have only either `value` or `label`, not both.");
            }
            if value.is_none() && label.is_none() {
                anyhow::bail!("Select action must have either `value` or `label`.");
            }

            let doc_js = if let Some(selectors) = get_frame_selectors(frame) {
                build_frame_document_js(&selectors)
            } else {
                "document".to_string()
            };

            if let Some(select_value) = value {
                let values = match select_value {
                    SelectValue::Single(v) => vec![v.clone()],
                    SelectValue::Multiple(v) => v.clone(),
                };
                let values_json = serde_json::to_string(&values)?;
                eprintln!(
                    "    Selecting value(s) {} in element: {}",
                    values_json, select
                );

                let script = format!(
                    "(() => {{
                        var el = {doc}.querySelector('{sel}');
                        var values = {values};
                        for (var i = 0; i < el.options.length; i++) {{
                            el.options[i].selected = values.includes(el.options[i].value);
                        }}
                        el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                    }})()",
                    doc = doc_js,
                    sel = select.replace('\'', "\\'"),
                    values = values_json
                );
                page.evaluate(script).await?;
            } else if let Some(label) = label {
                eprintln!("    Selecting label '{}' in element: {}", label, select);
                let script = format!(
                    "(() => {{
                        var el = {doc}.querySelector('{sel}');
                        for (var i = 0; i < el.options.length; i++) {{
                            if (el.options[i].text === '{lbl}') {{
                                el.options[i].selected = true;
                            }} else {{
                                el.options[i].selected = false;
                            }}
                        }}
                        el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                    }})()",
                    doc = doc_js,
                    sel = select.replace('\'', "\\'"),
                    lbl = label.replace('\'', "\\'")
                );
                page.evaluate(script).await?;
            }
        }

        TestAction::Uncheck(UncheckAction { uncheck, frame }) => {
            eprintln!("    Unchecking element: {}", uncheck);
            if let Some(selectors) = get_frame_selectors(frame) {
                let doc = build_frame_document_js(&selectors);
                let script = format!(
                    "(() => {{
                        var el = {doc}.querySelector('{sel}');
                        if (el.checked) el.click();
                    }})()",
                    doc = doc,
                    sel = uncheck.replace('\'', "\\'")
                );
                page.evaluate(script).await?;
            } else {
                let element = page.find_element(uncheck).await?;
                element.click().await?;
            }
        }

        TestAction::Wait(WaitAction { wait, url, frame }) => {
            if let Some(url) = url {
                eprintln!("    Waiting for URL: {}", url);
                page.wait_for_navigation().await?;

                let current_url = page
                    .url()
                    .await?
                    .ok_or_else(|| anyhow!("Failed to get current URL"))?;

                // Support glob-style `**` matching
                if url.contains("**") {
                    let pattern = url.replace("**", "");
                    if !current_url.contains(&pattern)
                        && !current_url.ends_with(pattern.trim_start_matches('/'))
                    {
                        // Check if the suffix after ** matches the end of the URL
                        let suffix = url.split("**").last().unwrap_or("");
                        if !suffix.is_empty() && !current_url.ends_with(suffix) {
                            anyhow::bail!(
                                "Expected URL matching '{}', but got '{}'",
                                url,
                                current_url
                            );
                        }
                    }
                } else if &current_url != url {
                    anyhow::bail!("Expected URL '{}', but got '{}'", url, current_url);
                }
            }

            let doc_js = if let Some(selectors) = get_frame_selectors(frame) {
                Some(build_frame_document_js(&selectors))
            } else {
                None
            };

            match wait {
                Wait::Number(milliseconds) => {
                    eprintln!("    Waiting for {} milliseconds", milliseconds);
                    tokio::time::sleep(std::time::Duration::from_millis(*milliseconds)).await;
                }
                Wait::String(selector) => {
                    eprintln!("    Waiting for element: {}", selector);
                    if let Some(doc) = doc_js {
                        // Poll for element in iframe
                        let script = format!(
                            "{doc}.querySelector('{sel}')",
                            doc = doc,
                            sel = selector.replace('\'', "\\'")
                        );
                        let timeout = std::time::Duration::from_secs(30);
                        let start = std::time::Instant::now();
                        loop {
                            let result = page.evaluate(script.as_str()).await?;
                            if !result
                                .into_value::<serde_json::Value>()
                                .unwrap_or(serde_json::Value::Null)
                                .is_null()
                            {
                                break;
                            }
                            if start.elapsed() > timeout {
                                anyhow::bail!(
                                    "Timeout waiting for element '{}' in frame",
                                    selector
                                );
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                    } else {
                        page.find_element(selector).await?;
                    }
                }
            }
        }
    }
    Ok(())
}
