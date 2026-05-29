use std::path::Path;

use anyhow::{Result, anyhow};
use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::dom::SetFileInputFilesParams;

use crate::{
    common::ROOT_DIR,
    config::ResolvedConfig,
    test::{
        actionability::{
            self, ActionabilityConfig, build_frame_document_js, cdp_click, cdp_hover,
            check_enabled, ensure_actionable, ensure_exists, ensure_focusable, escape_css_rule,
            escape_js_string, get_frame_selectors, scroll_into_view, wait_for_selector,
        },
        capture::save_screenshot,
    },
    types::{
        CheckAction, ClickAction, FileInput, FocusAction, GotoAction, HideAction, HoverAction,
        InputAction, PersistAction, PressAction, RemoveAction, SaveAction, ScreenshotType,
        ScrollAction, SelectAction, SelectValue, TestAction, TestScenario, TestSuite,
        UncheckAction, Wait, WaitAction,
    },
};

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
        let canonical_base = visual_tests_dir.canonicalize()?;
        let canonical_path = full_path.canonicalize()?;
        if !canonical_path.starts_with(&canonical_base) {
            anyhow::bail!("file '{}' escapes the visual_tests folder", f);
        }
        paths.push(
            canonical_path
                .to_str()
                .ok_or_else(|| anyhow!("Invalid file path"))?
                .to_string(),
        );
    }
    Ok(paths)
}

/// Build an ActionabilityConfig from a ResolvedConfig.
fn actionability_config(config: &ResolvedConfig) -> ActionabilityConfig {
    ActionabilityConfig {
        element_timeout_ms: config.element_timeout_ms,
        actionability_timeout_ms: config.actionability_timeout_ms,
        ..Default::default()
    }
}

/// Map a W3C `KeyboardEvent.key` value to the corresponding `code` value.
/// Covers common cases (named keys, ASCII letters, digits, space).
/// Unknown keys fall back to the key itself.
fn key_to_code(key: &str) -> String {
    match key {
        " " => "Space".to_string(),
        "Enter" | "Escape" | "Tab" | "Backspace" | "Delete" | "Insert" | "Home" | "End"
        | "PageUp" | "PageDown" | "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight"
        | "Shift" | "Control" | "Alt" | "Meta" | "CapsLock" | "NumLock" | "ScrollLock" => {
            key.to_string()
        }
        k if k.chars().count() == 1 => {
            let c = k.chars().next().unwrap_or(' ');
            if c.is_ascii_alphabetic() {
                format!("Key{}", c.to_ascii_uppercase())
            } else if c.is_ascii_digit() {
                format!("Digit{}", c)
            } else {
                key.to_string()
            }
        }
        _ => key.to_string(),
    }
}

pub async fn exec_test_action(
    page: &Page,
    test_suite: &TestSuite,
    scenario: &TestScenario,
    action: &TestAction,
    config: &ResolvedConfig,
) -> Result<()> {
    let ac = actionability_config(config);

    match action {
        TestAction::Check(CheckAction { check, frame }) => {
            eprintln!("    Check element: {}", check);
            let frame_selectors = get_frame_selectors(frame);
            let info = ensure_actionable(page, check, frame, &ac).await?;

            // Verify it's a checkbox or radio
            let sel_escaped = escape_js_string(check);
            let doc = frame_selectors
                .as_ref()
                .map(|s| build_frame_document_js(s))
                .unwrap_or_else(|| "document".to_string());
            let type_script = format!(
                "(() => {{ var el = {doc}.querySelector('{sel}'); return el.type; }})()",
                doc = doc,
                sel = sel_escaped
            );
            let el_type: String = page
                .evaluate(type_script)
                .await?
                .into_value()
                .unwrap_or_default();
            if el_type != "checkbox" && el_type != "radio" {
                anyhow::bail!(
                    "check action requires a checkbox or radio element, got type='{}'",
                    el_type
                );
            }

            // Check if already checked
            let checked_script = format!(
                "(() => {{ return {doc}.querySelector('{sel}').checked; }})()",
                doc = doc,
                sel = sel_escaped
            );
            let is_checked: bool = page
                .evaluate(checked_script)
                .await?
                .into_value()
                .unwrap_or(false);

            if !is_checked {
                // Click to check it
                let frame_offset = actionability::get_frame_offset(page, frame).await?;
                cdp_click(
                    page,
                    frame_offset.0 + info.center_x,
                    frame_offset.1 + info.center_y,
                )
                .await?;
            }
        }

        TestAction::Click(ClickAction { click, frame }) => {
            eprintln!("    Clicking element: {}", click);
            let info = ensure_actionable(page, click, frame, &ac).await?;
            let frame_offset = actionability::get_frame_offset(page, frame).await?;
            cdp_click(
                page,
                frame_offset.0 + info.center_x,
                frame_offset.1 + info.center_y,
            )
            .await?;
        }

        TestAction::Focus(FocusAction { focus, frame }) => {
            eprintln!("    Focusing element: {}", focus);
            let frame_selectors = get_frame_selectors(frame);
            ensure_focusable(page, focus, frame, &ac).await?;

            let sel_escaped = escape_js_string(focus);
            let doc = frame_selectors
                .as_ref()
                .map(|s| build_frame_document_js(s))
                .unwrap_or_else(|| "document".to_string());
            let script = format!(
                "(() => {{ {doc}.querySelector('{sel}').focus(); }})()",
                doc = doc,
                sel = sel_escaped
            );
            page.evaluate(script).await?;
        }

        TestAction::Goto(GotoAction { goto, .. }) => {
            eprintln!("    Navigating to URL: {}", goto);
            page.goto(goto).await?;
        }

        TestAction::Hide(HideAction { hide, frame }) => {
            eprintln!("    Hiding element: {}", hide);
            let frame_selectors = get_frame_selectors(frame);
            let css_escaped = escape_css_rule(hide);

            if let Some(selectors) = &frame_selectors {
                let doc = build_frame_document_js(selectors);
                let script = format!(
                    "(() => {{
                        var d = {doc};
                        var style = d.createElement('style');
                        style.type = 'text/css';
                        style.innerHTML = '{sel} {{ visibility: hidden !important; }}';
                        d.head.appendChild(style);
                    }})()",
                    doc = doc,
                    sel = css_escaped
                );
                page.evaluate(script).await?;
            } else {
                let script = format!(
                    "(() => {{
                        var style = document.createElement('style');
                        style.type = 'text/css';
                        style.innerHTML = '{sel} {{ visibility: hidden !important; }}';
                        document.head.appendChild(style);
                    }})()",
                    sel = css_escaped
                );
                page.evaluate(script.as_str()).await?;
            }
        }

        TestAction::Hover(HoverAction { hover, frame }) => {
            eprintln!("    Hovering over element: {}", hover);
            let info = ensure_actionable(page, hover, frame, &ac).await?;
            let frame_offset = actionability::get_frame_offset(page, frame).await?;
            cdp_hover(
                page,
                frame_offset.0 + info.center_x,
                frame_offset.1 + info.center_y,
            )
            .await?;
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

            let frame_selectors = get_frame_selectors(frame);

            if let Some(value) = value {
                eprintln!("    Typing into element: {} value: {}", input, value);
                ensure_focusable(page, input, frame, &ac).await?;
                check_enabled(page, input, frame).await?;

                let sel_escaped = escape_js_string(input);
                let val_escaped = escape_js_string(value);
                let doc = frame_selectors
                    .as_ref()
                    .map(|s| build_frame_document_js(s))
                    .unwrap_or_else(|| "document".to_string());

                let append_mode = append.unwrap_or(false);
                let clear_js = if !append_mode {
                    format!(
                        "{doc}.querySelector('{sel}').value = '';",
                        doc = doc,
                        sel = sel_escaped
                    )
                } else {
                    String::new()
                };

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
                    sel = sel_escaped,
                    clear = clear_js,
                    val = val_escaped
                );
                page.evaluate(script).await?;
            } else if let Some(file) = file {
                eprintln!("    Uploading file to element: {}", input);
                if frame.is_some() {
                    anyhow::bail!(
                        "input file upload inside iframes is not supported yet; selector '{}' has a frame configured",
                        input
                    );
                }
                let root_dir = ROOT_DIR
                    .get()
                    .ok_or_else(|| anyhow!("ROOT_DIR not initialized"))?;
                let visual_tests_dir = root_dir.join("visual_tests");
                let file_paths = resolve_file_paths(file, &visual_tests_dir)?;

                let info = ensure_actionable(page, input, frame, &ac).await?;

                let sel_escaped = escape_js_string(input);
                let type_script = format!(
                    "(() => {{ var el = document.querySelector('{sel}'); return el ? el.type : ''; }})()",
                    sel = sel_escaped
                );
                let input_type: String = page
                    .evaluate(type_script)
                    .await?
                    .into_value()
                    .unwrap_or_default();
                if input_type != "file" {
                    anyhow::bail!(
                        "input file upload requires an <input type='file'> element, got type='{}'",
                        input_type
                    );
                }

                let element = page.find_element(input).await?;

                if use_file_chooser.unwrap_or(false) {
                    cdp_click(page, info.center_x, info.center_y).await?;
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }

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
            // Validate persist name doesn't contain path separators
            if persist.contains('/') || persist.contains('\\') || persist.contains("..") {
                anyhow::bail!(
                    "Invalid persist name '{}': must not contain path separators or '..'",
                    persist
                );
            }
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
            let frame_selectors = get_frame_selectors(frame);
            ensure_focusable(page, press, frame, &ac).await?;

            let sel_escaped = escape_js_string(press);
            let key_escaped = escape_js_string(key);
            let code_escaped = escape_js_string(&key_to_code(key));
            let doc = frame_selectors
                .as_ref()
                .map(|s| build_frame_document_js(s))
                .unwrap_or_else(|| "document".to_string());

            // Focus the element (in-frame via JS), then dispatch key events.
            // We dispatch synthetic JS events that include the correct W3C `code`
            // so framework handlers see realistic values.
            let script = format!(
                "(() => {{
                    var el = {doc}.querySelector('{sel}');
                    el.focus();
                    var init = {{ key: '{key}', code: '{code}', bubbles: true, cancelable: true }};
                    el.dispatchEvent(new KeyboardEvent('keydown', init));
                    el.dispatchEvent(new KeyboardEvent('keypress', init));
                    el.dispatchEvent(new KeyboardEvent('keyup', init));
                }})()",
                doc = doc,
                sel = sel_escaped,
                key = key_escaped,
                code = code_escaped
            );
            page.evaluate(script).await?;

            // Also dispatch a native CDP key event so the browser sees real input
            // (e.g. form submission on Enter, Tab navigation). Only at the top
            // frame — nested-frame native key targeting requires execution
            // context resolution that chromiumoxide doesn't expose simply.
            if frame.is_none() {
                use chromiumoxide::cdp::browser_protocol::input::{
                    DispatchKeyEventParams, DispatchKeyEventType,
                };
                let key_str = key.clone();
                let code_str = key_to_code(key);
                let down = DispatchKeyEventParams::builder()
                    .r#type(DispatchKeyEventType::KeyDown)
                    .key(key_str.clone())
                    .code(code_str.clone())
                    .build()
                    .map_err(|e| anyhow!("Failed to build key down event: {:?}", e))?;
                page.execute(down).await?;
                let up = DispatchKeyEventParams::builder()
                    .r#type(DispatchKeyEventType::KeyUp)
                    .key(key_str)
                    .code(code_str)
                    .build()
                    .map_err(|e| anyhow!("Failed to build key up event: {:?}", e))?;
                page.execute(up).await?;
            }
        }

        TestAction::Remove(RemoveAction { remove, frame }) => {
            eprintln!("    Removing element: {}", remove);
            let frame_selectors = get_frame_selectors(frame);
            let css_escaped = escape_css_rule(remove);

            if let Some(selectors) = &frame_selectors {
                let doc = build_frame_document_js(selectors);
                let script = format!(
                    "(() => {{
                        var d = {doc};
                        var style = d.createElement('style');
                        style.type = 'text/css';
                        style.innerHTML = '{sel} {{ display: none !important; }}';
                        d.head.appendChild(style);
                    }})()",
                    doc = doc,
                    sel = css_escaped
                );
                page.evaluate(script).await?;
            } else {
                let script = format!(
                    "(() => {{
                        var style = document.createElement('style');
                        style.type = 'text/css';
                        style.innerHTML = '{sel} {{ display: none !important; }}';
                        document.head.appendChild(style);
                    }})()",
                    sel = css_escaped
                );
                page.evaluate(script.as_str()).await?;
            }
        }

        TestAction::Save(SaveAction { save, .. }) => {
            // Validate save name doesn't contain path separators
            if save.contains('/') || save.contains('\\') || save.contains("..") {
                anyhow::bail!(
                    "Invalid save name '{}': must not contain path separators or '..'",
                    save
                );
            }
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
            // Only wait for existence, no interactability needed for scroll
            ensure_exists(page, scroll, frame, &ac).await?;
            scroll_into_view(page, scroll, frame).await?;
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

            let frame_selectors = get_frame_selectors(frame);
            ensure_focusable(page, select, frame, &ac).await?;
            check_enabled(page, select, frame).await?;

            // Verify it's a <select> element
            let sel_escaped = escape_js_string(select);
            let doc = frame_selectors
                .as_ref()
                .map(|s| build_frame_document_js(s))
                .unwrap_or_else(|| "document".to_string());

            let tag_script = format!(
                "(() => {{ var el = {doc}.querySelector('{sel}'); return el ? el.tagName : ''; }})()",
                doc = doc,
                sel = sel_escaped
            );
            let tag_name: String = page
                .evaluate(tag_script)
                .await?
                .into_value()
                .unwrap_or_default();
            if tag_name.to_uppercase() != "SELECT" {
                anyhow::bail!(
                    "select action requires a <select> element, got <{}>",
                    tag_name.to_lowercase()
                );
            }

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
                    doc = doc,
                    sel = sel_escaped,
                    values = values_json
                );
                page.evaluate(script).await?;
            } else if let Some(label) = label {
                let label_escaped = escape_js_string(label);
                eprintln!("    Selecting label '{}' in element: {}", label, select);
                let script = format!(
                    "(() => {{
                        var el = {doc}.querySelector('{sel}');
                        for (var i = 0; i < el.options.length; i++) {{
                            if (el.options[i].text.trim() === '{lbl}'.trim()) {{
                                el.options[i].selected = true;
                            }} else {{
                                el.options[i].selected = false;
                            }}
                        }}
                        el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                    }})()",
                    doc = doc,
                    sel = sel_escaped,
                    lbl = label_escaped
                );
                page.evaluate(script).await?;
            }
        }

        TestAction::Uncheck(UncheckAction { uncheck, frame }) => {
            eprintln!("    Unchecking element: {}", uncheck);
            let frame_selectors = get_frame_selectors(frame);
            let info = ensure_actionable(page, uncheck, frame, &ac).await?;

            // Verify it's a checkbox or radio
            let sel_escaped = escape_js_string(uncheck);
            let doc = frame_selectors
                .as_ref()
                .map(|s| build_frame_document_js(s))
                .unwrap_or_else(|| "document".to_string());
            let type_script = format!(
                "(() => {{ var el = {doc}.querySelector('{sel}'); return el.type; }})()",
                doc = doc,
                sel = sel_escaped
            );
            let el_type: String = page
                .evaluate(type_script)
                .await?
                .into_value()
                .unwrap_or_default();
            if el_type != "checkbox" && el_type != "radio" {
                anyhow::bail!(
                    "uncheck action requires a checkbox or radio element, got type='{}'",
                    el_type
                );
            }

            // Only click if currently checked
            let checked_script = format!(
                "(() => {{ return {doc}.querySelector('{sel}').checked; }})()",
                doc = doc,
                sel = sel_escaped
            );
            let is_checked: bool = page
                .evaluate(checked_script)
                .await?
                .into_value()
                .unwrap_or(false);

            if is_checked {
                let frame_offset = actionability::get_frame_offset(page, frame).await?;
                cdp_click(
                    page,
                    frame_offset.0 + info.center_x,
                    frame_offset.1 + info.center_y,
                )
                .await?;
            }
        }

        TestAction::Wait(WaitAction { wait, url, frame }) => {
            if let Some(url) = url {
                eprintln!("    Waiting for URL: {}", url);

                // Poll-based URL check to avoid race condition with wait_for_navigation
                let timeout = std::time::Duration::from_millis(ac.element_timeout_ms);
                let start = std::time::Instant::now();

                loop {
                    let current_url = page
                        .url()
                        .await?
                        .ok_or_else(|| anyhow!("Failed to get current URL"))?;

                    let matches = if url.contains("**") {
                        // Glob-style matching
                        let parts: Vec<&str> = url.splitn(2, "**").collect();
                        let prefix = parts[0];
                        let suffix = parts.get(1).copied().unwrap_or("");
                        let prefix_ok = prefix.is_empty() || current_url.starts_with(prefix);
                        let suffix_ok = suffix.is_empty() || current_url.ends_with(suffix);
                        prefix_ok && suffix_ok
                    } else {
                        current_url == *url
                    };

                    if matches {
                        break;
                    }

                    if start.elapsed() > timeout {
                        let current_url = page.url().await?.unwrap_or_default();
                        anyhow::bail!(
                            "Timeout waiting for URL '{}', current URL is '{}'",
                            url,
                            current_url
                        );
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }

            let frame_selectors = get_frame_selectors(frame);
            let doc_js = frame_selectors.as_ref().map(|s| build_frame_document_js(s));

            match wait {
                Wait::Number(milliseconds) => {
                    eprintln!("    Waiting for {} milliseconds", milliseconds);
                    tokio::time::sleep(std::time::Duration::from_millis(*milliseconds)).await;
                }
                Wait::String(selector) => {
                    eprintln!("    Waiting for element: {}", selector);
                    if let Some(doc) = doc_js {
                        let sel_escaped = escape_js_string(selector);
                        let script =
                            format!("{doc}.querySelector('{sel}')", doc = doc, sel = sel_escaped);
                        let timeout = std::time::Duration::from_millis(ac.element_timeout_ms);
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
                                    "Timeout waiting for element '{}' in frame after {}ms",
                                    selector,
                                    ac.element_timeout_ms
                                );
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(
                                ac.poll_interval_ms,
                            ))
                            .await;
                        }
                    } else {
                        // Use the actionability module's wait_for_selector
                        wait_for_selector(page, selector, frame, &ac).await?;
                    }
                }
            }
        }
    }
    Ok(())
}
