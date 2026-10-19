//! Inspect and arrange scenario actions without leaving their surrounding configuration.
#![cfg_attr(test, allow(clippy::items_after_test_module))]

use super::*;
use std::rc::Rc;

type SequenceScope = (PathBuf, PathBuf, usize);

#[derive(Default)]
struct SequenceIdentity {
    scope: Option<SequenceScope>,
    actions: Vec<Value>,
    ids: Vec<u64>,
    next_id: u64,
    current: Option<u64>,
}

impl SequenceIdentity {
    fn allocate(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("Sequence identity exhausted");
        id
    }

    fn reconcile(&mut self, scope: Option<SequenceScope>, actions: Vec<Value>) {
        if self.scope == scope && self.actions == actions {
            return;
        }
        let ids = actions
            .iter()
            .map(|action| {
                // Without a known permutation, equal duplicates cannot identify their former rows.
                let previous = (self.scope == scope
                    && actions
                        .iter()
                        .filter(|candidate| *candidate == action)
                        .count()
                        == 1
                    && self
                        .actions
                        .iter()
                        .filter(|candidate| *candidate == action)
                        .count()
                        == 1)
                    .then(|| {
                        self.actions
                            .iter()
                            .position(|candidate| candidate == action)
                    })
                    .flatten();
                match previous {
                    Some(index) => self.ids[index],
                    None => self.allocate(),
                }
            })
            .collect::<Vec<_>>();
        self.current = self.current.filter(|id| ids.contains(id));
        self.scope = scope;
        self.actions = actions;
        self.ids = ids;
    }

    fn known_edit(
        &mut self,
        previous_ids: Vec<u64>,
        current: Option<u64>,
        index: usize,
        command: SequenceCommand,
    ) {
        self.ids = previous_ids;
        match command {
            SequenceCommand::Earlier => self.ids.swap(index, index - 1),
            SequenceCommand::Later => self.ids.swap(index, index + 1),
            SequenceCommand::Duplicate => {
                let id = self.allocate();
                self.ids.insert(index + 1, id);
            }
            _ => return,
        }
        self.current = current;
    }
}

#[derive(Clone, Copy)]
enum SequenceCommand {
    Open,
    Earlier,
    Later,
    Duplicate,
    Remove,
}

pub(super) type SequenceListState = gpui_kit::component::list::ListState<SequenceRows>;

#[derive(Clone)]
struct SequenceSnapshot {
    scope: SequenceScope,
    source: String,
    actions: Vec<Value>,
    ids: Vec<u64>,
}

pub(super) struct SequenceRows {
    owner: WeakEntity<Workbench>,
    identity: SequenceIdentity,
    snapshot: Option<Rc<SequenceSnapshot>>,
    busy: bool,
    invalid: bool,
}

impl SequenceRows {
    pub(super) fn new(owner: WeakEntity<Workbench>) -> Self {
        Self {
            owner,
            identity: SequenceIdentity::default(),
            snapshot: None,
            busy: false,
            invalid: false,
        }
    }

    fn dispatch(&self, index: usize, command: SequenceCommand, window: &mut Window, cx: &mut App) {
        let Some(snapshot) = self.snapshot.clone() else {
            return;
        };
        let Some(id) = self.identity.ids.get(index).copied() else {
            return;
        };
        let owner = self.owner.clone();
        // A delegate callback leases ListState; parent guards read it again.
        window.defer(cx, move |window, cx| {
            let _ = owner.update(cx, |this, cx| {
                this.sequence_command(snapshot, id, index, command, window, cx)
            });
        });
    }
}

impl gpui_kit::component::list::ListDelegate for SequenceRows {
    type Item = gpui_kit::component::list::ListItem;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.identity.ids.len()
    }

    fn set_selected_index(
        &mut self,
        index: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<SequenceListState>,
    ) {
        self.identity.current = index.and_then(|index| self.identity.ids.get(index.row).copied());
    }

    fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<SequenceListState>) {
        if !self.busy
            && let Some(index) = self.identity.current.and_then(|id| {
                self.identity
                    .ids
                    .iter()
                    .position(|candidate| *candidate == id)
            })
        {
            self.dispatch(index, SequenceCommand::Open, window, cx);
        }
    }

    fn render_item(
        &mut self,
        index: IndexPath,
        _: &mut Window,
        cx: &mut Context<SequenceListState>,
    ) -> Option<Self::Item> {
        use gpui_kit::assets::IconName;
        let index = index.row;
        let action = self.identity.actions.get(index)?;
        let id = *self.identity.ids.get(index)?;
        let snapshot = self.snapshot.clone()?;
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
        let description = target.clone();
        let mut commands = div().flex().items_center().gap_1().flex_shrink_0();
        for (name, icon, label, command, boundary) in [
            (
                "sequence-up",
                IconName::ArrowUp,
                "Move action earlier",
                SequenceCommand::Earlier,
                index == 0,
            ),
            (
                "sequence-down",
                IconName::ArrowDown,
                "Move action later",
                SequenceCommand::Later,
                index + 1 == self.identity.ids.len(),
            ),
            (
                "sequence-copy",
                IconName::Copy,
                "Duplicate action",
                SequenceCommand::Duplicate,
                false,
            ),
            (
                "sequence-remove",
                IconName::Trash,
                "Remove action",
                SequenceCommand::Remove,
                false,
            ),
        ] {
            let owner = self.owner.clone();
            let snapshot = snapshot.clone();
            commands = commands.child(
                Button::new((name, index))
                    .ghost()
                    .icon(icon)
                    .tooltip(label)
                    .accessibility_label(format!("{label} {}", index + 1))
                    .disabled(self.busy || self.invalid || boundary)
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        let _ = owner.update(cx, |this, cx| {
                            this.sequence_command(snapshot.clone(), id, index, command, window, cx)
                        });
                    }),
            );
        }
        let owner = self.owner.clone();
        Some(
            gpui_kit::component::list::ListItem::new(SharedString::from(format!(
                "sequence-action-{id}"
            )))
            .disabled(self.busy)
            .accessibility_label(format!("Action {}: {title}", index + 1))
            .tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(target.clone()).build(window, cx)
            })
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                let _ = owner.update(cx, |this, cx| {
                    this.sequence_command(
                        snapshot.clone(),
                        id,
                        index,
                        SequenceCommand::Open,
                        window,
                        cx,
                    )
                });
            })
            .child(
                div()
                    .id(("sequence-step", index))
                    .test_support()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .w_full()
                    .child(div().w_6().text_center().child((index + 1).to_string()))
                    .child(Icon::new(action_icon(kind)).small())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().font_weight(FontWeight::MEDIUM).child(title))
                            .child(
                                div()
                                    .truncate()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(description),
                            ),
                    )
                    .child(commands),
            ),
        )
    }
}

fn action_icon(kind: &str) -> gpui_kit::assets::IconName {
    use gpui_kit::assets::IconName as Icon;
    match kind {
        "goto" | "navigate" => Icon::Globe,
        "switchTab" => Icon::ExternalLink,
        "wait" => Icon::Timer,
        "input" | "press" | "key" => Icon::Keyboard,
        "check" | "uncheck" => Icon::SquareCheck,
        "select" => Icon::ListFilter,
        "scroll" | "wheel" | "swipe" => Icon::ArrowDown,
        "hide" => Icon::EyeOff,
        "remove" => Icon::Trash,
        "save" => Icon::Camera,
        "persist" => Icon::HardDrive,
        _ => Icon::MousePointer,
    }
}

#[cfg(test)]
mod tests {
    use super::{SequenceCommand, SequenceIdentity};
    use std::path::PathBuf;

    #[test]
    fn sequence_identity_reconciles_unique_actions_and_invalidates_ambiguous_duplicates() {
        let scope = Some((PathBuf::from("workspace"), PathBuf::from("suite.yaml"), 0));
        let click = serde_json::json!({"click": "#continue"});
        let wait = serde_json::json!({"wait": 500});
        let mut identity = SequenceIdentity::default();
        identity.reconcile(scope.clone(), vec![click.clone(), wait.clone()]);
        let original = identity.ids.clone();
        identity.current = Some(original[0]);
        identity.reconcile(scope.clone(), vec![wait.clone(), click.clone()]);
        assert_eq!(identity.ids, vec![original[1], original[0]]);
        assert_eq!(identity.current, Some(original[0]));
        let reordered = identity.ids.clone();
        identity.reconcile(scope.clone(), identity.actions.clone());
        assert_eq!(identity.ids, reordered);
        identity.reconcile(
            scope.clone(),
            vec![wait.clone(), click.clone(), click.clone()],
        );
        assert_eq!(identity.ids[0], original[1]);
        assert!(!identity.ids.contains(&original[0]));
        assert_ne!(identity.ids[1], identity.ids[2]);
        assert_eq!(identity.current, None);
        identity.known_edit(reordered, Some(original[0]), 1, SequenceCommand::Duplicate);
        assert_eq!(identity.ids[1], original[0]);
        assert_ne!(identity.ids[1], identity.ids[2]);
        assert_eq!(identity.current, Some(original[0]));
        let duplicated = identity.ids.clone();
        identity.known_edit(
            duplicated.clone(),
            identity.current,
            1,
            SequenceCommand::Later,
        );
        assert_eq!(
            identity.ids,
            vec![duplicated[0], duplicated[2], duplicated[1]]
        );
        assert_eq!(identity.current, Some(original[0]));
        identity.reconcile(
            Some((PathBuf::from("other"), PathBuf::from("suite.yaml"), 0)),
            identity.actions.clone(),
        );
        assert!(identity.ids.iter().all(|id| !duplicated.contains(id)));
        assert_eq!(identity.current, None);
        for scope in [
            (PathBuf::from("other"), PathBuf::from("different.yaml"), 0),
            (PathBuf::from("other"), PathBuf::from("different.yaml"), 1),
        ] {
            let previous = identity.ids.clone();
            identity.reconcile(Some(scope), identity.actions.clone());
            assert!(identity.ids.iter().all(|id| !previous.contains(id)));
        }
    }

    #[cfg(feature = "gui-tests")]
    #[gpui_kit::test]
    fn flat_collection_sequence_identity_tracks_equal_moves_and_duplicates(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use crate::desktop::session::Selection;
        use gpui_kit::AppContext;
        use gpui_kit::component::IndexPath;
        use gpui_kit::test::TestWindowExt;
        let directory = tempfile::tempdir().unwrap();
        let (handle, view) = crate::desktop::ui::tests::editor(cx, directory.path());
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.edit(
                    |session| {
                        session.set_field(
                            "actions",
                            Some(serde_json::json!([
                                {"click": "#same"}, {"click": "#same"}, {"wait": 500}
                            ])),
                        )
                    },
                    window,
                    cx,
                );
                this.field_search
                    .update(cx, |state, cx| state.set_value("actions", window, cx));
            });
            window.render_frame(cx);
            let ids = view
                .read(cx)
                .sequence_list
                .read(cx)
                .delegate()
                .identity
                .ids
                .clone();
            let stale = view
                .read(cx)
                .sequence_list
                .read(cx)
                .delegate()
                .snapshot
                .clone()
                .unwrap();
            assert_ne!(ids[0], ids[1]);
            assert!(window.find(format!("sequence-action-{}", ids[0])).visible());
            assert!(window.find(format!("sequence-action-{}", ids[1])).visible());
            view.read(cx).sequence_list.clone().update(cx, |list, cx| {
                list.set_selected_index(Some(IndexPath::new(0)), window, cx)
            });
            window.click(("sequence-down", 0_usize), cx);
            let list = view.read(cx).sequence_list.read(cx);
            assert_eq!(list.delegate().identity.ids, vec![ids[1], ids[0], ids[2]]);
            assert_eq!(list.delegate().identity.current, Some(ids[0]));
            assert_eq!(list.selected_index(), Some(IndexPath::new(1)));
            assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
            view.update(cx, |this, cx| {
                this.sequence_command(stale, ids[0], 0, SequenceCommand::Open, window, cx)
            });
            assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
            window.click(("sequence-copy", 1_usize), cx);
            let duplicated = view
                .read(cx)
                .sequence_list
                .read(cx)
                .delegate()
                .identity
                .ids
                .clone();
            assert_eq!(duplicated[0..2], [ids[1], ids[0]]);
            assert_eq!(duplicated[3], ids[2]);
            assert!(!ids.contains(&duplicated[2]));
            view.update(cx, |this, cx| {
                this.field_search
                    .update(cx, |state, cx| state.set_value("", window, cx));
                this.sync_sequence_list(window, cx);
            });
            window.render_frame(cx);
            assert_eq!(
                view.read(cx).sequence_list.read(cx).delegate().identity.ids,
                duplicated
            );
            view.update(cx, |this, cx| {
                this.session
                    .set_field(
                        "actions",
                        Some(serde_json::json!([
                            {"click": "#same"}, {"click": "#same"}, {"wait": 500}
                        ])),
                    )
                    .unwrap();
                this.sync_editor(window, cx);
                this.sync_sequence_list(window, cx);
            });
            let list = view.read(cx).sequence_list.read(cx);
            assert_eq!(list.delegate().identity.ids[2], ids[2]);
            assert!(
                list.delegate().identity.ids[..2]
                    .iter()
                    .all(|id| !duplicated.contains(id))
            );
            assert_eq!(list.delegate().identity.current, None);
            assert_eq!(list.selected_index(), None);
        })
        .unwrap();
    }

    #[cfg(feature = "gui-tests")]
    #[gpui_kit::test]
    async fn flat_collection_sequence_rejects_stale_callbacks_and_preserves_drafts(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use crate::desktop::session::Selection;
        use gpui_kit::AppContext;
        use gpui_kit::test::TestWindowExt;
        let directory = tempfile::tempdir().unwrap();
        let (handle, view) = crate::desktop::ui::tests::editor(cx, directory.path());
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.edit(
                    |session| {
                        session.set_field(
                            "actions",
                            Some(serde_json::json!([
                                {"click": "#first"}, {"wait": 500}
                            ])),
                        )
                    },
                    window,
                    cx,
                );
            });
            window.render_frame(cx);
            view.update(cx, |this, cx| {
                let rows = this.sequence_list.read(cx).delegate();
                let snapshot = rows.snapshot.clone().unwrap();
                let id = rows.identity.ids[0];
                let source = snapshot.source.clone();
                this.busy = true;
                this.sequence_command(
                    snapshot.clone(),
                    id,
                    0,
                    SequenceCommand::Duplicate,
                    window,
                    cx,
                );
                assert_eq!(
                    this.session.open.as_ref().unwrap().document.source(),
                    source
                );
                this.busy = false;
                let scope = this.session.open.as_ref().unwrap().entry.path.clone();
                this.session.open.as_mut().unwrap().entry.path = PathBuf::from("different.yaml");
                this.sequence_command(snapshot.clone(), id, 0, SequenceCommand::Open, window, cx);
                assert_eq!(this.session.selection, Selection::Scenario(0));
                this.session.open.as_mut().unwrap().entry.path = scope;
                this.tab = 1;
                let raw = source.replace("#first", "#pending");
                this.source
                    .update(cx, |state, cx| state.set_value(raw.clone(), window, cx));
                this.sequence_command(
                    snapshot.clone(),
                    id,
                    0,
                    SequenceCommand::Duplicate,
                    window,
                    cx,
                );
                assert_eq!(this.source.read(cx).value().as_ref(), raw);
                assert_eq!(
                    this.session.open.as_ref().unwrap().document.source(),
                    source
                );
                this.source
                    .update(cx, |state, cx| state.set_value(source.clone(), window, cx));
                this.tab = 0;
                this.select(
                    Selection::Action {
                        scenario: 0,
                        index: 0,
                    },
                    window,
                    cx,
                );
                let binding = this
                    .bindings
                    .iter()
                    .find(|binding| binding.field.key == "click")
                    .unwrap();
                let super::Control::Input(input) = &binding.control else {
                    panic!("Expected selector input")
                };
                let input = input.clone();
                input.update(cx, |state, cx| state.set_value("#pending", window, cx));
                this.sequence_command(snapshot.clone(), id, 0, SequenceCommand::Later, window, cx);
                assert_eq!(input.read(cx).value().as_ref(), "#pending");
                assert_eq!(
                    this.session.open.as_ref().unwrap().document.source(),
                    source
                );
                input.update(cx, |state, cx| state.set_value("#first", window, cx));
                this.select(Selection::Scenario(0), window, cx);
                this.sync_sequence_list(window, cx);
                this.sequence_list.update(cx, |list, cx| {
                    list.delegate()
                        .dispatch(0, SequenceCommand::Open, window, cx)
                });
                let changed = format!("{source}\n# external edit\n");
                this.session
                    .open
                    .as_mut()
                    .unwrap()
                    .document
                    .set_source(changed.clone())
                    .unwrap();
                this.sequence_command(snapshot, id, 0, SequenceCommand::Duplicate, window, cx);
                assert_eq!(
                    this.session.open.as_ref().unwrap().document.source(),
                    changed
                );
            });
        })
        .unwrap();
        cx.background_executor.run_until_parked();
        cx.update(|cx| {
            assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
            assert_eq!(
                view.read(cx).session.value().unwrap()["scenarios"][0]["actions"]
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
        });
    }

    #[cfg(feature = "gui-tests")]
    #[gpui_kit::test]
    fn flat_collection_sequence_virtual_scroll_focus_and_disabled_commands(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use crate::desktop::session::Selection;
        use gpui_kit::component::IndexPath;
        use gpui_kit::test::TestWindowExt;
        use gpui_kit::{AppContext, Focusable, px, size};
        for (width, height, dark) in [
            (1080., 720., false),
            (1080., 720., true),
            (1600., 1000., false),
            (1600., 1000., true),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let (handle, view) = crate::desktop::ui::tests::editor_at_size(
                cx,
                directory.path(),
                size(px(width), px(height)),
            );
            cx.update_window(handle.into(), |_, window, cx| {
                crate::desktop::ui::design::apply_theme(dark, None, cx);
                view.update(cx, |this, cx| {
                    this.edit(
                        |session| {
                            session.set_field(
                                "actions",
                                Some(serde_json::Value::Array(
                                    (0..64)
                                        .map(|index| serde_json::json!({"wait": index}))
                                        .collect(),
                                )),
                            )
                        },
                        window,
                        cx,
                    );
                    this.field_search
                        .update(cx, |state, cx| state.set_value("actions", window, cx));
                });
                window.render_frame(cx);
                assert!(window.try_find(("sequence-step", 63_usize)).is_none());
                view.read(cx)
                    .sequence_list
                    .clone()
                    .update(cx, |list, cx| list.focus(window, cx));
                window.press("end", cx);
                window.render_frame(cx);
                assert_eq!(
                    view.read(cx).sequence_list.read(cx).selected_index(),
                    Some(IndexPath::new(63))
                );
                let viewport = window.find("sequence-steps").bounds();
                let last = window.find(("sequence-step", 63_usize)).bounds();
                assert!(last.top() >= viewport.top());
                assert!(last.bottom() <= viewport.bottom());
                assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
                let source = view
                    .read(cx)
                    .session
                    .open
                    .as_ref()
                    .unwrap()
                    .document
                    .source()
                    .to_string();
                window.click(("sequence-down", 63_usize), cx);
                assert_eq!(
                    view.read(cx)
                        .session
                        .open
                        .as_ref()
                        .unwrap()
                        .document
                        .source(),
                    source
                );
                window.press("home", cx);
                window.render_frame(cx);
                assert_eq!(
                    view.read(cx).sequence_list.read(cx).selected_index(),
                    Some(IndexPath::new(0))
                );
                assert!(window.find(("sequence-step", 0_usize)).visible());
                view.update(cx, |this, _| this.busy = true);
                window.render_frame(cx);
                window.click(("sequence-copy", 0_usize), cx);
                window.press("end", cx);
                window.press("enter", cx);
                assert_eq!(
                    view.read(cx).sequence_list.read(cx).selected_index(),
                    Some(IndexPath::new(0))
                );
                assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
                assert_eq!(
                    view.read(cx)
                        .session
                        .open
                        .as_ref()
                        .unwrap()
                        .document
                        .source(),
                    source
                );
                view.update(cx, |this, _| this.busy = false);
                window.render_frame(cx);
                let search_focus = view.read(cx).field_search.read(cx).focus_handle(cx);
                window.focus(&search_focus, cx);
                window.press("end", cx);
                window.press("down", cx);
                assert!(search_focus.is_focused(window));
                assert_eq!(
                    view.read(cx).sequence_list.read(cx).selected_index(),
                    Some(IndexPath::new(0))
                );
                view.update(cx, |this, cx| {
                    this.tab = 1;
                    let focus = this.source.read(cx).focus_handle(cx);
                    window.focus(&focus, cx);
                });
                window.render_frame(cx);
                window.press("end", cx);
                window.press("down", cx);
                assert!(
                    view.read(cx)
                        .source
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window)
                );
                assert_eq!(
                    view.read(cx).sequence_list.read(cx).selected_index(),
                    Some(IndexPath::new(0))
                );
                assert_eq!(
                    view.read(cx)
                        .session
                        .open
                        .as_ref()
                        .unwrap()
                        .document
                        .source(),
                    source
                );
                view.update(cx, |this, _| this.tab = 0);
                window.render_frame(cx);
                view.read(cx)
                    .sequence_list
                    .clone()
                    .update(cx, |list, cx| list.focus(window, cx));
                window.press("space", cx);
                assert_eq!(
                    view.read(cx).session.selection,
                    Selection::Action {
                        scenario: 0,
                        index: 0
                    }
                );
                view.update(cx, |this, cx| {
                    this.select(Selection::Scenario(0), window, cx);
                    this.field_search
                        .update(cx, |state, cx| state.set_value("actions", window, cx));
                });
                window.render_frame(cx);
                window.click(("sequence-step", 2_usize), cx);
                assert_eq!(
                    view.read(cx).session.selection,
                    Selection::Action {
                        scenario: 0,
                        index: 2
                    }
                );
                view.update(cx, |this, cx| {
                    this.select(Selection::Scenario(0), window, cx);
                    this.edit(
                        |session| session.set_field("actions", Some(serde_json::json!([]))),
                        window,
                        cx,
                    );
                    this.field_search
                        .update(cx, |state, cx| state.set_value("actions", window, cx));
                });
                window.render_frame(cx);
                assert_eq!(window.find("sequence-count").label(), Some("0 actions"));
                assert_eq!(view.read(cx).sequence_list.read(cx).selected_index(), None);
                view.read(cx)
                    .sequence_list
                    .clone()
                    .update(cx, |list, cx| list.focus(window, cx));
                window.press("end", cx);
                window.press("enter", cx);
                assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
            })
            .unwrap();
        }
    }
}

fn action_group(kind: &str) -> usize {
    match kind {
        "goto" | "switchTab" | "navigate" | "dialog" | "wait" => 0,
        "hide" | "remove" => 2,
        "save" | "persist" => 3,
        _ => 1,
    }
}

impl Workbench {
    fn sequence_scope(&self) -> Option<SequenceScope> {
        Some((
            self.session.workspace.as_ref()?.root.clone(),
            self.session.open.as_ref()?.entry.path.clone(),
            self.session.selection.scenario()?,
        ))
    }

    fn activate_sequence(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let list = self.sequence_list.read(cx);
        if !list.focus_handle(cx).is_focused(window) {
            cx.propagate();
            return;
        }
        let index = list.selected_index().map_or(0, |index| index.row);
        let rows = list.delegate();
        let target = rows
            .snapshot
            .clone()
            .zip(rows.identity.ids.get(index).copied());
        if let Some((snapshot, id)) = target {
            self.sequence_command(snapshot, id, index, SequenceCommand::Open, window, cx);
        }
        cx.stop_propagation();
    }

    pub(super) fn sync_sequence_list(&self, window: &mut Window, cx: &mut Context<Self>) {
        let scope = self.sequence_scope();
        let actions = scope
            .as_ref()
            .and_then(|(_, _, scenario)| self.value["scenarios"][*scenario]["actions"].as_array())
            .cloned()
            .unwrap_or_default();
        let source = self
            .session
            .open
            .as_ref()
            .map(|open| open.document.source())
            .unwrap_or_default();
        self.sequence_list.update(cx, |list, cx| {
            let rows = list.delegate_mut();
            if rows.identity.scope == scope
                && rows.identity.actions == actions
                && rows
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.source.as_str())
                    .unwrap_or_default()
                    == source
                && rows
                    .snapshot
                    .as_ref()
                    .is_none_or(|snapshot| snapshot.ids == rows.identity.ids)
                && rows.busy == self.busy
                && rows.invalid == !self.field_errors.is_empty()
            {
                return;
            }
            rows.identity.reconcile(scope.clone(), actions.clone());
            rows.snapshot = scope.map(|scope| {
                Rc::new(SequenceSnapshot {
                    scope,
                    source: source.to_string(),
                    actions,
                    ids: rows.identity.ids.clone(),
                })
            });
            rows.busy = self.busy;
            rows.invalid = !self.field_errors.is_empty();
            let index = rows
                .identity
                .current
                .and_then(|id| {
                    rows.identity
                        .ids
                        .iter()
                        .position(|candidate| *candidate == id)
                })
                .map(IndexPath::new);
            list.set_selected_index(index, window, cx);
            list.set_selectable(!self.busy, cx);
            list.scroll_to_selected_item(window, cx);
            cx.notify();
        });
    }

    fn sequence_command(
        &mut self,
        snapshot: Rc<SequenceSnapshot>,
        id: u64,
        index: usize,
        command: SequenceCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy
            || self.sequence_scope().as_ref() != Some(&snapshot.scope)
            || self
                .session
                .open
                .as_ref()
                .is_none_or(|open| open.document.source() != snapshot.source)
            || self.value["scenarios"][snapshot.scope.2]["actions"].as_array()
                != Some(&snapshot.actions)
            || self
                .sequence_list
                .read(cx)
                .delegate()
                .identity
                .ids
                .get(index)
                != Some(&id)
        {
            return;
        }
        if !matches!(command, SequenceCommand::Open) && !self.field_errors.is_empty() {
            return;
        }
        if self.tab == 1 && self.source.read(cx).value().as_ref() != snapshot.source {
            return;
        }
        if self.tab == 0 {
            for binding in self.bindings.iter().filter(|binding| {
                binding.field.key == "actions"
                    || matches!(self.session.selection, Selection::Action { .. })
            }) {
                let draft = match &binding.control {
                    Control::Input(input) => Some(input.read(cx).value().to_string()),
                    Control::Textarea(input) => Some(input.read(cx).value().to_string()),
                    _ => None,
                };
                let current = self
                    .value
                    .pointer(&self.session.selection.path())
                    .and_then(|value| value.get(&binding.field.key))
                    .unwrap_or(&Value::Null);
                if draft.is_some_and(|draft| draft != binding.field.display(current)) {
                    self.status =
                        "Apply the changed action draft before using sequence commands".into();
                    cx.notify();
                    return;
                }
            }
        }
        if matches!(command, SequenceCommand::Earlier) && index == 0
            || matches!(command, SequenceCommand::Later) && index + 1 == snapshot.actions.len()
        {
            return;
        }
        let scenario = snapshot.scope.2;
        if matches!(command, SequenceCommand::Open | SequenceCommand::Remove) {
            let target = Selection::Action { scenario, index };
            self.select(target, window, cx);
            if self.session.selection == target {
                self.sequence_list.update(cx, |list, cx| {
                    list.set_selected_index(Some(IndexPath::new(index)), window, cx)
                });
                if matches!(command, SequenceCommand::Remove) {
                    self.confirm_remove(false, window, cx);
                } else {
                    window.focus(&self.focus, cx);
                }
            }
            return;
        }
        let rows = self.sequence_list.read(cx).delegate();
        let previous_ids = rows.identity.ids.clone();
        let current = rows.identity.current;
        let mut succeeded = false;
        self.edit(
            |session| {
                anyhow::ensure!(
                    session.value()?["scenarios"][scenario]["actions"].as_array()
                        == Some(&snapshot.actions),
                    "Action sequence changed while applying drafts"
                );
                let previous = session.selection;
                session.select(Selection::Action { scenario, index })?;
                let result = match command {
                    SequenceCommand::Earlier => session.move_selected(-1),
                    SequenceCommand::Later => session.move_selected(1),
                    SequenceCommand::Duplicate => session.duplicate_selected(),
                    _ => unreachable!(),
                };
                session.selection = previous;
                succeeded = result.is_ok();
                result
            },
            window,
            cx,
        );
        if succeeded {
            self.sequence_list.update(cx, |list, _| {
                let identity = &mut list.delegate_mut().identity;
                identity.actions = self.value["scenarios"][scenario]["actions"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                identity.known_edit(previous_ids, current, index, command);
            });
            self.sync_sequence_list(window, cx);
            self.sequence_list
                .update(cx, |list, cx| list.focus(window, cx));
        }
    }

    fn sequence_add(&self) -> AnyElement {
        let focus = self.focus.clone();
        Button::new("sequence-add")
            .ghost()
            .icon(IconName::Plus)
            .label("Add action")
            .disabled(self.busy || !self.field_errors.is_empty())
            .dropdown_menu(move |menu, window, cx| {
                design::restore_dropdown_menu_focus(
                    menu.focus_handle(cx),
                    focus.clone(),
                    window,
                    cx,
                );
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

    pub(super) fn render_sequence(
        &self,
        scenario: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let actions = self.value["scenarios"][scenario]["actions"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let row_height = self.sequence_list.update(cx, |list, cx| {
            use gpui_kit::component::list::ListDelegate as _;
            list.delegate_mut()
                .render_item(IndexPath::new(0), window, cx)
                .map_or(px(0.), |row| {
                    row.into_any_element()
                        .layout_as_root(
                            gpui_kit::size(
                                gpui_kit::AvailableSpace::MinContent,
                                gpui_kit::AvailableSpace::MinContent,
                            ),
                            window,
                            cx,
                        )
                        .height
                })
        });
        let focus = self.sequence_list.read(cx).focus_handle(cx);
        let rows = div()
            .id("sequence-steps")
            .test_support()
            .key_context("RegressifyList")
            .track_focus(&focus)
            .relative()
            .capture_action(cx.listener(
                |this, _: &gpui_kit::base::actions::Confirm, window, cx| {
                    this.activate_sequence(window, cx);
                },
            ))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.sequence_list
                        .update(cx, |list, cx| list.focus(window, cx));
                }),
            )
            .on_action(
                cx.listener(move |this, action: &lists::MoveListSelection, window, cx| {
                    if !this.busy {
                        this.sequence_list.update(cx, |list, cx| {
                            let index = action
                                .index(
                                    list.selected_index().map(|index| index.row),
                                    list.delegate().identity.ids.len(),
                                )
                                .map(IndexPath::new);
                            list.set_selected_index(index, window, cx);
                            list.scroll_to_selected_item(window, cx);
                            cx.notify();
                        });
                    }
                }),
            )
            .on_action(cx.listener(move |this, _: &OpenListItem, window, cx| {
                this.activate_sequence(window, cx);
            }))
            .on_action(cx.listener(move |this, _: &ToggleListItem, window, cx| {
                this.activate_sequence(window, cx);
            }))
            .flex()
            .flex_col()
            .h((row_height * actions.len() as f32).min(px(264.)))
            .child(gpui_kit::component::list::List::new(&self.sequence_list));
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
                    .flex_wrap()
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
