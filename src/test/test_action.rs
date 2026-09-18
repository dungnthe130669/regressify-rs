use std::{path::Path, time::Duration};

use anyhow::{Result, anyhow};
use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::dom::SetFileInputFilesParams;
use regex::Regex;
use serde::de::DeserializeOwned;

use crate::{
    common::ROOT_DIR,
    config::{self, ResolvedConfig},
    helpers::write_file_atomically,
    test::{
        actionability::{
            self, ActionabilityConfig, build_frame_document_js, cdp_click, cdp_hover,
            ensure_actionable, ensure_exists, ensure_focusable, escape_css_rule, escape_js_string,
            focus_element, get_frame_selectors, scroll_into_view, wait_for_selector,
        },
        capture::{
            artifact_write_error, save_screenshot, state_path, validate_screenshot_step_name,
            validate_state_name,
        },
        page_stability::wait_for_dom_ready,
    },
    types::{
        CheckAction, ClickAction, FileInput, FocusAction, GotoAction, HideAction, HoverAction,
        InputAction, PersistAction, PressAction, RemoveAction, SaveAction, ScreenshotType,
        ScrollAction, SelectAction, SelectValue, TestAction, TestScenario, TestSuite,
        UncheckAction, UrlReplacement, Wait, WaitAction,
    },
};

/// Resolve file paths from a FileInput, validating they are relative and exist.
fn resolve_file_paths(file: &FileInput, visual_tests_dir: &Path) -> Result<Vec<String>> {
    let files = match file {
        FileInput::Single(f) => vec![f.clone()],
        FileInput::Multiple(v) => v.clone(),
    };

    let mut paths = Vec::new();
    let canonical_base = visual_tests_dir.canonicalize()?;
    for f in &files {
        if Path::new(f).is_absolute() {
            anyhow::bail!("`file` must be a relative path to the \"visual_tests\" folder");
        }
        let full_path = visual_tests_dir.join(f);
        if !full_path.exists() {
            anyhow::bail!("file does not exist: {}", f);
        }
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

fn write_persisted_state(path: &Path, state: &serde_json::Value) -> Result<()> {
    let state_json = serde_json::to_vec_pretty(state)
        .map_err(|error| artifact_write_error("Failed to serialize persisted state", error))?;
    write_file_atomically(path, &state_json)
        .map_err(|error| artifact_write_error("Failed to write persisted state", error))
}

fn action_timeout(config: &ActionabilityConfig) -> Duration {
    Duration::from_millis(config.actionability_timeout_ms)
}

async fn evaluate_action<T>(
    page: &Page,
    script: impl Into<chromiumoxide::js::Evaluation>,
    config: &ActionabilityConfig,
    operation: &str,
) -> Result<T>
where
    T: DeserializeOwned,
{
    evaluate_action_with_timeout(page, script, action_timeout(config), operation).await
}

async fn evaluate_action_with_timeout<T>(
    page: &Page,
    script: impl Into<chromiumoxide::js::Evaluation>,
    timeout: Duration,
    operation: &str,
) -> Result<T>
where
    T: DeserializeOwned,
{
    let result = actionability::cdp_with_timeout(timeout, operation, page.evaluate(script)).await?;
    result
        .into_value()
        .map_err(|error| anyhow!("{} returned invalid data: {}", operation, error))
}

async fn execute_action<T>(
    page: &Page,
    command: T,
    config: &ActionabilityConfig,
    operation: &str,
) -> Result<()>
where
    T: chromiumoxide::Command,
{
    actionability::cdp_with_timeout(action_timeout(config), operation, page.execute(command))
        .await?;
    Ok(())
}

/// Map a W3C `KeyboardEvent.key` value to the corresponding `code` value.
/// Covers common cases (named keys, ASCII letters, digits, space).
/// Unknown keys fall back to the key itself.
fn key_to_code(key: &str) -> String {
    match key {
        " " | "Space" => "Space".to_string(),
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

#[derive(Debug, PartialEq, Eq)]
struct KeyStroke {
    key: String,
    code: String,
    modifiers: i64,
    text: Option<String>,
    virtual_key_code: Option<i64>,
}

fn modifier_bit(part: &str) -> Option<i64> {
    match part.to_ascii_lowercase().as_str() {
        "alt" | "option" => Some(1),
        "ctrl" | "control" => Some(2),
        "meta" | "cmd" | "command" | "super" => Some(4),
        "shift" => Some(8),
        _ => None,
    }
}

fn normalize_key(key: &str) -> String {
    match key {
        "Space" => " ".to_string(),
        _ => key.to_string(),
    }
}

fn virtual_key_code(key: &str) -> Option<i64> {
    match key {
        " " | "Space" => Some(32),
        "Enter" => Some(13),
        "Escape" => Some(27),
        "Tab" => Some(9),
        "Backspace" => Some(8),
        "Delete" => Some(46),
        "Insert" => Some(45),
        "Home" => Some(36),
        "End" => Some(35),
        "PageUp" => Some(33),
        "PageDown" => Some(34),
        "ArrowLeft" => Some(37),
        "ArrowUp" => Some(38),
        "ArrowRight" => Some(39),
        "ArrowDown" => Some(40),
        k if k.chars().count() == 1 => {
            let c = k.chars().next()?;
            if c.is_ascii_alphabetic() {
                Some(c.to_ascii_uppercase() as i64)
            } else if c.is_ascii_digit() {
                Some(c as i64)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn parse_press_key(input: &str) -> Result<KeyStroke> {
    let mut modifiers = 0_i64;
    let mut key: Option<String> = None;

    for part in input.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        if let Some(bit) = modifier_bit(part) {
            modifiers |= bit;
        } else if key.is_none() {
            key = Some(normalize_key(part));
        } else {
            anyhow::bail!(
                "Unsupported key combination '{}': only one non-modifier key is allowed",
                input
            );
        }
    }

    let key = key.ok_or_else(|| anyhow!("Press action key must include a non-modifier key"))?;
    let code = key_to_code(&key);
    let printable =
        key.chars().count() == 1 && key.chars().next().map(|c| !c.is_control()).unwrap_or(false);
    let text = if printable && (modifiers & !8) == 0 {
        Some(key.clone())
    } else {
        None
    };
    let virtual_key_code = virtual_key_code(&key);

    Ok(KeyStroke {
        key,
        code,
        modifiers,
        text,
        virtual_key_code,
    })
}

/// Validate a checkable control and determine whether a user click is needed.
fn checkable_click_required(action: &str, element_type: &str, is_checked: bool) -> Result<bool> {
    if element_type != "checkbox" && element_type != "radio" {
        anyhow::bail!(
            "{} action requires a checkbox or radio element, got type='{}'",
            action,
            element_type
        );
    }
    if action == "uncheck" && element_type == "radio" && is_checked {
        anyhow::bail!(
            "uncheck action cannot clear a checked radio button; select another radio in its group instead"
        );
    }

    match action {
        "check" => Ok(!is_checked),
        "uncheck" => Ok(is_checked),
        _ => anyhow::bail!("Unsupported checkable action '{}'", action),
    }
}

fn checked_state_script(selector: &str, frame: &Option<crate::types::Frame>) -> String {
    let frame_selectors = get_frame_selectors(frame);
    let document = frame_selectors
        .as_ref()
        .map(|selectors| build_frame_document_js(selectors))
        .unwrap_or_else(|| "document".to_string());
    let selector = escape_js_string(selector);
    format!(
        "(() => {{ const el = {document}.querySelector('{selector}'); \
         return el ? {{ found: true, checked: el.checked === true }} : {{ found: false }}; }})()"
    )
}

async fn wait_for_checked_state(
    page: &Page,
    selector: &str,
    frame: &Option<crate::types::Frame>,
    expected_checked: bool,
    config: &ActionabilityConfig,
    action: &str,
) -> Result<()> {
    let timeout = action_timeout(config);
    let start = std::time::Instant::now();
    let mut last_state = "not found";
    let script = checked_state_script(selector, frame);
    let operation = format!("verifying {} state", action);

    loop {
        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            anyhow::bail!(
                "{} action did not set element '{}' to checked={} after {}ms (last state: {})",
                action,
                selector,
                expected_checked,
                timeout.as_millis(),
                last_state
            );
        }
        let value: serde_json::Value =
            evaluate_action_with_timeout(page, script.as_str(), remaining, &operation).await?;
        if value.get("found").and_then(|field| field.as_bool()) == Some(true) {
            let checked = value
                .get("checked")
                .and_then(|field| field.as_bool())
                .unwrap_or(false);
            if checked == expected_checked {
                return Ok(());
            }
            last_state = if checked { "checked" } else { "unchecked" };
        }
        tokio::time::sleep(Duration::from_millis(config.poll_interval_ms).min(remaining)).await;
    }
}

#[derive(Debug, PartialEq, Eq)]
enum TextInputTarget {
    Native,
    ContentEditable,
}

fn parse_text_input_target(value: &serde_json::Value) -> Result<TextInputTarget> {
    match value.get("kind").and_then(|field| field.as_str()) {
        Some("native") => Ok(TextInputTarget::Native),
        Some("contenteditable") => Ok(TextInputTarget::ContentEditable),
        _ => {
            let error = value
                .get("error")
                .and_then(|field| field.as_str())
                .unwrap_or("unknown");
            let tag = value
                .get("tag")
                .and_then(|field| field.as_str())
                .unwrap_or("unknown")
                .to_lowercase();
            match error {
                "readonly" => anyhow::bail!("input action target <{}> is read-only", tag),
                "not_editable" => anyhow::bail!(
                    "input action requires an editable <input>, <textarea>, or contenteditable element; got <{}>",
                    tag
                ),
                _ => anyhow::bail!(
                    "input action could not validate target <{}>: {}",
                    tag,
                    error
                ),
            }
        }
    }
}

async fn dispatch_key_stroke(
    page: &Page,
    keystroke: &KeyStroke,
    config: &ActionabilityConfig,
) -> Result<()> {
    use chromiumoxide::cdp::browser_protocol::input::{
        DispatchKeyEventParams, DispatchKeyEventType,
    };

    let mut down = DispatchKeyEventParams::builder()
        .r#type(DispatchKeyEventType::KeyDown)
        .key(keystroke.key.clone())
        .code(keystroke.code.clone());
    if keystroke.modifiers != 0 {
        down = down.modifiers(keystroke.modifiers);
    }
    if let Some(virtual_key_code) = keystroke.virtual_key_code {
        down = down
            .windows_virtual_key_code(virtual_key_code)
            .native_virtual_key_code(virtual_key_code);
    }
    if let Some(text) = &keystroke.text {
        down = down.text(text.clone()).unmodified_text(text.clone());
    }
    let down = down
        .build()
        .map_err(|error| anyhow!("Failed to build key down event: {:?}", error))?;
    execute_action(page, down, config, "dispatching key down").await?;

    let mut up = DispatchKeyEventParams::builder()
        .r#type(DispatchKeyEventType::KeyUp)
        .key(keystroke.key.clone())
        .code(keystroke.code.clone());
    if keystroke.modifiers != 0 {
        up = up.modifiers(keystroke.modifiers);
    }
    if let Some(virtual_key_code) = keystroke.virtual_key_code {
        up = up
            .windows_virtual_key_code(virtual_key_code)
            .native_virtual_key_code(virtual_key_code);
    }
    let up = up
        .build()
        .map_err(|error| anyhow!("Failed to build key up event: {:?}", error))?;
    execute_action(page, up, config, "dispatching key up").await?;

    Ok(())
}

pub(crate) struct ActionExecutionContext<'a> {
    pub page: &'a Page,
    pub test_suite: &'a TestSuite,
    pub scenario: &'a TestScenario,
    pub config: &'a ResolvedConfig,
    pub url_replacements: &'a [UrlReplacement],
    pub viewport_label: Option<&'a str>,
    pub log_prefix: &'a str,
}

pub async fn exec_test_action(ctx: ActionExecutionContext<'_>, action: &TestAction) -> Result<()> {
    let ActionExecutionContext {
        page,
        test_suite,
        scenario,
        config,
        url_replacements,
        viewport_label,
        log_prefix,
    } = ctx;
    let ac = actionability_config(config);

    match action {
        TestAction::Check(CheckAction { check, frame }) => {
            crate::progress::scenario_log!("{} Check element: {}", log_prefix, check);
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
            let el_type: String =
                evaluate_action(page, type_script, &ac, "reading checkable control type").await?;
            let checked_script = format!(
                "(() => {{ return {doc}.querySelector('{sel}').checked; }})()",
                doc = doc,
                sel = sel_escaped
            );
            let is_checked: bool =
                evaluate_action(page, checked_script, &ac, "reading checkable control state")
                    .await?;

            if checkable_click_required("check", &el_type, is_checked)? {
                let frame_offset = actionability::get_frame_offset(page, frame, &ac).await?;
                cdp_click(
                    page,
                    frame_offset.0 + info.center_x,
                    frame_offset.1 + info.center_y,
                    &ac,
                )
                .await?;
                wait_for_checked_state(page, check, frame, true, &ac, "check").await?;
            }
        }

        TestAction::Click(ClickAction { click, frame }) => {
            crate::progress::scenario_log!("{} Clicking element: {}", log_prefix, click);
            let info = ensure_actionable(page, click, frame, &ac).await?;
            let frame_offset = actionability::get_frame_offset(page, frame, &ac).await?;
            cdp_click(
                page,
                frame_offset.0 + info.center_x,
                frame_offset.1 + info.center_y,
                &ac,
            )
            .await?;
        }

        TestAction::Focus(FocusAction { focus, frame }) => {
            crate::progress::scenario_log!("{} Focusing element: {}", log_prefix, focus);
            ensure_focusable(page, focus, frame, &ac).await?;
            focus_element(page, focus, frame, &ac).await?;
        }

        TestAction::Goto(GotoAction { goto, .. }) => {
            let target_url = apply_action_url_replacements(goto, url_replacements)?;
            crate::progress::scenario_log!("{} Navigating to URL: {}", log_prefix, target_url);
            actionability::cdp_with_timeout(
                Duration::from_millis(ac.element_timeout_ms),
                "navigating to action URL",
                page.goto(target_url.as_str()),
            )
            .await?;
            wait_for_dom_ready(
                page,
                std::time::Duration::from_millis(config.element_timeout_ms),
            )
            .await?;
        }

        TestAction::Hide(HideAction { hide, frame }) => {
            crate::progress::scenario_log!("{} Hiding element: {}", log_prefix, hide);
            // Wait for the element to exist so it is actually present (and hidden)
            // in the captured screenshot; a typo'd selector fails clearly instead
            // of silently leaving the element visible.
            ensure_exists(page, hide, frame, &ac).await?;
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
                let _: serde_json::Value =
                    evaluate_action(page, script, &ac, "hiding the requested element").await?;
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
                let _: serde_json::Value =
                    evaluate_action(page, script, &ac, "hiding the requested element").await?;
            }
        }

        TestAction::Hover(HoverAction { hover, frame }) => {
            crate::progress::scenario_log!("{} Hovering over element: {}", log_prefix, hover);
            let info = ensure_actionable(page, hover, frame, &ac).await?;
            let frame_offset = actionability::get_frame_offset(page, frame, &ac).await?;
            cdp_hover(
                page,
                frame_offset.0 + info.center_x,
                frame_offset.1 + info.center_y,
                &ac,
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
                crate::progress::scenario_log!("{} Typing into element: {}", log_prefix, input);
                ensure_focusable(page, input, frame, &ac).await?;

                let sel_escaped = escape_js_string(input);
                let doc = frame_selectors
                    .as_ref()
                    .map(|s| build_frame_document_js(s))
                    .unwrap_or_else(|| "document".to_string());
                let append_mode = append.unwrap_or(false);
                let target_script = format!(
                    "(() => {{
                        const el = {doc}.querySelector('{sel}');
                        if (!el) return {{ error: 'not_found' }};
                        const tag = el.tagName;
                        const blockedTypes = new Set(['button', 'checkbox', 'color', 'file', 'hidden', 'image', 'radio', 'range', 'reset', 'submit']);
                        const native = tag === 'INPUT' && !blockedTypes.has(el.type)
                            || tag === 'TEXTAREA';
                        if (!native && !el.isContentEditable)
                            return {{ error: 'not_editable', tag }};
                        if (el.readOnly === true || el.getAttribute('aria-readonly') === 'true')
                            return {{ error: 'readonly', tag }};
                        return {{ kind: native ? 'native' : 'contenteditable' }};
                    }})()",
                    doc = doc,
                    sel = sel_escaped
                );
                let target_value: serde_json::Value =
                    evaluate_action(page, target_script, &ac, "validating text input target")
                        .await?;
                let target = parse_text_input_target(&target_value)?;
                focus_element(page, input, frame, &ac).await?;

                if frame.is_none() {
                    // CDP key/text events use Chromium's editing pipeline and
                    // therefore honor maxlength, input filtering, and beforeinput.
                    if !append_mode {
                        let select_all = if cfg!(target_os = "macos") {
                            parse_press_key("Meta+A")?
                        } else {
                            parse_press_key("Ctrl+A")?
                        };
                        dispatch_key_stroke(page, &select_all, &ac).await?;
                        dispatch_key_stroke(page, &parse_press_key("Backspace")?, &ac).await?;
                    }

                    use chromiumoxide::cdp::browser_protocol::input::InsertTextParams;
                    if !value.is_empty() {
                        let insert = InsertTextParams::builder()
                            .text(value.clone())
                            .build()
                            .map_err(|error| anyhow!("Failed to build insert text: {:?}", error))?;
                        execute_action(page, insert, &ac, "inserting input text").await?;
                    }

                    if matches!(target, TextInputTarget::Native) {
                        // CDP text insertion fires input but not change.
                        let change_script = format!(
                            "(() => {{ const el = {doc}.querySelector('{sel}'); \
                             el.dispatchEvent(new Event('change', {{ bubbles: true }})); }})()",
                            doc = doc,
                            sel = sel_escaped
                        );
                        let _: serde_json::Value = evaluate_action(
                            page,
                            change_script,
                            &ac,
                            "dispatching input change event",
                        )
                        .await?;
                    }
                } else {
                    // CDP text targeting inside a frame is unreliable. Mutate via
                    // the frame's own DOM realm while preserving framework events.
                    let value_json = serde_json::to_string(value)?
                        .replace('\u{2028}', "\\u2028")
                        .replace('\u{2029}', "\\u2029");
                    let kind = match target {
                        TextInputTarget::Native => "native",
                        TextInputTarget::ContentEditable => "contenteditable",
                    };

                    let script = format!(
                        "(() => {{
                            const el = {doc}.querySelector('{sel}');
                            const view = el.ownerDocument.defaultView;
                            const kind = '{kind}';
                            const value = {value};
                            const beforeInput = (inputType, data) => el.dispatchEvent(
                                new view.InputEvent('beforeinput', {{
                                    bubbles: true, cancelable: true, inputType, data
                                }})
                            );
                            const fireInput = (inputType, data) => el.dispatchEvent(
                                new view.InputEvent('input', {{ bubbles: true, inputType, data }})
                            );
                            const setNativeValue = (nextValue) => {{
                                const constructor = el.tagName === 'TEXTAREA'
                                    ? view.HTMLTextAreaElement : view.HTMLInputElement;
                                const setter = Object.getOwnPropertyDescriptor(constructor.prototype, 'value').set;
                                setter.call(el, nextValue);
                            }};
                            if (!{append}) {{
                                if (!beforeInput('deleteContentBackward', null))
                                    return {{ ok: false, error: 'beforeinput_prevented' }};
                                if (kind === 'native') setNativeValue('');
                                else el.textContent = '';
                                fireInput('deleteContentBackward', null);
                            }}
                            if (value.length > 0) {{
                                if (!beforeInput('insertText', value))
                                    return {{ ok: false, error: 'beforeinput_prevented' }};
                                if (kind === 'native') setNativeValue(el.value + value);
                                else el.textContent = el.textContent + value;
                                fireInput('insertText', value);
                            }}
                            if (kind === 'native')
                                el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                            return {{ ok: true }};
                        }})()",
                        doc = doc,
                        sel = sel_escaped,
                        kind = kind,
                        value = value_json,
                        append = append_mode
                    );
                    let result: serde_json::Value =
                        evaluate_action(page, script, &ac, "updating text input inside iframe")
                            .await?;
                    if result.get("ok").and_then(|field| field.as_bool()) != Some(true) {
                        let error = result
                            .get("error")
                            .and_then(|field| field.as_str())
                            .unwrap_or("unknown");
                        anyhow::bail!("input action failed: {}", error);
                    }
                }
            } else if let Some(file) = file {
                crate::progress::scenario_log!(
                    "{} Uploading file to element: {}",
                    log_prefix,
                    input
                );
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
                let input_type: String =
                    evaluate_action(page, type_script, &ac, "reading file input type").await?;
                if input_type != "file" {
                    anyhow::bail!(
                        "input file upload requires an <input type='file'> element, got type='{}'",
                        input_type
                    );
                }

                let element = actionability::cdp_with_timeout(
                    action_timeout(&ac),
                    "resolving file input element",
                    page.find_element(input),
                )
                .await?;

                if use_file_chooser.unwrap_or(false) {
                    use chromiumoxide::cdp::browser_protocol::page::SetInterceptFileChooserDialogParams;
                    // Intercept the file chooser BEFORE clicking so the click does
                    // not open the blocking native OS file-picker dialog (which
                    // would hang the renderer). We still set the files via CDP.
                    let enable = SetInterceptFileChooserDialogParams::builder()
                        .enabled(true)
                        .build()
                        .map_err(|e| anyhow!("Failed to enable file chooser intercept: {:?}", e))?;
                    let disable = SetInterceptFileChooserDialogParams::builder()
                        .enabled(false)
                        .build()
                        .map_err(|e| {
                            anyhow!("Failed to disable file chooser intercept: {:?}", e)
                        })?;
                    execute_action(page, enable, &ac, "enabling file chooser interception").await?;

                    let click_result = async {
                        cdp_click(page, info.center_x, info.center_y, &ac).await?;
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        Ok::<(), anyhow::Error>(())
                    }
                    .await;
                    let disable_result =
                        execute_action(page, disable, &ac, "disabling file chooser interception")
                            .await;
                    click_result?;
                    disable_result?;
                }

                let cmd = SetFileInputFilesParams::builder()
                    .files(file_paths)
                    .node_id(element.node_id)
                    .build()
                    .map_err(|e| anyhow!("Failed to build SetFileInputFilesParams: {}", e))?;
                execute_action(page, cmd, &ac, "setting file input contents").await?;
            } else {
                anyhow::bail!("`input` action must have either `value` or `file`");
            }
        }

        TestAction::Persist(PersistAction { persist, .. }) => {
            validate_state_name(persist)?;
            crate::progress::scenario_log!("{} Persisting state: {}", log_prefix, persist);

            // Collect cookies
            let cookies = actionability::cdp_with_timeout(
                action_timeout(&ac),
                "reading browser cookies for persisted state",
                page.get_cookies(),
            )
            .await?;
            let cookies_json = serde_json::to_value(&cookies)?;

            // Collect localStorage
            let local_storage_str: String = evaluate_action(
                page,
                "JSON.stringify(Object.fromEntries(Object.entries(localStorage)))",
                &ac,
                "reading localStorage for persisted state",
            )
            .await?;
            let local_storage: serde_json::Value = serde_json::from_str(&local_storage_str)
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

            let state = serde_json::json!({
                "cookies": cookies_json,
                "localStorage": local_storage,
            });

            let state_path = state_path(persist)?;
            if let Some(state_dir) = state_path.parent() {
                std::fs::create_dir_all(state_dir).map_err(|error| {
                    artifact_write_error("Failed to create persisted-state directory", error)
                })?;
            }
            write_persisted_state(&state_path, &state)?;
            crate::progress::scenario_log!(
                "{} State saved to: {}",
                log_prefix,
                state_path.display()
            );
        }

        TestAction::Press(PressAction { press, key, frame }) => {
            crate::progress::scenario_log!(
                "{} Pressing key '{}' on element: {}",
                log_prefix,
                key,
                press
            );
            ensure_focusable(page, press, frame, &ac).await?;
            focus_element(page, press, frame, &ac).await?;

            let keystroke = parse_press_key(key)?;
            dispatch_key_stroke(page, &keystroke, &ac).await?;
        }

        TestAction::Remove(RemoveAction { remove, frame }) => {
            crate::progress::scenario_log!("{} Removing element: {}", log_prefix, remove);
            // Wait for the element to exist before removing it so a typo'd selector
            // fails clearly instead of silently leaving the element in the capture.
            ensure_exists(page, remove, frame, &ac).await?;
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
                let _: serde_json::Value =
                    evaluate_action(page, script, &ac, "removing the requested element").await?;
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
                let _: serde_json::Value =
                    evaluate_action(page, script, &ac, "removing the requested element").await?;
            }
        }

        TestAction::Save(SaveAction { save, .. }) => {
            validate_screenshot_step_name(save)?;
            crate::progress::scenario_log!("{} Saving screenshot: {}", log_prefix, save);

            save_screenshot(
                test_suite,
                scenario,
                page,
                ScreenshotType::Step(save.to_string()),
                viewport_label,
            )
            .await?
        }

        TestAction::Scroll(ScrollAction { scroll, frame }) => {
            crate::progress::scenario_log!("{} Scrolling to element: {}", log_prefix, scroll);
            // Only wait for existence, no interactability needed for scroll
            ensure_exists(page, scroll, frame, &ac).await?;
            scroll_into_view(page, scroll, frame, &ac).await?;
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
            let tag_name: String =
                evaluate_action(page, tag_script, &ac, "reading select element tag").await?;
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
                // serde_json output is valid JS except for U+2028/U+2029 which are
                // line terminators in JS string/source context; escape them so the
                // array literal can be safely embedded in the evaluated script.
                let values_json = serde_json::to_string(&values)?
                    .replace('\u{2028}', "\\u2028")
                    .replace('\u{2029}', "\\u2029");
                crate::progress::scenario_log!(
                    "{} Selecting value(s) {} in element: {}",
                    log_prefix,
                    values_json,
                    select
                );

                let script = format!(
                    "(() => {{
                        var el = {doc}.querySelector('{sel}');
                        var values = {values};
                        if (values.length > 1 && !el.multiple) {{
                            return {{ ok: false, error: 'Cannot select multiple values on a non-multiple <select>' }};
                        }}
                        var missing = values.filter(function (value) {{
                            return !Array.prototype.some.call(el.options, function (option) {{
                                return option.value === value;
                            }});
                        }});
                        if (missing.length > 0) {{
                            return {{ ok: false, error: 'No option value matched: ' + missing.join(', ') }};
                        }}
                        for (var i = 0; i < el.options.length; i++) {{
                            el.options[i].selected = values.includes(el.options[i].value);
                        }}
                        el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                        return {{ ok: true }};
                    }})()",
                    doc = doc,
                    sel = sel_escaped,
                    values = values_json
                );
                let result: serde_json::Value =
                    evaluate_action(page, script, &ac, "selecting option values").await?;
                if !result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let error = result
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Select action failed");
                    anyhow::bail!("{}", error);
                }
            } else if let Some(label) = label {
                let label_escaped = escape_js_string(label);
                crate::progress::scenario_log!(
                    "{} Selecting label '{}' in element: {}",
                    log_prefix,
                    label,
                    select
                );
                let script = format!(
                    "(() => {{
                        var el = {doc}.querySelector('{sel}');
                        var matched = false;
                        for (var i = 0; i < el.options.length; i++) {{
                            if (el.options[i].text.trim() === '{lbl}'.trim()) {{
                                el.options[i].selected = true;
                                matched = true;
                            }} else {{
                                el.options[i].selected = false;
                            }}
                        }}
                        if (!matched) {{
                            return {{ ok: false, error: 'No option label matched: {lbl}' }};
                        }}
                        el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                        return {{ ok: true }};
                    }})()",
                    doc = doc,
                    sel = sel_escaped,
                    lbl = label_escaped
                );
                let result: serde_json::Value =
                    evaluate_action(page, script, &ac, "selecting option label").await?;
                if !result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let error = result
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Select action failed");
                    anyhow::bail!("{}", error);
                }
            }
        }

        TestAction::Uncheck(UncheckAction { uncheck, frame }) => {
            crate::progress::scenario_log!("{} Unchecking element: {}", log_prefix, uncheck);
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
            let el_type: String =
                evaluate_action(page, type_script, &ac, "reading checkable control type").await?;
            let checked_script = format!(
                "(() => {{ return {doc}.querySelector('{sel}').checked; }})()",
                doc = doc,
                sel = sel_escaped
            );
            let is_checked: bool =
                evaluate_action(page, checked_script, &ac, "reading checkable control state")
                    .await?;

            if checkable_click_required("uncheck", &el_type, is_checked)? {
                let frame_offset = actionability::get_frame_offset(page, frame, &ac).await?;
                cdp_click(
                    page,
                    frame_offset.0 + info.center_x,
                    frame_offset.1 + info.center_y,
                    &ac,
                )
                .await?;
                wait_for_checked_state(page, uncheck, frame, false, &ac, "uncheck").await?;
            }
        }

        TestAction::Wait(WaitAction { wait, url, frame }) => {
            if let Some(url) = url {
                let target_url = apply_action_url_replacements(url, url_replacements)?;
                crate::progress::scenario_log!("{} Waiting for URL: {}", log_prefix, target_url);
                let matcher = UrlMatcher::compile(&target_url)?;

                // Poll-based URL check to avoid race condition with wait_for_navigation
                let timeout = std::time::Duration::from_millis(ac.element_timeout_ms);
                let start = std::time::Instant::now();

                loop {
                    let remaining = timeout.saturating_sub(start.elapsed());
                    if remaining.is_zero() {
                        anyhow::bail!("Timeout waiting for URL '{}'", target_url);
                    }
                    let current_url = actionability::cdp_with_timeout(
                        remaining,
                        "reading current URL",
                        page.url(),
                    )
                    .await?
                    .ok_or_else(|| anyhow!("Failed to get current URL"))?;

                    let matches = matcher.matches(&current_url);

                    if matches {
                        break;
                    }

                    if start.elapsed() > timeout {
                        anyhow::bail!(
                            "Timeout waiting for URL '{}', current URL is '{}'",
                            target_url,
                            current_url
                        );
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }

            match wait {
                Wait::Number(milliseconds) => {
                    crate::progress::scenario_log!(
                        "{} Waiting for {} milliseconds",
                        log_prefix,
                        milliseconds
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(*milliseconds)).await;
                }
                Wait::String(selector) => {
                    crate::progress::scenario_log!(
                        "{} Waiting for element: {}",
                        log_prefix,
                        selector
                    );
                    wait_for_selector(page, selector, frame, &ac).await?;
                }
            }
        }
    }
    Ok(())
}

fn apply_action_url_replacements(url: &str, replacements: &[UrlReplacement]) -> Result<String> {
    config::replacements::apply_replacements(url, replacements)
}

enum UrlMatcher {
    Exact(String),
    Glob(Regex),
}

impl UrlMatcher {
    fn compile(pattern: &str) -> Result<Self> {
        if !pattern.contains('*') {
            return Ok(Self::Exact(pattern.to_string()));
        }

        let mut expression = String::from("^");
        let mut chars = pattern.chars().peekable();
        let mut in_query = false;
        while let Some(character) = chars.next() {
            match character {
                '?' if !in_query => {
                    in_query = true;
                    expression.push_str(r"\?");
                }
                '*' => {
                    if chars.next_if_eq(&'*').is_some() {
                        if chars.next_if_eq(&'/').is_some() {
                            // A globstar directory can match zero directories:
                            // `/ ** / dashboard` also matches `/dashboard`.
                            expression.push_str("(?:.*/)?");
                        } else {
                            expression.push_str(".*");
                        }
                    } else if in_query {
                        expression.push_str(".*");
                    } else {
                        expression.push_str("[^/?#]*");
                    }
                }
                '?' => expression.push('.'),
                _ => expression.push_str(&regex::escape(&character.to_string())),
            }
        }
        expression.push('$');

        Regex::new(&expression)
            .map(Self::Glob)
            .map_err(|error| anyhow!("Invalid URL wait pattern '{}': {}", pattern, error))
    }

    fn matches(&self, url: &str) -> bool {
        match self {
            Self::Exact(expected) => url == expected,
            Self::Glob(regex) => regex.is_match(url),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_press_key_supports_ctrl_shortcut() {
        let key = parse_press_key("Ctrl+A").unwrap();
        assert_eq!(key.key, "A");
        assert_eq!(key.code, "KeyA");
        assert_eq!(key.modifiers, 2);
        assert_eq!(key.text, None);
        assert_eq!(key.virtual_key_code, Some(65));
    }

    #[test]
    fn parse_press_key_supports_shift_tab() {
        let key = parse_press_key("Shift+Tab").unwrap();
        assert_eq!(key.key, "Tab");
        assert_eq!(key.code, "Tab");
        assert_eq!(key.modifiers, 8);
        assert_eq!(key.virtual_key_code, Some(9));
    }

    #[test]
    fn parse_press_key_rejects_multiple_non_modifier_keys() {
        let err = parse_press_key("Ctrl+A+B").unwrap_err();
        assert!(err.to_string().contains("only one non-modifier key"));
    }

    #[test]
    fn action_url_replacements_apply_before_navigation_or_matching() {
        let replacements = vec![UrlReplacement {
            ref_pattern: "https://prod.example.com".to_string(),
            test: "https://test.example.com".to_string(),
            regex: None,
            flags: None,
        }];

        assert_eq!(
            apply_action_url_replacements("https://prod.example.com/dashboard", &replacements)
                .unwrap(),
            "https://test.example.com/dashboard"
        );
    }

    #[test]
    fn url_matches_supports_multi_segment_globs() {
        let deep_dashboard = UrlMatcher::compile("https://example.com/**/dashboard").unwrap();
        let one_segment = UrlMatcher::compile("https://example.com/*/overview").unwrap();

        assert!(deep_dashboard.matches("https://example.com/dashboard"));
        assert!(deep_dashboard.matches("https://example.com/a/b/dashboard"));
        assert!(one_segment.matches("https://example.com/a/overview"));
        assert!(!one_segment.matches("https://example.com/a/b/overview"));
        assert!(!deep_dashboard.matches("https://example.com/a/b/settings"));
    }

    #[test]
    fn url_matches_keeps_query_delimiter_literal() {
        let matcher = UrlMatcher::compile("https://example.com/**/dashboard?tab=*").unwrap();

        assert!(matcher.matches("https://example.com/dashboard?tab=metrics"));
        assert!(!matcher.matches("https://example.com/dashboard?section=metrics"));
    }

    #[test]
    fn checkable_click_required_only_clicks_when_state_needs_change() {
        assert!(checkable_click_required("check", "checkbox", false).unwrap());
        assert!(!checkable_click_required("check", "radio", true).unwrap());
        assert!(checkable_click_required("uncheck", "checkbox", true).unwrap());
        assert!(!checkable_click_required("uncheck", "checkbox", false).unwrap());
    }

    #[test]
    fn checkable_click_required_rejects_checked_radio_uncheck() {
        let error = checkable_click_required("uncheck", "radio", true).unwrap_err();
        assert!(error.to_string().contains("cannot clear a checked radio"));
    }

    #[test]
    fn checkable_click_required_rejects_non_checkable_elements() {
        let error = checkable_click_required("check", "text", false).unwrap_err();
        assert!(error.to_string().contains("checkbox or radio"));
    }

    #[test]
    fn checked_state_script_reads_checkbox_state() {
        if std::process::Command::new("node")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }

        let temp = tempfile::tempdir().unwrap();
        let script_path = temp.path().join("checked-state.js");
        let script = checked_state_script("#newsletter", &None);
        std::fs::write(
            &script_path,
            format!(
                "global.document = {{ querySelector: () => ({{ checked: false }}) }};\n\
                 const result = {script};\n\
                 if (!result.found || result.checked !== false) throw new Error(JSON.stringify(result));\n"
            ),
        )
        .unwrap();
        let output = std::process::Command::new("node")
            .arg(&script_path)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn text_input_target_accepts_editable_target_types() {
        assert_eq!(
            parse_text_input_target(&serde_json::json!({ "kind": "native" })).unwrap(),
            TextInputTarget::Native
        );
        assert_eq!(
            parse_text_input_target(&serde_json::json!({ "kind": "contenteditable" })).unwrap(),
            TextInputTarget::ContentEditable
        );
    }

    #[test]
    fn text_input_target_reports_non_editable_and_readonly_failures() {
        let non_editable = parse_text_input_target(&serde_json::json!({
            "error": "not_editable",
            "tag": "DIV"
        }))
        .unwrap_err();
        let read_only = parse_text_input_target(&serde_json::json!({
            "error": "readonly",
            "tag": "INPUT"
        }))
        .unwrap_err();

        assert!(non_editable.to_string().contains("contenteditable"));
        assert!(read_only.to_string().contains("read-only"));
    }

    #[test]
    fn persisted_state_write_replaces_valid_json_atomically() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("auth.json");
        std::fs::write(&path, b"old state").unwrap();
        let state = serde_json::json!({ "cookies": [], "localStorage": {} });

        write_persisted_state(&path, &state).unwrap();

        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path).unwrap()).unwrap(),
            state
        );
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
    }
}
