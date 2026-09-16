use super::*;

pub(super) fn apply_theme(dark: bool, window: Option<&mut Window>, cx: &mut App) {
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        None,
        cx,
    );
    let theme = Theme::global_mut(cx);
    theme.font_family = if cfg!(target_os = "windows") {
        "Bahnschrift"
    } else if cfg!(target_os = "macos") {
        "Avenir Next"
    } else {
        "Noto Sans"
    }
    .into();
    theme.font_size = px(14.);
    theme.radius = px(4.);
    theme.radius_lg = px(6.);
    theme.focus_ring = false;
    let colors = &mut theme.colors;
    colors.background = rgb(if dark { 0x191f22 } else { 0xffffff }).into();
    colors.foreground = rgb(if dark { 0xe8eeed } else { 0x1d302d }).into();
    colors.muted = rgb(if dark { 0x252f32 } else { 0xf0f4f3 }).into();
    colors.muted_foreground = rgb(if dark { 0x9dada9 } else { 0x657873 }).into();
    colors.border = rgb(if dark { 0x38474a } else { 0xdde6e3 }).into();
    colors.primary = rgb(if dark { 0x45c8aa } else { 0x087f69 }).into();
    colors.primary_hover = rgb(if dark { 0x60d6bc } else { 0x086c5b }).into();
    colors.primary_active = rgb(if dark { 0x35ab90 } else { 0x075647 }).into();
    colors.primary_foreground = rgb(if dark { 0x10251f } else { 0xffffff }).into();
    colors.accent = rgb(if dark { 0x25483e } else { 0xe2f3ec }).into();
    colors.accent_foreground = colors.foreground;
    colors.secondary = colors.muted;
    colors.secondary_hover = colors.accent;
    colors.secondary_active = colors.accent;
    colors.secondary_foreground = colors.foreground;
    colors.button_primary = colors.primary;
    colors.button_primary_hover = colors.primary_hover;
    colors.button_primary_active = colors.primary_active;
    colors.button_primary_foreground = colors.primary_foreground;
    colors.success = rgb(if dark { 0x73c98a } else { 0x20854c }).into();
    colors.danger = rgb(if dark { 0xff8b87 } else { 0xbc3d40 }).into();
    colors.warning = rgb(if dark { 0xf0c572 } else { 0xa6740c }).into();
    colors.info = rgb(if dark { 0x77c4e2 } else { 0x267d9e }).into();
    colors.ring = colors.primary;
    colors.caret = colors.primary;
    theme.tokens = (&theme.colors).into();
    Theme::sync_base(cx);
    if let Some(window) = window {
        window.refresh();
    }
}

pub(super) fn group(key: &str) -> (usize, &'static str) {
    match key {
        "label" | "url" | "id" | "description" => (0, "General"),
        "viewportNames"
        | "viewportsPath"
        | "selectors"
        | "misMatchThreshold"
        | "delay"
        | "postInteractionWait"
        | "width"
        | "height" => (1, "Capture"),
        "needs" | "state" | "restore" | "requiredLogin" | "cookiePath" => {
            (2, "Session & dependencies")
        }
        "browser"
        | "asyncCaptureLimit"
        | "asyncCompareLimit"
        | "debug"
        | "elementTimeout"
        | "actionabilityTimeout"
        | "settleTimeout"
        | "scenarioTimeout" => (3, "Execution"),
        "hideSelectors" | "removeSelectors" | "cssOverridePath" | "jsOnReadyPath"
        | "useCssOverride" | "urlReplacements" => (4, "Page setup"),
        _ => (5, "Behavior"),
    }
}
