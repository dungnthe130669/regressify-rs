use std::{path::Path, time::Duration};

use super::keyboard::{dispatch_key_stroke, parse_press_key, type_text};
use super::runtime::Target as Page;
use anyhow::{Result, anyhow};
use chromiumoxide::cdp::browser_protocol::dom::SetFileInputFilesParams;
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
            artifact_write_error, save_screenshot_with_target, state_path,
            validate_screenshot_selector, validate_screenshot_step_name, validate_state_name,
        },
    },
    types::{
        CheckAction, ClickAction, FileInput, FocusAction, GotoAction, HideAction, HoverAction,
        InputAction, PersistAction, PressAction, RemoveAction, SaveAction, ScreenshotType,
        ScrollAction, SelectAction, SelectValue, TabTarget, TestAction, TestScenario, TestSuite,
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

/// Validate action field combinations that the JSON Schema does not express, so
/// suite preflight fails before any browser starts. Execution keeps its own
/// checks because prerequisite and legacy actions are also built in code.
pub(crate) fn validate_action_fields(action: &TestAction) -> Result<()> {
    match action {
        TestAction::Input(InputAction { value, file, .. }) => match (value, file) {
            (Some(_), Some(_)) => {
                anyhow::bail!("`input` action must not contain both `value` and `file`")
            }
            (None, None) => anyhow::bail!("`input` action must have either `value` or `file`"),
            _ => {}
        },
        TestAction::Select(SelectAction { value, label, .. }) => match (value, label) {
            (Some(_), Some(_)) => {
                anyhow::bail!("Select action must have only either `value` or `label`, not both.")
            }
            (None, None) => anyhow::bail!("Select action must have either `value` or `label`."),
            _ => {}
        },
        TestAction::Press(PressAction { key, .. }) => {
            parse_press_key(key).map_err(|error| {
                anyhow!(
                    "{error}. Key names are case-sensitive, for example Enter, Tab, ArrowDown, or Control+A"
                )
            })?;
        }
        TestAction::Persist(PersistAction { persist, .. }) => validate_state_name(persist)?,
        TestAction::SwitchTab(action) => anyhow::ensure!(
            action.switch_tab == TabTarget::New || action.url.is_none(),
            "switchTab.url is only supported with switchTab: new"
        ),
        _ => {}
    }
    Ok(())
}

/// Confirm upload files exist inside `visual_tests` during suite preflight.
pub(crate) fn validate_action_files(action: &TestAction, visual_tests_dir: &Path) -> Result<()> {
    if let TestAction::Input(InputAction {
        file: Some(file), ..
    }) = action
    {
        resolve_file_paths(file, visual_tests_dir)?;
    }
    Ok(())
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

pub(crate) struct ActionExecutionContext<'a> {
    pub page: &'a chromiumoxide::Page,
    pub runtime: &'a super::runtime::Runtime,
    pub storage: &'a super::storage_state::StorageState,
    pub test_suite: &'a TestSuite,
    pub scenario: &'a TestScenario,
    pub config: &'a ResolvedConfig,
    pub url_replacements: &'a [UrlReplacement],
    pub viewport_label: Option<&'a str>,
    pub log_prefix: &'a str,
}

pub async fn exec_test_action(ctx: ActionExecutionContext<'_>, action: &TestAction) -> Result<()> {
    use crate::diagnostics::{record::Stage, runtime as diagnostics};
    let fields = action_diagnostic_fields(action, diagnostics::next_action_index());
    let stage = if matches!(action, TestAction::Persist(_)) {
        Stage::StateHooks
    } else {
        Stage::ActionsFrames
    };
    diagnostics::action_scope(
        fields.clone(),
        diagnostics::stage(stage, fields, exec_test_action_inner(ctx, action)),
    )
    .await
}

pub(crate) fn action_diagnostic_identity(
    action: &TestAction,
) -> (crate::diagnostics::record::SafeValue, Option<&str>) {
    use crate::diagnostics::record::SafeValue;
    match action {
        TestAction::Check(action) => (SafeValue::Check, Some(&action.check)),
        TestAction::Click(action) => (SafeValue::Click, Some(&action.click)),
        TestAction::Focus(action) => (SafeValue::Focus, Some(&action.focus)),
        TestAction::Goto(action) => (SafeValue::Navigate, Some(&action.goto)),
        TestAction::Hide(action) => (SafeValue::Hide, Some(&action.hide)),
        TestAction::Hover(action) => (SafeValue::Hover, Some(&action.hover)),
        TestAction::Input(action) => (SafeValue::Input, Some(&action.input)),
        TestAction::Persist(action) => (SafeValue::Persist, Some(&action.persist)),
        TestAction::Press(action) => (SafeValue::Press, Some(&action.press)),
        TestAction::Remove(action) => (SafeValue::Remove, Some(&action.remove)),
        TestAction::Save(action) => (SafeValue::Save, Some(&action.save)),
        TestAction::Select(action) => (SafeValue::Select, Some(&action.select)),
        TestAction::Scroll(action) => (SafeValue::Scroll, Some(&action.scroll)),
        TestAction::SwitchTab(_) => (SafeValue::SwitchTab, None),
        TestAction::Uncheck(action) => (SafeValue::Uncheck, Some(&action.uncheck)),
        TestAction::Wait(action) => (
            SafeValue::Wait,
            match &action.wait {
                Wait::String(selector) => Some(selector),
                Wait::Number(_) => None,
            },
        ),
    }
}

pub(crate) fn action_diagnostic_fields(
    action: &TestAction,
    index: u64,
) -> Vec<(
    crate::diagnostics::record::Field,
    crate::diagnostics::record::FieldValue,
)> {
    use crate::diagnostics::record::*;
    let (kind, target) = action_diagnostic_identity(action);
    let depth = match action.frame() {
        Some(crate::types::Frame::Single(_)) => 1,
        Some(crate::types::Frame::Multiple(frames)) => frames.len(),
        None => 0,
    };
    let mut fields = vec![
        (Field::ActionIndex, FieldValue::Number(index)),
        (Field::ActionType, FieldValue::Kind(kind)),
        (Field::FrameDepth, FieldValue::Number(depth as u64)),
    ];
    if let Some(target) = target {
        fields.push((
            Field::Target,
            FieldValue::Identity(SafeIdentity::source(target)),
        ));
    }
    fields
}

async fn exec_test_action_inner(
    ctx: ActionExecutionContext<'_>,
    action: &TestAction,
) -> Result<()> {
    let ActionExecutionContext {
        page: main_page,
        runtime,
        storage,
        test_suite,
        scenario,
        config,
        url_replacements,
        viewport_label,
        log_prefix,
    } = ctx;
    let ac = actionability_config(config);
    match action {
        TestAction::SwitchTab(_) => {
            anyhow::bail!("Tab selection must run through the scenario page manager");
        }
        TestAction::Persist(PersistAction { persist, frame, .. }) => {
            validate_state_name(persist)?;
            let target = match frame {
                Some(_) => Some(
                    runtime
                        .target(frame, Duration::from_millis(ac.element_timeout_ms))
                        .await?,
                ),
                None => None,
            };
            let state = actionability::cdp_with_timeout(
                action_timeout(&ac),
                "capturing browser storage state",
                storage.capture_with_target(main_page, target.as_ref()),
            )
            .await?;

            let state_path = state_path(persist)?;
            if let Some(state_dir) = state_path.parent() {
                std::fs::create_dir_all(state_dir).map_err(|error| {
                    artifact_write_error("Failed to create persisted-state directory", error)
                })?;
            }
            return write_persisted_state(&state_path, &state);
        }
        TestAction::Save(SaveAction {
            save,
            selector,
            frame,
            ..
        }) => {
            validate_screenshot_step_name(save)?;
            validate_screenshot_selector(selector.as_deref())?;
            let target = match frame {
                Some(_) => Some(
                    runtime
                        .target(frame, Duration::from_millis(ac.element_timeout_ms))
                        .await?,
                ),
                None => None,
            };
            super::page_stability::prepare_screenshot(main_page, config, log_prefix).await?;

            return save_screenshot_with_target(
                test_suite,
                scenario,
                main_page,
                target.as_ref(),
                ScreenshotType::Step(save.to_string()),
                viewport_label,
                selector.as_deref(),
            )
            .await;
        }
        TestAction::Wait(WaitAction {
            wait: Wait::Number(milliseconds),
            url: None,
            frame: None,
            ..
        }) => {
            tokio::time::sleep(std::time::Duration::from_millis(*milliseconds)).await;
            return Ok(());
        }
        _ => {}
    }

    let target = runtime
        .target(action.frame(), Duration::from_millis(ac.element_timeout_ms))
        .await?;
    let page = &target;
    let frame = &None;
    let navigation = if matches!(
        action,
        TestAction::Click(_)
            | TestAction::Check(_)
            | TestAction::Uncheck(_)
            | TestAction::Press(_)
            | TestAction::Input(_)
            | TestAction::Select(_)
    ) {
        Some(page.navigation_watch().await?)
    } else {
        None
    };

    match action {
        TestAction::SwitchTab(_) | TestAction::Persist(_) | TestAction::Save(_) => {
            unreachable!("handled before target resolution")
        }
        TestAction::Check(CheckAction { check, .. }) => {
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

        TestAction::Click(ClickAction { click, .. }) => {
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

        TestAction::Focus(FocusAction { focus, .. }) => {
            ensure_focusable(page, focus, frame, &ac).await?;
            focus_element(page, focus, frame, &ac).await?;
        }

        TestAction::Goto(GotoAction { goto, .. }) => {
            let target_url = apply_action_url_replacements(goto, url_replacements)?;
            crate::diagnostics::runtime::navigation(&target_url, ac.element_timeout_ms, async {
                actionability::cdp_with_timeout(
                    Duration::from_millis(ac.element_timeout_ms),
                    "navigating to action URL",
                    page.goto(target_url.as_str()),
                )
                .await?;
                page.wait_for_ready(Duration::from_millis(config.element_timeout_ms))
                    .await
            })
            .await?;
        }

        TestAction::Hide(HideAction { hide, .. }) => {
            // Wait for the element to exist so it is actually present (and hidden)
            // in the captured screenshot; a typo'd selector fails clearly instead
            // of silently leaving the element visible.
            ensure_exists(page, hide, frame, &ac).await?;
            let frame_selectors = get_frame_selectors(frame);
            let rule = serde_json::to_string(&format!(
                "{} {{ visibility: hidden !important; }}",
                escape_css_rule(hide)
            ))?;
            let doc = frame_selectors
                .as_ref()
                .map(|selectors| build_frame_document_js(selectors))
                .unwrap_or_else(|| "document".to_string());
            let script = format!(
                "(() => {{
                    const d = {doc};
                    const style = d.createElement('style');
                    style.textContent = {rule};
                    (d.head || d.documentElement).appendChild(style);
                }})()"
            );
            let _: serde_json::Value =
                evaluate_action(page, script, &ac, "hiding the requested element").await?;
        }

        TestAction::Hover(HoverAction { hover, .. }) => {
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
            ..
        }) => {
            if value.is_some() && file.is_some() {
                anyhow::bail!("`input` action must not contain both `value` and `file`");
            }

            let frame_selectors = get_frame_selectors(frame);

            if let Some(value) = value {
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

                if !append_mode {
                    dispatch_key_stroke(page, &parse_press_key("ControlOrMeta+A")?, &ac).await?;
                    dispatch_key_stroke(page, &parse_press_key("Backspace")?, &ac).await?;
                }
                type_text(page, value, &ac).await?;
                if matches!(target, TextInputTarget::Native) {
                    let script = format!(
                        "(() => {{ const element = {doc}.querySelector('{sel}'); element.dispatchEvent(new Event('change', {{bubbles:true}})); }})()",
                        doc = doc,
                        sel = sel_escaped
                    );
                    let _: serde_json::Value =
                        evaluate_action(page, script, &ac, "dispatching input change event")
                            .await?;
                }
            } else if let Some(file) = file {
                let root_dir = ROOT_DIR
                    .get()
                    .ok_or_else(|| anyhow!("ROOT_DIR not initialized"))?;
                let visual_tests_dir = root_dir.join("visual_tests");
                let file_paths = resolve_file_paths(file, &visual_tests_dir)?;

                let info = ensure_actionable(page, input, frame, &ac).await?;

                if use_file_chooser.unwrap_or(false) {
                    let offset = page.frame_offset().await?;
                    page.choose_files(
                        file_paths,
                        offset.0 + info.center_x,
                        offset.1 + info.center_y,
                        &ac,
                    )
                    .await?;
                    return Ok(());
                }

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

        TestAction::Press(PressAction { press, key, .. }) => {
            ensure_focusable(page, press, frame, &ac).await?;
            focus_element(page, press, frame, &ac).await?;

            let keystroke = parse_press_key(key)?;
            dispatch_key_stroke(page, &keystroke, &ac).await?;
        }

        TestAction::Remove(RemoveAction { remove, .. }) => {
            // Wait for the element to exist before removing it so a typo'd selector
            // fails clearly instead of silently leaving the element in the capture.
            ensure_exists(page, remove, frame, &ac).await?;
            let frame_selectors = get_frame_selectors(frame);
            let rule = serde_json::to_string(&format!(
                "{} {{ display: none !important; }}",
                escape_css_rule(remove)
            ))?;
            let doc = frame_selectors
                .as_ref()
                .map(|selectors| build_frame_document_js(selectors))
                .unwrap_or_else(|| "document".to_string());
            let script = format!(
                "(() => {{
                    const d = {doc};
                    const style = d.createElement('style');
                    style.textContent = {rule};
                    (d.head || d.documentElement).appendChild(style);
                }})()"
            );
            let _: serde_json::Value =
                evaluate_action(page, script, &ac, "removing the requested element").await?;
        }

        TestAction::Scroll(ScrollAction { scroll, .. }) => {
            // Only wait for existence, no interactability needed for scroll
            ensure_exists(page, scroll, frame, &ac).await?;
            scroll_into_view(page, scroll, frame, &ac).await?;
        }

        TestAction::Select(SelectAction {
            select,
            value,
            label,
            ..
        }) => {
            if value.is_some() && label.is_some() {
                anyhow::bail!("Select action must have only either `value` or `label`, not both.");
            }
            if value.is_none() && label.is_none() {
                anyhow::bail!("Select action must have either `value` or `label`.");
            }

            ensure_focusable(page, select, frame, &ac).await?;
            let values = value.as_ref().map(|value| match value {
                SelectValue::Single(value) => vec![value.clone()],
                SelectValue::Multiple(values) => values.clone(),
            });
            let script = format!(
                r#"(() => {{
                const element = document.querySelector({selector});
                if (!element) return {{waiting:'select element disappeared'}};
                if (element.tagName !== 'SELECT') return {{error:'select action requires a <select> element'}};
                const values = {values};
                const label = {label};
                if (values && values.length > 1 && !element.multiple)
                    return {{error:'Cannot select multiple values on a non-multiple <select>'}};
                const options = Array.from(element.options);
                const selected = values ? values.map(value => options.find(option => option.value === value))
                    : [options.find(option => option.label.trim() === label.trim())];
                if (selected.some(option => !option)) return {{waiting:'requested option not found'}};
                if (element.disabled || selected.some(option => option.disabled || option.closest('optgroup')?.disabled))
                    return {{waiting:'requested option is disabled'}};
                for (const option of options) option.selected = selected.includes(option);
                element.dispatchEvent(new Event('input', {{bubbles:true}}));
                element.dispatchEvent(new Event('change', {{bubbles:true}}));
                return {{ok:true}};
            }})()"#,
                selector = serde_json::to_string(select)?,
                values = serde_json::to_string(&values)?,
                label = serde_json::to_string(label)?
            );
            let started = std::time::Instant::now();
            loop {
                let result: serde_json::Value =
                    evaluate_action(page, script.as_str(), &ac, "selecting options").await?;
                if result["ok"] == true {
                    break;
                }
                if let Some(error) = result["error"].as_str() {
                    anyhow::bail!("{}: {}", select, error);
                }
                if started.elapsed() >= Duration::from_millis(ac.element_timeout_ms) {
                    anyhow::bail!(
                        "Timed out selecting '{}' after {}ms: {}",
                        select,
                        ac.element_timeout_ms,
                        result["waiting"].as_str().unwrap_or("options unavailable")
                    );
                }
                tokio::time::sleep(Duration::from_millis(ac.poll_interval_ms)).await;
            }
        }

        TestAction::Uncheck(UncheckAction { uncheck, .. }) => {
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

        TestAction::Wait(WaitAction { wait, url, .. }) => {
            if let Some(url) = url {
                let target_url = apply_action_url_replacements(url, url_replacements)?;
                let matcher = UrlMatcher::compile(&target_url)?;

                // Poll-based URL check to avoid race condition with wait_for_navigation
                let timeout = std::time::Duration::from_millis(ac.element_timeout_ms);
                let start = std::time::Instant::now();

                actionability::diagnostic_condition(
                    crate::diagnostics::record::SafeValue::Navigate,
                    &target_url,
                    async {
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

                            if matcher.matches(&current_url) {
                                return Ok(());
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
                    },
                )
                .await?;
            }

            match wait {
                Wait::Number(milliseconds) => {
                    tokio::time::sleep(std::time::Duration::from_millis(*milliseconds)).await;
                }
                Wait::String(selector) => {
                    wait_for_selector(
                        page,
                        selector,
                        frame,
                        &ac,
                        crate::diagnostics::record::Stage::ActionsFrames,
                    )
                    .await?;
                }
            }
        }
    }
    if let Some(navigation) = navigation {
        page.finish_navigation(navigation, Duration::from_millis(ac.element_timeout_ms))
            .await?;
    }
    Ok(())
}

fn apply_action_url_replacements(url: &str, replacements: &[UrlReplacement]) -> Result<String> {
    config::replacements::apply_replacements(url, replacements)
}

pub(super) enum UrlMatcher {
    Exact(String),
    Glob(globset::GlobMatcher),
}

impl UrlMatcher {
    pub(super) fn compile(pattern: &str) -> Result<Self> {
        if !pattern.contains(['*', '{', '\\']) {
            return Ok(Self::Exact(pattern.to_string()));
        }
        let pattern_literal_queries = pattern.replace('?', r"\?");
        let glob = globset::GlobBuilder::new(&pattern_literal_queries)
            .literal_separator(true)
            .backslash_escape(true)
            .empty_alternates(true)
            .build()
            .map_err(|error| anyhow!("Invalid URL wait pattern '{}': {}", pattern, error))?;
        Ok(Self::Glob(glob.compile_matcher()))
    }

    pub(super) fn matches(&self, url: &str) -> bool {
        match self {
            Self::Exact(expected) => url == expected,
            Self::Glob(regex) => regex.is_match(url),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn action_diagnostics_keep_selectors_and_omit_simulated_input() {
        use crate::diagnostics::record::{Field, FieldValue, SafeIdentity};

        for payload in [
            "simulated free text from an end user",
            "synthetic-password",
            "https://example.test/user-supplied-content",
        ] {
            let action: crate::types::TestAction = serde_json::from_value(
                serde_json::json!({"input": "input[data-secret='literal-selector']", "value": payload}),
            )
            .unwrap();
            let fields = super::action_diagnostic_fields(&action, 1);
            assert!(fields.contains(&(
                Field::Target,
                FieldValue::Identity(SafeIdentity::source(
                    "input[data-secret='literal-selector']"
                ))
            )));
            let encoded = serde_json::to_string(&fields).unwrap();
            assert!(encoded.contains("literal-selector"));
            assert!(!encoded.contains(payload));
        }
    }

    #[tokio::test]
    async fn failing_action_records_index_type_and_last_unmet_condition_without_script_contents() {
        use crate::diagnostics::{journal::JournalReader, record::*, runtime as diagnostics};
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("common")).unwrap();
        std::fs::write(
            root.path().join("common/_on-ready.js"),
            "document.querySelector('#private-hook').value = 'synthetic-hook-secret';",
        )
        .unwrap();
        let action: crate::types::TestAction = serde_json::from_value(
            serde_json::json!({"click":"[data-private='synthetic-selector-secret']"}),
        )
        .unwrap();
        let operation =
            diagnostics::Operation::start(root.path(), OperationKind::Capture, None, None, None);
        let result = diagnostics::scope(Some(operation.context()), async {
            crate::test::hooks::DocumentHooks::load(
                root.path(),
                &crate::types::TestScenario::default(),
                &crate::config::ResolvedConfig::default(),
            )?;
            let fields = super::action_diagnostic_fields(&action, 7);
            diagnostics::action_scope(
                fields.clone(),
                diagnostics::stage(
                    Stage::ActionsFrames,
                    fields,
                    crate::test::actionability::diagnostic_condition(
                        SafeValue::Visible,
                        "[data-private='synthetic-selector-secret']",
                        async {
                            Err::<(), _>(anyhow::anyhow!(
                                "Element synthetic-selector-secret is not visible after 100ms"
                            ))
                        },
                    ),
                ),
            )
            .await
        })
        .await;
        assert!(result.is_err());
        let reference = operation.finish(Outcome::Failure).await;
        let mut found = false;
        JournalReader::open(root.path(), &reference.operation_id)
            .unwrap()
            .visit(|record| {
                let encoded = serde_json::to_string(&record).unwrap();
                for secret in [
                    "synthetic-hook-secret",
                    "private-hook",
                    "document.querySelector",
                ] {
                    assert!(!encoded.contains(secret));
                }
                if record.fields.get(&Field::Condition)
                    == Some(&FieldValue::Kind(SafeValue::Visible))
                    && record.outcome == Some(Outcome::Failure)
                {
                    found = true;
                    assert_eq!(
                        record.fields.get(&Field::ActionIndex),
                        Some(&FieldValue::Number(7))
                    );
                    assert_eq!(
                        record.fields.get(&Field::ActionType),
                        Some(&FieldValue::Kind(SafeValue::Click))
                    );
                    assert_eq!(
                        record.fields.get(&Field::Target),
                        Some(&FieldValue::Identity(SafeIdentity::source(
                            "[data-private='synthetic-selector-secret']"
                        )))
                    );
                }
            })
            .unwrap();
        assert!(found);
    }

    #[tokio::test]
    async fn navigation_timeout_identifies_stage_and_cause_without_credentials() {
        use crate::diagnostics::{journal::JournalReader, record::*, runtime};
        let root = tempfile::tempdir().unwrap();
        let operation =
            runtime::Operation::start(root.path(), OperationKind::Capture, None, None, None);
        let destination = "https://synthetic-user:synthetic-password@example.test/private-path?token=synthetic-query#private-fragment";
        let result = runtime::scope(
            Some(operation.context()),
            runtime::navigation(
                destination,
                1,
                crate::test::actionability::cdp_with_timeout(
                    std::time::Duration::from_millis(1),
                    "navigating to action URL",
                    std::future::pending::<anyhow::Result<()>>(),
                ),
            ),
        )
        .await;
        assert!(result.is_err());
        let reference = operation.finish(Outcome::Failure).await;
        let reader = JournalReader::open(root.path(), &reference.operation_id).unwrap();
        let mut failure = false;
        reader
            .visit(|record| {
                let encoded = serde_json::to_string(&record).unwrap();
                for secret in ["synthetic-user", "synthetic-password", "synthetic-query"] {
                    assert!(!encoded.contains(secret));
                }
                if record.stage == Stage::Navigation && record.outcome == Some(Outcome::Failure) {
                    failure = true;
                    assert_eq!(record.error_cause.unwrap().category, ErrorCategory::Timeout);
                    assert_eq!(
                        record.fields.get(&Field::Destination),
                        Some(&FieldValue::Identity(SafeIdentity::source(
                            "https://example.test/private-path?token=[redacted]#private-fragment"
                        )))
                    );
                }
            })
            .unwrap();
        assert!(failure);
    }

    use super::*;

    #[tokio::test]
    #[ignore = "requires managed Chromium; run regressify install"]
    async fn save_actions_wait_for_the_resolved_screenshot_delay() -> Result<()> {
        use crate::test::{browser_tests, runtime::Runtime, storage_state::StorageState};

        let root = match std::env::var_os("REGRESSIFY_SAVE_DELAY_FIXTURE") {
            Some(path) => std::path::PathBuf::from(path),
            None => {
                let workspace = tempfile::tempdir()?;
                let output = std::process::Command::new(std::env::current_exe()?)
                    .args([
                        "--exact",
                        "test::test_action::tests::save_actions_wait_for_the_resolved_screenshot_delay",
                        "--ignored",
                        "--nocapture",
                    ])
                    .env("REGRESSIFY_SAVE_DELAY_FIXTURE", workspace.path())
                    .output()?;
                anyhow::ensure!(
                    output.status.success(),
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                return Ok(());
            }
        };
        ROOT_DIR.set(root.clone()).unwrap();
        let output_dir = root.join(".regressify");
        crate::common::REGRESSIFY_DIR
            .set(output_dir.clone())
            .unwrap();
        let site = browser_tests::Site::new(|request| {
            let _ = request.respond(
                tiny_http::Response::from_string("<!doctype html><body></body>").with_header(
                    tiny_http::Header::from_bytes("Content-Type", "text/html").unwrap(),
                ),
            );
        })?;
        let (mut browser, handler, directory) =
            browser_tests::launch_browser("save-delay", Default::default()).await?;
        let result: Result<()> = async {
            let suite = TestSuite {
                id: Some("delay".into()),
                test_date: Some("run".into()),
                ..Default::default()
            };
            for (level, workspace_delay, scenario_delay, freeze_animations) in [
                ("workspace", 2400.0, None, false),
                ("scenario", 6000.0, Some(2400.0), true),
            ] {
                let workspace = config::WorkspaceConfig {
                    delay: Some(workspace_delay),
                    freeze_animations: Some(freeze_animations),
                    ..Default::default()
                };
                let scenario = TestScenario {
                    id: Some(level.into()),
                    url: site.origin.clone(),
                    delay: scenario_delay,
                    ..Default::default()
                };
                let resolved = config::resolve_config(&workspace, &suite, &scenario);
                anyhow::ensure!(resolved.delay == Some(2400.0));
                let page = browser.new_page(site.origin.as_str()).await?;
                let runtime = Runtime::new(&browser, &page).await?;
                let storage = StorageState::install(&browser, &page, None, &site.origin).await?;
                let context = || ActionExecutionContext {
                    page: &page,
                    runtime: &runtime,
                    storage: &storage,
                    test_suite: &suite,
                    scenario: &scenario,
                    config: &resolved,
                    url_replacements: &[],
                    viewport_label: None,
                    log_prefix: level,
                };
                for name in ["first", "second"] {
                    page.evaluate(
                        r#"(() => {
                            document.body.style.cssText = 'margin: 0; min-height: 100vh; background: rgb(255, 0, 0)';
                            const animated = document.createElement('div');
                            animated.style.cssText = 'position: absolute; left: 20px; top: 20px; width: 40px; height: 40px; background: blue';
                            document.body.replaceChildren(animated);
                            window.fixtureAnimations = [
                                animated.animate([{ opacity: 0.25 }, { opacity: 1 }], { duration: 60000, fill: 'forwards' }),
                                animated.animate([{ transform: 'translateX(0)' }, { transform: 'translateX(10px)' }], { duration: 1000, iterations: Infinity }),
                            ];
                            window.fixtureAnimations[0].onfinish = () => {
                                window.fixtureAnimations.push(animated.animate(
                                    [{ opacity: 1 }, { opacity: 0.5 }], { duration: 1000, iterations: Infinity }
                                ));
                            };
                            setTimeout(() => document.body.style.backgroundColor = 'rgb(0, 255, 0)', 1200);
                        })()"#,
                    )
                    .await?;
                    let action = TestAction::Save(SaveAction {
                        save: name.into(),
                        selector: None,
                        frame: None,
                        viewport_names: None,
                    });
                    let start = std::time::Instant::now();
                    exec_test_action(context(), &action).await?;
                    let key = crate::test::capture::final_screenshot_order_key(
                        level,
                        None,
                        &ScreenshotType::Step(name.into()),
                    );
                    let path = output_dir
                        .join("delay/bitmaps_test/run")
                        .join(format!("{key}.png"));
                    let image = lodepng::decode32(std::fs::read(path)?)?;
                    let pixel = image
                        .buffer
                        .first()
                        .ok_or_else(|| anyhow!("Saved screenshot was empty"))?;
                    anyhow::ensure!(
                        (pixel.r, pixel.g, pixel.b) == (0, 255, 0),
                        "{level} {name} captured before the configured delay: {pixel:?}"
                    );
                    anyhow::ensure!(start.elapsed() >= Duration::from_millis(2400));
                    let animations: serde_json::Value = page
                        .evaluate("window.fixtureAnimations.map(animation => [animation.playState, animation.currentTime])")
                        .await?
                        .into_value()?;
                    if freeze_animations {
                        anyhow::ensure!(
                            animations[0][0] == "finished"
                                && animations[1] == serde_json::json!(["paused", 0])
                                && animations[2] == serde_json::json!(["paused", 0]),
                            "{level} {name} did not stabilize checkpoint animations: {animations}"
                        );
                    } else {
                        anyhow::ensure!(
                            animations[0][0] == "running" && animations[1][0] == "running",
                            "{level} {name} ignored freezeAnimations=false: {animations}"
                        );
                    }
                }
                let invalid = TestAction::Save(SaveAction {
                    save: "../invalid".into(),
                    selector: None,
                    frame: None,
                    viewport_names: None,
                });
                let invalid_result = tokio::time::timeout(
                    Duration::from_millis(500),
                    exec_test_action(context(), &invalid),
                )
                .await?;
                anyhow::ensure!(invalid_result.is_err(), "Invalid save names must fail before waiting");
                page.close().await?;
            }
            Ok(())
        }
        .await;
        browser.close().await?;
        handler.await?;
        let _ = std::fs::remove_dir_all(directory);
        result
    }

    #[test]
    fn url_waits_support_brace_alternatives_and_literal_queries() {
        let matcher = UrlMatcher::compile("**/{login,dashboard}?tab=*").unwrap();
        assert!(matcher.matches("https://example.com/login?tab=one"));
        assert!(matcher.matches("https://example.com/account/dashboard?tab=two"));
        assert!(!matcher.matches("https://example.com/settings?tab=one"));
        assert!(!matcher.matches("https://example.com/loginXtab=one"));
        assert!(UrlMatcher::compile("**/{login,dashboard").is_err());
    }

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
