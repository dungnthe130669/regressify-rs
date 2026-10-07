use super::*;

use crate::desktop::workspace::ThemePreference;

pub(super) fn restore_managed_menu_focus<View: 'static>(
    owner: WeakEntity<View>,
    menu_focus: FocusHandle,
    restore: FocusHandle,
    invalidate: fn(&mut View),
    window: &mut Window,
    cx: &mut Context<gpui_kit::component::menu::PopupMenu>,
) {
    let menu = cx.entity();
    window
        .subscribe(&menu, cx, move |_, _: &DismissEvent, window, cx| {
            let owner = owner.clone();
            let menu_focus = menu_focus.clone();
            let restore = restore.clone();
            window.defer(cx, move |window, cx| {
                // Managed menu painting can re-focus its closing view; never override a newer focus owner.
                if !menu_focus.is_focused(window) {
                    return;
                }
                let _ = owner.update(cx, |this, cx| {
                    invalidate(this);
                    window.focus(&restore, cx);
                    cx.notify();
                });
            });
        })
        .detach();
}

#[derive(IntoElement)]
pub(super) struct ToolbarFocus {
    id: ElementId,
    toolbar: gpui_kit::component::toolbar::Toolbar,
}

impl ToolbarFocus {
    pub(super) fn new(
        id: impl Into<ElementId>,
        toolbar: gpui_kit::component::toolbar::Toolbar,
    ) -> Self {
        Self {
            id: id.into(),
            toolbar,
        }
    }
}

impl RenderOnce for ToolbarFocus {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| {
                cx.focus_handle().tab_stop(false)
            })
            .read(cx)
            .clone();
        div()
            .id(self.id)
            .w_full()
            .min_w_0()
            .track_focus(&focus)
            .on_key_down(move |event, window, cx| {
                if event.keystroke.modifiers != Modifiers::default()
                    || !focus.contains_focused(window, cx)
                    || gpui_kit::base::GlobalState::is_in_deferred_context(cx)
                {
                    return;
                }
                let backward = match event.keystroke.key.as_str() {
                    "left" => true,
                    "right" => false,
                    _ => return,
                };
                let Some(start) = window.focused(cx) else {
                    return;
                };
                let mut previous = start.clone();
                let mut destination = start.clone();
                // Kit 0.7.1 fails backward wrapping; choose scoped destinations from bounded forward order.
                for _ in 0..1024 {
                    window.focus_next(cx);
                    let Some(next) = window.focused(cx) else {
                        break;
                    };
                    if next == start {
                        window.focus(&destination, cx);
                        cx.stop_propagation();
                        return;
                    }
                    if next == previous {
                        break;
                    }
                    if focus.contains(&next, window) {
                        if !backward {
                            cx.stop_propagation();
                            return;
                        }
                        destination = next.clone();
                    }
                    previous = next;
                }
                window.focus(&start, cx);
                cx.stop_propagation();
            })
            .child(self.toolbar.disabled(true))
    }
}

pub(super) fn titlebar_control(element: impl IntoElement) -> AnyElement {
    div()
        .flex_shrink_0()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(element)
        .into_any_element()
}

pub(super) fn selection_checkbox(
    id: impl Into<ElementId>,
    checked: bool,
    partial: bool,
    disabled: bool,
    cx: &App,
) -> gpui_kit::base::Checkbox {
    let ring = cx.theme().ring;
    gpui_kit::base::Checkbox::new(id)
        .checked(checked)
        .indeterminate(partial)
        .disabled(disabled)
        .flex()
        .items_center()
        .gap_2()
        .border_1()
        .border_color(cx.theme().transparent)
        .rounded_sm()
        .focus(move |style| style.border_color(ring))
        .when(disabled, |control| control.opacity(0.5))
        .child(
            div()
                .size(px(16.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .border_1()
                .rounded_sm()
                .border_color(if checked || partial {
                    cx.theme().primary
                } else {
                    cx.theme().input
                })
                .bg(if checked || partial {
                    cx.theme().primary
                } else {
                    cx.theme().background
                })
                .child(
                    Icon::new(if partial {
                        gpui_kit::assets::IconName::Minus
                    } else {
                        gpui_kit::assets::IconName::Check
                    })
                    .size(px(12.))
                    .text_color(cx.theme().primary_foreground)
                    .opacity(if checked || partial { 1. } else { 0. }),
                ),
        )
}

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
    theme.focus_ring = true;
    theme.scrollbar_mode = gpui_kit::component::scroll::ScrollbarMode::Always;
    let colors = &mut theme.colors;
    colors.background = rgb(if dark { 0x181c20 } else { 0xffffff }).into();
    colors.foreground = rgb(if dark { 0xecf0f2 } else { 0x223036 }).into();
    colors.muted = rgb(if dark { 0x252b30 } else { 0xf2f4f5 }).into();
    colors.muted_foreground = rgb(if dark { 0xa9b6bd } else { 0x4f5f65 }).into();
    colors.border = rgb(if dark { 0x48535b } else { 0xd2dbdf }).into();
    colors.input = rgb(if dark { 0x7b878e } else { 0x849197 }).into();
    colors.primary = rgb(if dark { 0x54c5a6 } else { 0x087f69 }).into();
    colors.primary_hover = rgb(if dark { 0x60d6bc } else { 0x086c5b }).into();
    colors.primary_active = rgb(if dark { 0x35ab90 } else { 0x075647 }).into();
    colors.primary_foreground = rgb(if dark { 0x10251f } else { 0xffffff }).into();
    colors.accent = rgb(if dark { 0x203d35 } else { 0xe2f3ec }).into();
    colors.accent_foreground = colors.foreground;
    colors.secondary = colors.muted;
    colors.secondary_hover = colors.accent;
    colors.secondary_active = colors.accent;
    colors.secondary_foreground = colors.foreground;
    colors.button = rgb(if dark { 0x30383f } else { 0xf8fafb }).into();
    colors.button_hover = colors.accent;
    colors.button_active = colors.muted;
    colors.button_foreground = colors.foreground;
    colors.button_secondary = colors.secondary;
    colors.button_secondary_hover = colors.secondary_hover;
    colors.button_secondary_active = colors.secondary_active;
    colors.button_secondary_foreground = colors.secondary_foreground;
    colors.button_primary = colors.primary;
    colors.button_primary_hover = colors.primary_hover;
    colors.button_primary_active = colors.primary_active;
    colors.button_primary_foreground = colors.primary_foreground;
    colors.success = rgb(if dark { 0x81d79c } else { 0x206c43 }).into();
    colors.danger = rgb(if dark { 0xff969a } else { 0xb12f3b }).into();
    colors.danger_hover = rgb(if dark { 0xffb5b8 } else { 0x962331 }).into();
    colors.danger_active = rgb(if dark { 0xec7c85 } else { 0x7d1d28 }).into();
    colors.danger_foreground = rgb(if dark { 0x2d1217 } else { 0xffffff }).into();
    colors.button_danger = colors.danger;
    colors.button_danger_hover = colors.danger_hover;
    colors.button_danger_active = colors.danger_active;
    colors.button_danger_foreground = colors.danger_foreground;
    colors.warning = rgb(if dark { 0xe8c174 } else { 0x825606 }).into();
    colors.info = rgb(if dark { 0x81cce7 } else { 0x236680 }).into();
    colors.popover = rgb(if dark { 0x22292e } else { 0xffffff }).into();
    colors.popover_foreground = colors.foreground;
    colors.list = colors.background;
    colors.list_head = colors.muted;
    colors.list_even = colors.muted;
    colors.list_hover = colors.accent;
    colors.list_active = colors.accent;
    colors.list_active_border = colors.primary;
    colors.tab = colors.background;
    colors.tab_bar = colors.background;
    colors.tab_bar_segmented = colors.muted;
    colors.tab_foreground = colors.muted_foreground;
    colors.tab_active = colors.popover;
    colors.tab_active_foreground = colors.foreground;
    colors.sidebar = colors.muted;
    colors.sidebar_foreground = colors.foreground;
    colors.sidebar_border = colors.border;
    colors.sidebar_accent = colors.accent;
    colors.sidebar_accent_foreground = colors.foreground;
    colors.sidebar_primary = colors.primary;
    colors.sidebar_primary_foreground = colors.primary_foreground;
    colors.title_bar = colors.muted;
    colors.title_bar_border = colors.border;
    colors.status_bar = colors.muted;
    colors.status_bar_border = colors.border;
    colors.window_border = colors.border;
    colors.group_box = colors.muted;
    colors.group_box_foreground = colors.foreground;
    colors.selection = colors.primary.opacity(if dark { 0.28 } else { 0.18 });
    colors.link = colors.info;
    colors.link_hover = colors.primary;
    colors.link_active = colors.primary_active;
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
        "label" | "url" | "id" | "displayName" | "description" => (0, "General"),
        "viewportNames"
        | "viewportsPath"
        | "selectors"
        | "misMatchThreshold"
        | "pixelMatchThreshold"
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
            .xsmall()
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

#[cfg(all(test, feature = "gui-tests"))]
mod tests {
    use gpui_kit::component::ActiveTheme;
    use gpui_kit::{Hsla, TestAppContext};

    fn luminance(color: Hsla) -> f64 {
        let color = color.to_rgb();
        let linear = |channel: f32| {
            let channel = f64::from(channel);
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
    }

    #[gpui_kit::test]
    fn both_palettes_keep_text_readable_and_keyboard_focus_visible(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        for dark in [false, true] {
            cx.update(|cx| {
                super::apply_theme(dark, None, cx);
                let theme = cx.theme();
                for (name, foreground, background) in [
                    ("body", theme.foreground, theme.background),
                    ("secondary", theme.muted_foreground, theme.muted),
                    ("selection", theme.accent_foreground, theme.accent),
                    (
                        "primary button",
                        theme.button_primary_foreground,
                        theme.button_primary,
                    ),
                    ("neutral button", theme.button_foreground, theme.button),
                    (
                        "danger button",
                        theme.button_danger_foreground,
                        theme.button_danger,
                    ),
                    ("popup", theme.popover_foreground, theme.popover),
                    ("tab", theme.tab_foreground, theme.tab_bar_segmented),
                    ("success", theme.success, theme.muted),
                    ("warning", theme.warning, theme.muted),
                    ("error", theme.danger, theme.muted),
                    ("info", theme.info, theme.muted),
                ] {
                    let front = luminance(foreground);
                    let back = luminance(background);
                    let contrast = (front.max(back) + 0.05) / (front.min(back) + 0.05);
                    assert!(
                        contrast >= 4.5,
                        "{name} contrast {contrast:.2} with dark={dark}"
                    );
                }
                assert!(theme.focus_ring, "Keyboard focus must stay visible");
            });
        }
    }
}
