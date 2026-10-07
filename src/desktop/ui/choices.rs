//! Retained list choices augment free-form fields without replacing their drafts.

use super::*;
use gpui_kit::component::combobox::{Combobox, ComboboxEvent, ComboboxState};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct ValueChoice {
    value: String,
    label: SharedString,
}

impl ValueChoice {
    pub(super) fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }
}

impl gpui_kit::component::select::SelectItem for ValueChoice {
    type Value = String;
    fn title(&self) -> SharedString {
        self.label.clone()
    }
    fn value(&self) -> &String {
        &self.value
    }
    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        // Windows exposes text-label names from their value, not aria_label.
        div()
            .id(SharedString::from(format!("choice-option-{}", self.value)))
            .role(Role::Label)
            .aria_label(self.label.clone())
            .aria_value(self.label.clone())
            .test_support()
            .child(self.label.clone())
    }
}

pub(super) type ValueChoices = SearchableVec<ValueChoice>;

pub(super) fn value_choices(values: &[String]) -> ValueChoices {
    ValueChoices::new(
        values
            .iter()
            .map(|value| ValueChoice::new(value.clone(), value.clone()))
            .collect::<Vec<_>>(),
    )
}

pub(super) struct ListChoicePicker {
    pub(super) state: Entity<ComboboxState<ValueChoices>>,
    items: Vec<ValueChoice>,
    selected: Vec<String>,
    _subscription: Subscription,
}

pub(super) struct ValueSelector {
    pub(super) state: Entity<SelectState<ValueChoices>>,
    items: Vec<ValueChoice>,
    selected: Option<String>,
}

impl ValueSelector {
    pub(super) fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            state: cx.new(|cx| {
                SelectState::new(ValueChoices::new(Vec::new()), None, window, cx).searchable(true)
            }),
            items: Vec::new(),
            selected: None,
        }
    }

    pub(super) fn sync(
        &mut self,
        items: Vec<ValueChoice>,
        selected: Option<String>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let present = selected
            .as_ref()
            .filter(|selected| items.iter().any(|item| &item.value == *selected));
        if self.items == items
            && self.selected == selected
            && self.state.read(cx).selected_value() == present
        {
            return;
        }
        self.state.update(cx, |state, cx| {
            state.set_items(ValueChoices::new(items.clone()), window, cx);
            match &selected {
                Some(value) => state.set_selected_value(value, window, cx),
                None => state.set_selected_index(None, window, cx),
            }
        });
        self.items = items;
        self.selected = selected;
    }
}

#[derive(Default)]
pub(super) struct ViewportChoices {
    key: Option<(PathBuf, String, bool)>,
    generation: u64,
    values: Vec<crate::config::viewports::ViewportDef>,
    error: Option<String>,
    loading: bool,
    validation_key: Option<(PathBuf, PathBuf, String)>,
    validation_generation: u64,
    validation_issues: Vec<crate::desktop::validation::Diagnostic>,
}

impl ViewportChoices {
    pub(super) fn invalidate(&mut self) {
        self.key = None;
        self.generation += 1;
        self.validation_key = None;
        self.validation_generation += 1;
        self.validation_issues.clear();
    }

    pub(super) fn action_issues(
        &self,
        root: &std::path::Path,
        open: &crate::desktop::workspace::OpenDocument,
    ) -> &[crate::desktop::validation::Diagnostic] {
        if self
            .validation_key
            .as_ref()
            .is_some_and(|(workspace, path, source)| {
                workspace == root && path == &open.entry.path && source == open.document.source()
            })
        {
            &self.validation_issues
        } else {
            &[]
        }
    }
}

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
pub(super) struct ToggleListChoice {
    pub key: String,
    pub value: String,
}

impl Workbench {
    pub(super) fn sync_list_choice_pickers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.list_choice_pickers.retain(|key, _| {
            self.bindings
                .iter()
                .any(|binding| &binding.field.key == key)
        });
        let fields = self
            .bindings
            .iter()
            .filter(|binding| matches!(binding.field.key.as_str(), "viewportNames" | "needs"))
            .map(|binding| binding.field.clone())
            .collect::<Vec<_>>();
        for field in fields {
            if field.key == "viewportNames" && self.viewport_choices.loading {
                self.list_choice_pickers.remove(&field.key);
                continue;
            }
            let choices = self.list_choices(&field.key);
            let explicit = self
                .value
                .pointer(&self.session.selection.path())
                .and_then(|value| value.get(&field.key))
                .unwrap_or(&Value::Null);
            let current = self
                .bindings
                .iter()
                .find(|binding| binding.field.key == field.key)
                .and_then(|binding| {
                    if let Control::Textarea(input) = &binding.control {
                        let text = input.read(cx).value();
                        (text.as_ref() != field.display(explicit))
                            .then(|| field.parse(&text).ok())
                            .flatten()
                    } else {
                        None
                    }
                });
            let explicit = current.as_ref().unwrap_or(explicit);
            let mut selected = schema::string_values(if explicit.is_null() {
                &self.effective[&field.key]
            } else {
                explicit
            });
            let viewports = field.key == "viewportNames";
            if viewports && selected.is_empty() {
                selected = choices.iter().map(|(value, _)| value.clone()).collect();
            }
            let matches = |left: &str, right: &str| {
                if viewports {
                    left.eq_ignore_ascii_case(right)
                } else {
                    left == right
                }
            };
            let mut items = choices
                .into_iter()
                .map(|(value, label)| ValueChoice::new(value, label))
                .collect::<Vec<_>>();
            for value in &selected {
                if !items.iter().any(|item| matches(&item.value, value)) {
                    items.push(ValueChoice::new(
                        value.clone(),
                        format!("{value} (unavailable)"),
                    ));
                }
            }
            if items.is_empty() {
                self.list_choice_pickers.remove(&field.key);
                continue;
            }
            let selected = items
                .iter()
                .filter(|item| selected.iter().any(|value| matches(&item.value, value)))
                .map(|item| item.value.clone())
                .collect::<Vec<_>>();
            if !self.list_choice_pickers.contains_key(&field.key) {
                let state = cx.new(|cx| {
                    ComboboxState::new(ValueChoices::new(Vec::new()), Vec::new(), window, cx)
                        .multiple(true)
                        .searchable(true)
                });
                let scope = self.choice_binding_scope.clone();
                let key = field.key.clone();
                let subscription =
                    cx.subscribe_in(&state, window, move |this, state, event, window, cx| {
                        if this.choice_binding_scope != scope
                            || this
                                .list_choice_pickers
                                .get(&key)
                                .is_none_or(|picker| &picker.state != state)
                        {
                            return;
                        }
                        if let ComboboxEvent::Change(values) = event {
                            let picker = &this.list_choice_pickers[&key];
                            let changes = picker
                                .items
                                .iter()
                                .filter(|item| {
                                    picker.selected.contains(&item.value)
                                        != values.contains(&item.value)
                                })
                                .map(|item| item.value.clone())
                                .collect::<Vec<_>>();
                            if let [value] = changes.as_slice() {
                                this.toggle_list_choice(
                                    &ToggleListChoice {
                                        key: key.clone(),
                                        value: value.clone(),
                                    },
                                    window,
                                    cx,
                                );
                            }
                            this.sync_list_choice_pickers(window, cx);
                        }
                    });
                self.list_choice_pickers.insert(
                    field.key.clone(),
                    ListChoicePicker {
                        state,
                        items: Vec::new(),
                        selected: Vec::new(),
                        _subscription: subscription,
                    },
                );
            }
            let picker = self.list_choice_pickers.get_mut(&field.key).unwrap();
            let actual = picker.state.read(cx).selected_values();
            let same_selection = actual.len() == selected.len()
                && actual.iter().all(|value| selected.contains(value));
            if picker.items != items || !same_selection {
                picker.state.update(cx, |state, cx| {
                    let query = state.query(cx);
                    if picker.items != items {
                        state.set_items(ValueChoices::new(items.clone()), window, cx);
                    }
                    state.set_selected_values(&selected, window, cx);
                    if !query.is_empty() {
                        state.set_query(query, window, cx);
                    }
                });
            }
            picker.items = items;
            picker.selected = selected;
        }
    }

    pub(super) fn load_viewport_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.session.workspace.clone() else {
            self.viewport_choices = ViewportChoices {
                generation: self.viewport_choices.generation + 1,
                ..Default::default()
            };
            return;
        };
        let path = self.effective["viewportsPath"]
            .as_str()
            .unwrap_or("common/_viewports.yaml")
            .to_string();
        let explicit = self.effective["viewportsPathIsExplicit"]
            .as_bool()
            .unwrap_or(false);
        let key = (workspace.root.clone(), path.clone(), explicit);
        if self.viewport_choices.key.as_ref() == Some(&key) {
            return;
        }
        self.viewport_choices.key = Some(key.clone());
        self.viewport_choices.generation += 1;
        let generation = self.viewport_choices.generation;
        self.viewport_choices.values.clear();
        self.viewport_choices.error = None;
        self.viewport_choices.loading = true;
        let task = cx.background_executor().spawn(async move {
            workspace.viewport_choices(std::path::Path::new(&path), explicit)
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                if this.viewport_choices.generation != generation
                    || this.viewport_choices.key.as_ref() != Some(&key)
                {
                    return;
                }
                this.viewport_choices.loading = false;
                match result {
                    Ok(values) => this.viewport_choices.values = values,
                    Err(error) => this.viewport_choices.error = Some(format!("{error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn load_action_viewport_diagnostics(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(workspace), Some(open)) =
            (self.session.workspace.clone(), self.session.open.as_ref())
        else {
            self.viewport_choices.validation_key = None;
            self.viewport_choices.validation_generation += 1;
            self.viewport_choices.validation_issues.clear();
            return;
        };
        if open.entry.kind != DocumentKind::Suite {
            self.viewport_choices.validation_key = None;
            self.viewport_choices.validation_generation += 1;
            self.viewport_choices.validation_issues.clear();
            return;
        }
        let key = (
            workspace.root.clone(),
            open.entry.path.clone(),
            open.document.source().to_string(),
        );
        if self.viewport_choices.validation_key.as_ref() == Some(&key) {
            return;
        }
        self.viewport_choices.validation_key = Some(key.clone());
        self.viewport_choices.validation_generation += 1;
        self.viewport_choices.validation_issues.clear();
        let generation = self.viewport_choices.validation_generation;
        let format = open.document.format;
        let source = key.2.clone();
        let suite_name = open
            .entry
            .suite_name()
            .unwrap_or_else(|| open.entry.label());
        let task = cx.background_executor().spawn(async move {
            let document = Document::new(DocumentKind::Suite, format, source)?;
            workspace.action_viewport_diagnostics(&document, &suite_name)
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                if this.viewport_choices.validation_generation != generation
                    || this.viewport_choices.validation_key.as_ref() != Some(&key)
                {
                    return;
                }
                this.viewport_choices.validation_issues = result.unwrap_or_else(|error| {
                    vec![crate::desktop::validation::Diagnostic {
                        pointer: String::new(),
                        message: format!("{error:#}"),
                    }]
                });
                this.refresh_value();
                this.update_source_diagnostics(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn list_choices(&self, key: &str) -> Vec<(String, String)> {
        match key {
            "viewportNames" => {
                let viewports = if matches!(self.session.selection, Selection::Action { .. }) {
                    schema::action_viewport_choices(&self.effective, &self.viewport_choices.values)
                        .unwrap_or_default()
                } else {
                    self.viewport_choices.values.clone()
                };
                viewports
                    .into_iter()
                    .map(|viewport| {
                        (
                            viewport.label.clone(),
                            format!(
                                "{} ({} x {})",
                                viewport.label, viewport.width, viewport.height
                            ),
                        )
                    })
                    .collect()
            }
            "needs" => schema::dependency_choices(&self.value, self.session.selection.scenario())
                .into_iter()
                .map(|scenario| {
                    (
                        scenario.id.clone(),
                        format!("{} ({})", scenario.label, scenario.id),
                    )
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    pub(super) fn toggle_list_choice(
        &mut self,
        action: &ToggleListChoice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        let Some(binding) = self
            .bindings
            .iter()
            .find(|binding| binding.field.key == action.key)
        else {
            return;
        };
        let field = binding.field.clone();
        let selected = self
            .value
            .pointer(&self.session.selection.path())
            .and_then(|value| value.get(&field.key))
            .unwrap_or(&Value::Null);
        let current = if let Control::Textarea(input) = &binding.control {
            let text = input.read(cx).value();
            if text.as_ref() == field.display(selected) {
                Ok(selected.clone())
            } else {
                field.parse(&text)
            }
        } else {
            return;
        };
        let choices = self
            .list_choices(&field.key)
            .into_iter()
            .map(|(value, _)| value)
            .collect::<Vec<_>>();
        let result = current.and_then(|current| {
            field.toggle_list_choice(
                &current,
                &self.effective[&field.key],
                &action.value,
                &choices,
            )
        });
        match result {
            Ok(value) => {
                let text = value.to_string();
                self.change_field(&field, &text, window, cx);
                if !self.field_errors.contains_key(&field.key)
                    && let Some(binding) = self
                        .bindings
                        .iter()
                        .find(|binding| binding.field.key == field.key)
                    && let Control::Textarea(input) = &binding.control
                {
                    input.update(cx, |state, cx| state.set_value(text, window, cx));
                }
            }
            Err(error) => {
                self.status = format!("{error:#}");
                cx.notify();
            }
        }
    }

    pub(super) fn render_list_choices(
        &self,
        field: &Field,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !matches!(field.key.as_str(), "viewportNames" | "needs") {
            return None;
        }
        if field.key == "viewportNames"
            && let Some(error) = &self.viewport_choices.error
        {
            return Some(
                div()
                    .id("viewport-choice-error")
                    .role(Role::Alert)
                    .aria_label(error.clone())
                    .test_support()
                    .text_xs()
                    .text_color(cx.theme().warning)
                    .child(error.clone())
                    .into_any_element(),
            );
        }
        let picker = self.list_choice_pickers.get(&field.key)?;
        let viewports = field.key == "viewportNames";
        let label = if viewports {
            "Choose viewports"
        } else {
            "Choose dependencies"
        };
        Some(
            div()
                .id(SharedString::from(format!("choices-{}", field.key)))
                .role(Role::Group)
                .aria_label(label)
                .test_support()
                .w_full()
                .min_w_0()
                .child(
                    Combobox::new(&picker.state)
                        .w_full()
                        .placeholder(label)
                        .disabled(self.busy || self.field_errors.contains_key(&field.key)),
                )
                .into_any_element(),
        )
    }
}
