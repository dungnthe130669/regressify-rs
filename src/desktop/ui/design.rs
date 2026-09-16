use super::*;

use crate::desktop::workspace::ThemePreference;

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
pub(super) struct ChooseTheme {
    pub preference: ThemePreference,
}

pub(super) fn apply_preference(
    preference: ThemePreference,
    window: Option<&mut Window>,
    cx: &mut App,
) {
    cx.set_window_appearance(match preference {
        ThemePreference::System => None,
        ThemePreference::Light => Some(WindowAppearance::Light),
        ThemePreference::Dark => Some(WindowAppearance::Dark),
    });
    let appearance = window
        .as_ref()
        .map(|window| window.appearance())
        .unwrap_or_else(|| cx.window_appearance());
    apply_theme(
        preference.is_dark(matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        )),
        window,
        cx,
    );
}

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

impl Workbench {
    pub(super) fn change_theme(
        &mut self,
        action: &ChooseTheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.preferences.theme_preference() == action.preference {
            return;
        }
        self.preferences.set_theme_preference(action.preference);
        apply_preference(action.preference, Some(window), cx);
        self.save_preferences(window, cx);
    }

    pub(super) fn system_appearance_changed(
        &mut self,
        appearance: WindowAppearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.preferences.theme_preference() == ThemePreference::System {
            apply_theme(
                matches!(
                    appearance,
                    WindowAppearance::Dark | WindowAppearance::VibrantDark
                ),
                Some(window),
                cx,
            );
            cx.notify();
        }
    }

    pub(super) fn render_theme_picker(&self) -> AnyElement {
        let preference = self.preferences.theme_preference();
        let (label, icon) = match preference {
            ThemePreference::System => ("System", gpui_kit::assets::IconName::Monitor),
            ThemePreference::Light => ("Light", gpui_kit::assets::IconName::Sun),
            ThemePreference::Dark => ("Dark", gpui_kit::assets::IconName::Moon),
        };
        let focus = self.focus.clone();
        Button::new("theme-picker")
            .ghost()
            .icon(icon)
            .accessibility_label(format!("Appearance: {label}"))
            .tooltip(format!("Appearance: {label}"))
            .disabled(self.busy)
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu.action_context(focus.clone());
                for (choice, label) in [
                    (ThemePreference::System, "System"),
                    (ThemePreference::Light, "Light"),
                    (ThemePreference::Dark, "Dark"),
                ] {
                    menu = menu.menu_with_check(
                        label,
                        preference == choice,
                        Box::new(ChooseTheme { preference: choice }),
                    );
                }
                menu
            })
            .into_any_element()
    }
}
