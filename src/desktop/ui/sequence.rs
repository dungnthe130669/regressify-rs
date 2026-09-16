//! Inspect and arrange scenario actions without leaving their surrounding configuration.

use super::*;

fn action_icon(kind: &str) -> gpui_kit::assets::IconName {
    use gpui_kit::assets::IconName as Icon;
    match kind {
        "goto" => Icon::Globe,
        "wait" => Icon::Timer,
        "input" | "press" => Icon::Keyboard,
        "check" | "uncheck" => Icon::SquareCheck,
        "select" => Icon::ListFilter,
        "scroll" => Icon::ArrowDown,
        "hide" => Icon::EyeOff,
        "remove" => Icon::Trash,
        "save" => Icon::Camera,
        "persist" => Icon::HardDrive,
        _ => Icon::MousePointer,
    }
}

fn action_group(kind: &str) -> usize {
    match kind {
        "goto" | "wait" => 0,
        "hide" | "remove" => 2,
        "save" | "persist" => 3,
        _ => 1,
    }
}

impl Workbench {
    fn sequence_edit(
        &mut self,
        scenario: usize,
        index: usize,
        operation: fn(&mut Session) -> Result<()>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit(
            |session| {
                let previous = session.selection;
                session.select(Selection::Action { scenario, index })?;
                let result = operation(session);
                session.selection = previous;
                result
            },
            window,
            cx,
        );
    }

    fn sequence_add(&self) -> AnyElement {
        let focus = self.focus.clone();
        Button::new("sequence-add")
            .ghost()
            .small()
            .icon(IconName::Plus)
            .label("Add action")
            .disabled(self.busy || !self.field_errors.is_empty())
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu.action_context(focus.clone()).scrollable(true);
                let mut actions = ACTIONS.to_vec();
                actions.sort_by_key(|kind| action_group(kind));
                let mut previous = None;
                for kind in actions {
                    let group = action_group(kind);
                    if previous.is_some_and(|previous| previous != group) {
                        menu = menu.separator();
                    }
                    menu = menu.menu_with_icon(
                        kind.to_string(),
                        action_icon(kind),
                        Box::new(AddBrowserAction { kind: kind.into() }),
                    );
                    previous = Some(group);
                }
                menu
            })
            .into_any_element()
    }

    pub(super) fn render_sequence(&self, scenario: usize, cx: &mut Context<Self>) -> AnyElement {
        let actions = self.value["scenarios"][scenario]["actions"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let disabled = self.busy || !self.field_errors.is_empty();
        let mut rows = div()
            .id("sequence-steps")
            .flex()
            .flex_col()
            .max_h(px(264.))
            .overflow_y_scroll();
        for (index, action) in actions.iter().enumerate() {
            let kind = ACTIONS
                .iter()
                .find(|kind| action.get(**kind).is_some())
                .copied()
                .unwrap_or("action");
            let target = if let Some(text) = action[kind].as_str() {
                text.to_string()
            } else if action[kind].is_number() && kind == "wait" {
                format!("{} ms", action[kind])
            } else {
                action[kind].to_string()
            };
            let title = format!("{}{}", kind[..1].to_uppercase(), &kind[1..]);
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new(("sequence-step", index))
                            .ghost()
                            .h(px(62.))
                            .flex_1()
                            .min_w_0()
                            .justify_start()
                            .accessibility_label(format!("Action {}: {title}", index + 1))
                            .tooltip(target.clone())
                            .disabled(self.busy)
                            .child(
                                div()
                                    .w_6()
                                    .text_center()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child((index + 1).to_string()),
                            )
                            .child(
                                Icon::new(action_icon(kind))
                                    .small()
                                    .text_color(cx.theme().primary),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .flex_1()
                                    .min_w_0()
                                    .text_left()
                                    .child(div().font_weight(FontWeight::MEDIUM).child(title))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .truncate()
                                            .child(target),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select(Selection::Action { scenario, index }, window, cx)
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .flex_shrink_0()
                            .child(
                                Button::new(("sequence-up", index))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::ArrowUp)
                                    .tooltip("Move action earlier")
                                    .accessibility_label(format!(
                                        "Move action {} earlier",
                                        index + 1
                                    ))
                                    .disabled(disabled || index == 0)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.sequence_edit(
                                            scenario,
                                            index,
                                            |session| session.move_selected(-1),
                                            window,
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                Button::new(("sequence-down", index))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::ArrowDown)
                                    .tooltip("Move action later")
                                    .accessibility_label(format!("Move action {} later", index + 1))
                                    .disabled(disabled || index + 1 == actions.len())
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.sequence_edit(
                                            scenario,
                                            index,
                                            |session| session.move_selected(1),
                                            window,
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                Button::new(("sequence-copy", index))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Copy)
                                    .tooltip("Duplicate action")
                                    .accessibility_label(format!("Duplicate action {}", index + 1))
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.sequence_edit(
                                            scenario,
                                            index,
                                            Session::duplicate_selected,
                                            window,
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                Button::new(("sequence-remove", index))
                                    .ghost()
                                    .xsmall()
                                    .icon(gpui_kit::assets::IconName::Trash)
                                    .tooltip("Remove action")
                                    .accessibility_label(format!("Remove action {}", index + 1))
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let target = Selection::Action { scenario, index };
                                        this.select(target, window, cx);
                                        if this.session.selection == target {
                                            this.confirm_remove(false, window, cx);
                                        }
                                    })),
                            ),
                    ),
            );
        }
        div()
            .id("scenario-sequence")
            .test_support()
            .flex()
            .flex_col()
            .gap_2()
            .w_full()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Action sequence"),
                    )
                    .child(
                        div()
                            .id("sequence-count")
                            .role(Role::Status)
                            .aria_label(format!("{} actions", actions.len()))
                            .test_support()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(actions.len().to_string()),
                    )
                    .child(div().flex_1())
                    .child(self.sequence_add()),
            )
            .when(actions.is_empty(), |section| {
                section.child(
                    div()
                        .py_3()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("No actions"),
                )
            })
            .child(rows)
            .into_any_element()
    }
}
