use std::{collections::BTreeMap, sync::OnceLock, time::Duration};

use anyhow::{Result, anyhow};
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchKeyEventParams, DispatchKeyEventType, InsertTextParams,
};
use serde::Deserialize;

use super::{
    actionability::{ActionabilityConfig, cdp_with_timeout},
    runtime::Target,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Definition {
    key: String,
    key_code: i64,
    key_code_without_location: Option<i64>,
    shift_key: Option<String>,
    shift_key_code: Option<i64>,
    text: Option<String>,
    location: Option<i64>,
}

fn definitions() -> Result<&'static BTreeMap<String, Definition>> {
    static DEFINITIONS: OnceLock<
        std::result::Result<BTreeMap<String, Definition>, serde_json::Error>,
    > = OnceLock::new();
    DEFINITIONS
        .get_or_init(|| serde_json::from_str(include_str!("vendor/keyboard.json")))
        .as_ref()
        .map_err(|error| anyhow!("Invalid embedded keyboard map: {error}"))
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct KeyStroke {
    pub key: String,
    pub code: String,
    pub modifiers: i64,
    pub text: Option<String>,
    pub virtual_key_code: Option<i64>,
    pub location: i64,
}

fn modifier_bit(part: &str) -> Option<i64> {
    match part.to_ascii_lowercase().as_str() {
        "alt" | "option" | "altleft" | "altright" => Some(1),
        "ctrl" | "control" | "controlleft" | "controlright" => Some(2),
        "meta" | "cmd" | "command" | "super" | "metaleft" | "metaright" => Some(4),
        "shift" | "shiftleft" | "shiftright" => Some(8),
        "controlormeta" => Some(if cfg!(target_os = "macos") { 4 } else { 2 }),
        _ => None,
    }
}

pub(super) fn parse_press_key(input: &str) -> Result<KeyStroke> {
    let mut modifiers = 0;
    let mut key = input;
    while let Some((part, remainder)) = key.split_once('+') {
        let Some(bit) = modifier_bit(part.trim()) else {
            break;
        };
        modifiers |= bit;
        key = remainder;
    }
    let key = match key {
        "Ctrl" | "Control" => "ControlLeft",
        "ControlOrMeta" if cfg!(target_os = "macos") => "MetaLeft",
        "ControlOrMeta" => "ControlLeft",
        "Shift" => "ShiftLeft",
        "Alt" | "Option" => "AltLeft",
        "Meta" | "Cmd" | "Command" | "Super" => "MetaLeft",
        _ => key,
    };
    if key.is_empty() || (key != "+" && key.contains('+')) {
        anyhow::bail!(
            "Unsupported key combination '{}': only one non-modifier key is allowed",
            input
        );
    }
    let definitions = definitions()?;
    let definition = definitions
        .get_key_value(key)
        .or_else(|| {
            definitions
                .iter()
                .find(|(_, definition)| definition.key == key && definition.location != Some(3))
        })
        .or_else(|| {
            definitions.iter().find(|(_, definition)| {
                definition.shift_key.as_deref() == Some(key) && definition.location != Some(3)
            })
        });
    let Some((code, definition)) = definition else {
        anyhow::bail!("Unknown keyboard key '{}'", key);
    };
    let shifted = modifiers & 8 != 0 || definition.shift_key.as_deref() == Some(key);
    let resolved = if shifted {
        definition.shift_key.as_ref().unwrap_or(&definition.key)
    } else {
        &definition.key
    };
    modifiers |= modifier_bit(resolved).unwrap_or(0);
    let text = if modifiers & !8 != 0 {
        None
    } else {
        definition
            .text
            .clone()
            .or_else(|| (resolved.chars().count() == 1).then(|| resolved.clone()))
    };
    Ok(KeyStroke {
        key: resolved.clone(),
        code: code.clone(),
        modifiers,
        text,
        virtual_key_code: Some(
            if shifted {
                definition.shift_key_code
            } else {
                None
            }
            .or(definition.key_code_without_location)
            .unwrap_or(definition.key_code),
        ),
        location: definition.location.unwrap_or(0),
    })
}

async fn dispatch_event(
    target: &Target,
    stroke: &KeyStroke,
    modifiers: i64,
    down: bool,
    config: &ActionabilityConfig,
) -> Result<()> {
    let mut command = DispatchKeyEventParams::builder()
        .r#type(if !down {
            DispatchKeyEventType::KeyUp
        } else if stroke.text.is_some() {
            DispatchKeyEventType::KeyDown
        } else {
            DispatchKeyEventType::RawKeyDown
        })
        .key(stroke.key.clone())
        .code(stroke.code.clone())
        .modifiers(modifiers)
        .location(stroke.location)
        .is_keypad(stroke.location == 3);
    if let Some(code) = stroke.virtual_key_code {
        command = command
            .windows_virtual_key_code(code)
            .native_virtual_key_code(code);
    }
    if down && let Some(text) = &stroke.text {
        command = command.text(text.clone()).unmodified_text(text.clone());
    }
    let command = command
        .build()
        .map_err(|error| anyhow!("Invalid key event: {error}"))?;
    cdp_with_timeout(
        Duration::from_millis(config.actionability_timeout_ms),
        "dispatching keyboard event",
        target.execute(command),
    )
    .await?;
    Ok(())
}

pub(super) async fn dispatch_key_stroke(
    target: &Target,
    stroke: &KeyStroke,
    config: &ActionabilityConfig,
) -> Result<()> {
    let mut held = Vec::new();
    let mut modifiers = 0;
    for name in ["Alt", "Control", "Meta", "Shift"] {
        let bit = modifier_bit(name).unwrap_or(0);
        if stroke.modifiers & bit != 0 && modifier_bit(&stroke.key) != Some(bit) {
            let modifier = parse_press_key(name)?;
            modifiers |= bit;
            dispatch_event(target, &modifier, modifiers, true, config).await?;
            held.push(modifier);
        }
    }
    let result = async {
        dispatch_event(target, stroke, stroke.modifiers, true, config).await?;
        dispatch_event(
            target,
            stroke,
            stroke.modifiers & !modifier_bit(&stroke.key).unwrap_or(0),
            false,
            config,
        )
        .await
    }
    .await;
    for modifier in held.iter().rev() {
        modifiers &= !modifier_bit(&modifier.key).unwrap_or(0);
        let release = dispatch_event(target, modifier, modifiers, false, config).await;
        if result.is_ok() {
            release?;
        }
    }
    result
}

pub(super) async fn type_text(
    target: &Target,
    text: &str,
    config: &ActionabilityConfig,
) -> Result<()> {
    for character in text.chars() {
        let text = character.to_string();
        if let Ok(stroke) = parse_press_key(&text) {
            dispatch_key_stroke(target, &stroke, config).await?;
        } else {
            let command = InsertTextParams::new(text);
            cdp_with_timeout(
                Duration::from_millis(config.actionability_timeout_ms),
                "inserting Unicode text",
                target.execute(command),
            )
            .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_map_supports_modifiers_punctuation_codes_and_function_keys() {
        assert_eq!(parse_press_key("+").unwrap().code, "Equal");
        assert_eq!(parse_press_key("+").unwrap().text.as_deref(), Some("+"));
        assert_eq!(parse_press_key("Control++").unwrap().modifiers, 2);
        assert_eq!(parse_press_key("Shift").unwrap().code, "ShiftLeft");
        assert_eq!(
            parse_press_key("Shift+a").unwrap().text.as_deref(),
            Some("A")
        );
        assert_eq!(
            parse_press_key("ControlOrMeta+A").unwrap().modifiers,
            if cfg!(target_os = "macos") { 4 } else { 2 }
        );
        assert_eq!(parse_press_key("KeyA").unwrap().key, "a");
        assert_eq!(parse_press_key(" ").unwrap().code, "Space");
        assert_eq!(parse_press_key("F12").unwrap().virtual_key_code, Some(123));
        assert!(parse_press_key("Ctrl+A+B").is_err());
        assert!(parse_press_key("UnknownKey").is_err());
    }
}
