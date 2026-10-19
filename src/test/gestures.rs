use std::time::Duration;

use anyhow::{Context, Result, anyhow, ensure};
use chromiumoxide::{
    Command,
    cdp::browser_protocol::input::{
        CancelDraggingParams, DispatchDragEventParams, DispatchDragEventType,
        DispatchMouseEventParams, DispatchMouseEventType, DispatchTouchEventParams,
        DispatchTouchEventType, DragData, MouseButton, SetInterceptDragsParams, TouchPoint,
    },
};
use serde_json::{Value, json};

use crate::types::{
    ActionPoint, ClickAction, DragAction, DragMode, DropFilesAction, PointerButton,
    PointerModifier, WaitAction, WaitState, WheelAction,
};

use super::{
    actionability::{self, ActionabilityConfig, cdp_with_timeout},
    cdp::{Client, Event},
    keyboard::{key_event, parse_press_key},
    runtime::Target,
};

#[derive(Clone)]
struct CleanupCommand {
    method: &'static str,
    params: Value,
}

fn command_data<C: Command>(command: C) -> Result<CleanupCommand> {
    let std::borrow::Cow::Borrowed(method) = command.identifier() else {
        anyhow::bail!("CDP input command identifier must be static");
    };
    let mut params = serde_json::to_value(command)?;
    // The pinned bindings omit empty arrays that CDP still requires for end/drop events.
    if method == DispatchTouchEventParams::IDENTIFIER && params.get("touchPoints").is_none() {
        params["touchPoints"] = json!([]);
    }
    if method == DispatchDragEventParams::IDENTIFIER && params["data"].get("items").is_none() {
        params["data"]["items"] = json!([]);
    }
    Ok(CleanupCommand { method, params })
}

pub(super) struct InputGuard {
    client: Client,
    session: String,
    timeout: Duration,
    cleanup: Vec<Option<CleanupCommand>>,
}

impl InputGuard {
    pub fn new(target: &Target, config: &ActionabilityConfig) -> Self {
        let (client, session) = target.input_connection();
        Self {
            client,
            session,
            timeout: Duration::from_millis(config.actionability_timeout_ms),
            cleanup: Vec::new(),
        }
    }

    pub fn release_on_exit<C: Command>(&mut self, command: C) -> Result<usize> {
        let index = self.cleanup.len();
        self.cleanup.push(Some(command_data(command)?));
        Ok(index)
    }

    pub fn released(&mut self, index: usize) {
        self.cleanup[index] = None;
    }

    pub async fn send<C: Command>(&self, command: C) -> Result<()> {
        let command = command_data(command)?;
        self.client
            .call_with_timeout(
                Some(&self.session),
                command.method,
                command.params,
                self.timeout,
            )
            .await?;
        Ok(())
    }

    pub async fn finish(mut self, result: Result<()>) -> Result<()> {
        let mut failure = None;
        while let Some(command) = self.cleanup.last().cloned() {
            let Some(command) = command else {
                self.cleanup.pop();
                continue;
            };
            if let Err(error) = self
                .client
                .call_with_timeout(
                    Some(&self.session),
                    command.method,
                    command.params,
                    self.timeout,
                )
                .await
            {
                failure.get_or_insert(error);
            }
            self.cleanup.pop();
        }
        match (result, failure) {
            (result, None) => result,
            (Ok(()), Some(error)) => Err(error.context("Failed to release browser input state")),
            (Err(error), Some(cleanup)) => {
                Err(error.context(format!("Browser input cleanup also failed: {cleanup}")))
            }
        }
    }
}

impl Drop for InputGuard {
    fn drop(&mut self) {
        let commands: Vec<_> = self.cleanup.drain(..).rev().flatten().collect();
        if commands.is_empty() {
            return;
        }
        // Scenario cancellation drops the gesture future. Release owned input even on that path.
        let client = self.client.clone();
        let session = self.session.clone();
        let timeout = self.timeout.min(Duration::from_secs(3));
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                for command in commands {
                    if client
                        .call_with_timeout(Some(&session), command.method, command.params, timeout)
                        .await
                        .is_err()
                    {
                        crate::progress::scenario_log!(
                            "WARNING: cancelled browser interaction could not release input state"
                        );
                    }
                }
            });
        } else {
            crate::progress::scenario_log!(
                "WARNING: browser input cleanup requires a running async runtime"
            );
        }
    }
}

fn modifier_mask(modifiers: Option<&[PointerModifier]>) -> i64 {
    modifiers
        .unwrap_or_default()
        .iter()
        .fold(0, |mask, modifier| {
            mask | match modifier {
                PointerModifier::Alt => 1,
                PointerModifier::Control => 2,
                PointerModifier::Meta => 4,
                PointerModifier::Shift => 8,
                PointerModifier::ControlOrMeta => {
                    if cfg!(target_os = "macos") {
                        4
                    } else {
                        2
                    }
                }
            }
        })
}

async fn hold_modifiers(
    guard: &mut InputGuard,
    modifiers: Option<&[PointerModifier]>,
) -> Result<i64> {
    let mask = modifier_mask(modifiers);
    let mut held = 0;
    for (name, bit) in [("Alt", 1), ("Control", 2), ("Meta", 4), ("Shift", 8)] {
        if mask & bit != 0 {
            let stroke = parse_press_key(name)?;
            guard.release_on_exit(key_event(&stroke, held, false)?)?;
            held |= bit;
            guard.send(key_event(&stroke, held, true)?).await?;
        }
    }
    Ok(mask)
}

fn mouse(
    kind: DispatchMouseEventType,
    point: ActionPoint,
    button: PointerButton,
    pressed: bool,
    count: i64,
    modifiers: i64,
) -> Result<DispatchMouseEventParams> {
    let (button, bit) = match button {
        PointerButton::Left => (MouseButton::Left, 1),
        PointerButton::Middle => (MouseButton::Middle, 4),
        PointerButton::Right => (MouseButton::Right, 2),
    };
    DispatchMouseEventParams::builder()
        .r#type(kind)
        .x(point.x)
        .y(point.y)
        .button(button)
        .buttons(if pressed { bit } else { 0 })
        .click_count(count)
        .modifiers(modifiers)
        .build()
        .map_err(anyhow::Error::msg)
}

async fn point(
    target: &Target,
    selector: &str,
    position: Option<ActionPoint>,
    prepare: bool,
    config: &ActionabilityConfig,
) -> Result<ActionPoint> {
    let info = if prepare {
        actionability::ensure_actionable_at(target, selector, &None, position, config).await?
    } else {
        actionability::check_enabled(target, selector, &None, config).await?;
        actionability::check_not_obscured_at(target, selector, &None, position, config).await?
    };
    cdp_with_timeout(
        Duration::from_millis(config.actionability_timeout_ms),
        "checking gesture frame coverage",
        target.check_frame_coverage(info.center_x, info.center_y),
    )
    .await?;
    let offset = cdp_with_timeout(
        Duration::from_millis(config.actionability_timeout_ms),
        "projecting gesture coordinates",
        target.frame_offset(),
    )
    .await?;
    Ok(ActionPoint {
        x: info.center_x + offset.0,
        y: info.center_y + offset.1,
    })
}

async fn offset_end(
    target: &Target,
    start: ActionPoint,
    offset: ActionPoint,
    config: &ActionabilityConfig,
) -> Result<ActionPoint> {
    let frame = cdp_with_timeout(
        Duration::from_millis(config.actionability_timeout_ms),
        "projecting gesture endpoint",
        target.frame_offset(),
    )
    .await?;
    let end = ActionPoint {
        x: start.x + offset.x,
        y: start.y + offset.y,
    };
    ensure!(
        end.x.is_finite() && end.y.is_finite(),
        "Gesture endpoint must be finite"
    );
    let (x, y) = (end.x - frame.0, end.y - frame.1);
    let visible: bool = cdp_with_timeout(
        Duration::from_millis(config.actionability_timeout_ms),
        "checking gesture endpoint",
        target.evaluate(format!(
            "({x} >= 0 && {y} >= 0 && {x} < innerWidth && {y} < innerHeight)"
        )),
    )
    .await?
    .into_value()?;
    ensure!(
        visible,
        "Gesture endpoint is outside the selected document's viewport"
    );
    cdp_with_timeout(
        Duration::from_millis(config.actionability_timeout_ms),
        "checking gesture endpoint frame coverage",
        target.check_frame_coverage(x, y),
    )
    .await?;
    Ok(end)
}

fn path_point(start: ActionPoint, end: ActionPoint, step: u64, steps: u64) -> ActionPoint {
    let fraction = step as f64 / steps as f64;
    ActionPoint {
        x: start.x + (end.x - start.x) * fraction,
        y: start.y + (end.y - start.y) * fraction,
    }
}

pub(super) async fn click(
    target: &Target,
    action: &ClickAction,
    config: &ActionabilityConfig,
) -> Result<()> {
    let point = point(target, &action.click, action.position, true, config).await?;
    let button = action.button.unwrap_or_default();
    let mut guard = InputGuard::new(target, config);
    let result = async {
        let modifiers = hold_modifiers(&mut guard, action.modifiers.as_deref()).await?;
        guard
            .send(mouse(
                DispatchMouseEventType::MouseMoved,
                point,
                button,
                false,
                0,
                modifiers,
            )?)
            .await?;
        for count in 1..=action.click_count.unwrap_or(1) {
            let release = mouse(
                DispatchMouseEventType::MouseReleased,
                point,
                button,
                false,
                count.into(),
                modifiers,
            )?;
            let cleanup = guard.release_on_exit(release.clone())?;
            guard
                .send(mouse(
                    DispatchMouseEventType::MousePressed,
                    point,
                    button,
                    true,
                    count.into(),
                    modifiers,
                )?)
                .await?;
            guard.send(release).await?;
            guard.released(cleanup);
        }
        Ok(())
    }
    .await;
    guard.finish(result).await
}

pub(super) async fn wheel(
    target: &Target,
    action: &WheelAction,
    config: &ActionabilityConfig,
) -> Result<()> {
    let point = point(target, &action.wheel, action.position, true, config).await?;
    let mut guard = InputGuard::new(target, config);
    let result = async {
        let modifiers = hold_modifiers(&mut guard, action.modifiers.as_deref()).await?;
        guard
            .send(mouse(
                DispatchMouseEventType::MouseMoved,
                point,
                PointerButton::Left,
                false,
                0,
                modifiers,
            )?)
            .await?;
        let command = DispatchMouseEventParams::builder()
            .r#type(DispatchMouseEventType::MouseWheel)
            .x(point.x)
            .y(point.y)
            .delta_x(action.delta_x.unwrap_or(0.0))
            .delta_y(action.delta_y.unwrap_or(0.0))
            .modifiers(modifiers)
            .build()
            .map_err(anyhow::Error::msg)?;
        guard.send(command).await
    }
    .await;
    guard.finish(result).await
}

fn drag_event(
    kind: DispatchDragEventType,
    point: ActionPoint,
    data: &DragData,
    modifiers: i64,
) -> DispatchDragEventParams {
    DispatchDragEventParams {
        r#type: kind,
        x: point.x,
        y: point.y,
        data: data.clone(),
        modifiers: Some(modifiers),
    }
}

fn intercepted(event: Event, session: &str) -> Result<Option<DragData>> {
    if event.session_id.as_deref() != Some(session) || event.method != "Input.dragIntercepted" {
        return Ok(None);
    }
    Ok(Some(
        serde_json::from_value(event.params["data"].clone()).context("Invalid native drag data")?,
    ))
}

pub(super) async fn drag(
    target: &Target,
    action: &DragAction,
    config: &ActionabilityConfig,
) -> Result<()> {
    if let Some(to) = &action.to {
        point(target, to, action.to_position, true, config).await?;
    }
    let start = point(target, &action.drag, action.position, true, config).await?;
    let end = match (&action.to, action.offset) {
        (Some(to), None) => point(target, to, action.to_position, false, config)
            .await
            .context("Both drag endpoints must be visible together")?,
        (None, Some(offset)) => offset_end(target, start, offset, config).await?,
        _ => anyhow::bail!("drag requires exactly one of to or offset"),
    };
    ensure!(
        start.x != end.x || start.y != end.y,
        "Drag endpoints must differ"
    );
    let native = match action.mode.unwrap_or_default() {
        DragMode::Pointer => false,
        DragMode::Html5 => true,
        DragMode::Auto => {
            let selector = serde_json::to_string(&action.drag)?;
            cdp_with_timeout(Duration::from_millis(config.actionability_timeout_ms), "detecting native draggable", target.evaluate(format!("(() => {{ const el = document.querySelector({selector}); if (!el) throw new Error('Drag source disappeared'); return !!(el.draggable || el.closest('[draggable=\"true\"]')); }})()"))).await?.into_value::<bool>()?
        }
    };
    let mut guard = InputGuard::new(target, config);
    let mut events = guard.client.subscribe();
    let result = async {
        let modifiers = hold_modifiers(&mut guard, action.modifiers.as_deref()).await?;
        if native {
            guard.release_on_exit(SetInterceptDragsParams::new(false))?;
            guard.send(SetInterceptDragsParams::new(true)).await?;
        }
        guard
            .send(mouse(
                DispatchMouseEventType::MouseMoved,
                start,
                PointerButton::Left,
                false,
                0,
                modifiers,
            )?)
            .await?;
        let release = mouse(
            DispatchMouseEventType::MouseReleased,
            end,
            PointerButton::Left,
            false,
            1,
            modifiers,
        )?;
        let mouse_cleanup = guard.release_on_exit(release.clone())?;
        if native {
            guard.release_on_exit(CancelDraggingParams {})?;
        }
        guard
            .send(mouse(
                DispatchMouseEventType::MousePressed,
                start,
                PointerButton::Left,
                true,
                1,
                modifiers,
            )?)
            .await?;
        let duration = action.duration.unwrap_or(300);
        let steps = duration.div_ceil(16).clamp(2, 240);
        let started = tokio::time::Instant::now();
        let mut data = None;
        for step in 1..=steps {
            tokio::time::sleep_until(started + Duration::from_millis(duration * step / steps))
                .await;
            let point = path_point(start, end, step, steps);
            if let Some(data) = &data {
                guard
                    .send(drag_event(
                        DispatchDragEventType::DragOver,
                        point,
                        data,
                        modifiers,
                    ))
                    .await?;
            } else {
                guard
                    .send(mouse(
                        DispatchMouseEventType::MouseMoved,
                        point,
                        PointerButton::Left,
                        true,
                        0,
                        modifiers,
                    )?)
                    .await?;
                if native {
                    loop {
                        match events.try_recv() {
                            Ok(event) => {
                                if let Some(captured) = intercepted(event, &guard.session)? {
                                    guard
                                        .send(drag_event(
                                            DispatchDragEventType::DragEnter,
                                            point,
                                            &captured,
                                            modifiers,
                                        ))
                                        .await?;
                                    data = Some(captured);
                                    break;
                                }
                            }
                            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
                            Err(_) => {
                                anyhow::bail!("Native drag event stream closed or overflowed")
                            }
                        }
                    }
                }
            }
        }
        if native {
            let data = match data {
                Some(data) => data,
                None => {
                    let data = tokio::time::timeout(
                        Duration::from_millis(config.actionability_timeout_ms),
                        async {
                            loop {
                                let event = events.recv().await.map_err(|_| {
                                    anyhow!("Native drag event stream closed or overflowed")
                                })?;
                                if let Some(data) = intercepted(event, &guard.session)? {
                                    return Ok::<_, anyhow::Error>(data);
                                }
                            }
                        },
                    )
                    .await
                    .map_err(|_| anyhow!("The source did not start a native HTML5 drag"))??;
                    guard
                        .send(drag_event(
                            DispatchDragEventType::DragEnter,
                            end,
                            &data,
                            modifiers,
                        ))
                        .await?;
                    data
                }
            };
            guard
                .send(drag_event(
                    DispatchDragEventType::DragOver,
                    end,
                    &data,
                    modifiers,
                ))
                .await?;
            guard
                .send(drag_event(
                    DispatchDragEventType::Drop,
                    end,
                    &data,
                    modifiers,
                ))
                .await?;
        }
        guard.send(release).await?;
        guard.released(mouse_cleanup);
        Ok(())
    }
    .await;
    guard.finish(result).await
}

pub(super) async fn drop_files(
    target: &Target,
    action: &DropFilesAction,
    files: Vec<String>,
    config: &ActionabilityConfig,
) -> Result<()> {
    let point = point(target, &action.drop_files, action.position, true, config).await?;
    let mut data = DragData::new(Vec::new(), 1);
    data.files = Some(files);
    let mut guard = InputGuard::new(target, config);
    guard.release_on_exit(CancelDraggingParams {})?;
    let result = async {
        for kind in [
            DispatchDragEventType::DragEnter,
            DispatchDragEventType::DragOver,
            DispatchDragEventType::Drop,
        ] {
            guard.send(drag_event(kind, point, &data, 0)).await?;
        }
        Ok(())
    }
    .await;
    guard.finish(result).await
}

fn touch_event(
    kind: DispatchTouchEventType,
    point: Option<ActionPoint>,
) -> DispatchTouchEventParams {
    DispatchTouchEventParams {
        r#type: kind,
        touch_points: point
            .map(|point| TouchPoint::new(point.x, point.y))
            .into_iter()
            .collect(),
        modifiers: None,
        timestamp: None,
    }
}

pub(super) async fn touch(
    target: &Target,
    selector: &str,
    position: Option<ActionPoint>,
    offset: Option<ActionPoint>,
    duration: u64,
    config: &ActionabilityConfig,
) -> Result<()> {
    let start = point(target, selector, position, true, config).await?;
    let end = if let Some(offset) = offset {
        offset_end(target, start, offset, config).await?
    } else {
        start
    };
    let enabled: bool = cdp_with_timeout(
        Duration::from_millis(config.actionability_timeout_ms),
        "checking touch capability",
        target.evaluate("navigator.maxTouchPoints > 0"),
    )
    .await?
    .into_value()?;
    ensure!(
        enabled,
        "Touch capability must be enabled before navigation for this capture"
    );
    let mut guard = InputGuard::new(target, config);
    let cleanup = guard.release_on_exit(touch_event(DispatchTouchEventType::TouchCancel, None))?;
    let result = async {
        guard
            .send(touch_event(DispatchTouchEventType::TouchStart, Some(start)))
            .await?;
        if offset.is_some() {
            let steps = duration.div_ceil(16).clamp(2, 240);
            let started = tokio::time::Instant::now();
            for step in 1..=steps {
                tokio::time::sleep_until(started + Duration::from_millis(duration * step / steps))
                    .await;
                guard
                    .send(touch_event(
                        DispatchTouchEventType::TouchMove,
                        Some(path_point(start, end, step, steps)),
                    ))
                    .await?;
            }
        } else {
            tokio::time::sleep(Duration::from_millis(duration)).await;
        }
        guard
            .send(touch_event(DispatchTouchEventType::TouchEnd, None))
            .await?;
        guard.released(cleanup);
        Ok(())
    }
    .await;
    guard.finish(result).await
}

pub(super) async fn wait_condition(
    target: &Target,
    selector: &str,
    action: &WaitAction,
    config: &ActionabilityConfig,
) -> Result<()> {
    use crate::diagnostics::record::SafeValue;
    let state = action.state.unwrap_or_default();
    let condition = match state {
        WaitState::Visible => {
            if action.text.is_some() || action.contains_text.is_some() || action.value.is_some() {
                SafeValue::Content
            } else {
                SafeValue::Visible
            }
        }
        WaitState::Hidden => SafeValue::Hidden,
        WaitState::Attached => SafeValue::Exists,
        WaitState::Detached => SafeValue::Detached,
        WaitState::Enabled => SafeValue::Enabled,
        WaitState::Disabled => SafeValue::Disabled,
    };
    let options = json!({ "selector": selector, "state": state, "text": action.text, "containsText": action.contains_text, "value": action.value });
    let visibility = actionability::visibility_script(selector, &None);
    let enabled = actionability::enabled_script(selector, &None);
    let script = format!(
        r#"(() => {{
        const options = {options};
        const el = document.querySelector(options.selector);
        const normalize = value => value.replace(/\s+/g, ' ').trim();
        if (!el) return options.state === 'hidden' || options.state === 'detached';
        const visible = ({visibility}).visible;
        const disabled = ({enabled}).disabled;
        const matches = {{ visible, hidden: !visible, attached: true, detached: false, enabled: !disabled, disabled }};
        if (!matches[options.state]) return false;
        if (options.text !== null && normalize(el.innerText ?? el.textContent ?? '') !== normalize(options.text)) return false;
        if (options.containsText !== null && !normalize(el.innerText ?? el.textContent ?? '').includes(normalize(options.containsText))) return false;
        if (options.value !== null && (!['INPUT','TEXTAREA','SELECT'].includes(el.tagName) || el.value !== options.value)) return false;
        return true;
    }})()"#
    );
    actionability::diagnostic_condition(condition, selector, async {
        let deadline =
            tokio::time::Instant::now() + Duration::from_millis(config.element_timeout_ms);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            ensure!(
                !remaining.is_zero(),
                "Timed out waiting for the requested element state/text/value"
            );
            let matched: bool = cdp_with_timeout(
                remaining,
                "checking element state/text/value",
                target.evaluate(script.clone()),
            )
            .await?
            .into_value()?;
            if matched {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(config.poll_interval_ms).min(remaining)).await;
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_empty_cdp_arrays_are_preserved() {
        let touch = command_data(touch_event(DispatchTouchEventType::TouchEnd, None)).unwrap();
        assert_eq!(touch.params["touchPoints"], json!([]));
        let drag = command_data(drag_event(
            DispatchDragEventType::Drop,
            ActionPoint { x: 1.0, y: 2.0 },
            &DragData::new(vec![], 1),
            0,
        ))
        .unwrap();
        assert_eq!(drag.params["data"]["items"], json!([]));
    }

    #[test]
    fn gesture_paths_and_modifier_masks_are_deterministic() {
        let point = path_point(
            ActionPoint { x: 10.0, y: 20.0 },
            ActionPoint { x: 30.0, y: 0.0 },
            1,
            2,
        );
        assert_eq!((point.x, point.y), (20.0, 10.0));
        assert_eq!(
            modifier_mask(Some(&[
                PointerModifier::Shift,
                PointerModifier::Control,
                PointerModifier::Control
            ])),
            10
        );
    }
}
