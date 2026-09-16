//! Retained list choices augment free-form fields without replacing their drafts.

use super::*;

#[derive(Default)]
pub(super) struct ViewportChoices {
    key: Option<(PathBuf, String, bool)>,
    generation: u64,
    values: Vec<crate::config::viewports::ViewportDef>,
    error: Option<String>,
    loading: bool,
}

impl ViewportChoices {
    pub(super) fn invalidate(&mut self) {
        self.key = None;
        self.generation += 1;
    }
}

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
pub(super) struct ToggleListChoice {
    pub key: String,
    pub value: String,
}

impl Workbench {
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

    fn list_choices(&self, key: &str) -> Vec<(String, String)> {
        match key {
            "viewportNames" => self
                .viewport_choices
                .values
                .iter()
                .map(|viewport| {
                    (
                        viewport.label.clone(),
                        format!(
                            "{} ({} x {})",
                            viewport.label, viewport.width, viewport.height
                        ),
                    )
                })
                .collect(),
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
        explicit: &Value,
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
        let choices = self.list_choices(&field.key);
        if choices.is_empty() {
            return None;
        }
        let mut selected = schema::string_values(if explicit.is_null() {
            &self.effective[&field.key]
        } else {
            explicit
        });
        let viewports = field.key == "viewportNames";
        if viewports && selected.is_empty() {
            selected = choices.iter().map(|(value, _)| value.clone()).collect();
        }
        let key = field.key.clone();
        let focus = self.focus.clone();
        Some(
            Button::new(SharedString::from(format!("choices-{}", field.key)))
                .ghost()
                .small()
                .icon(if viewports {
                    gpui_kit::assets::IconName::Monitor
                } else {
                    gpui_kit::assets::IconName::GitBranch
                })
                .label(if viewports {
                    "Choose viewports"
                } else {
                    "Choose dependencies"
                })
                .disabled(self.busy || self.field_errors.contains_key(&field.key))
                .dropdown_menu(move |menu, _, _| {
                    let mut menu = menu.action_context(focus.clone()).scrollable(true);
                    for (value, label) in &choices {
                        let checked = selected.iter().any(|selected| {
                            if viewports {
                                value.eq_ignore_ascii_case(selected)
                            } else {
                                value == selected
                            }
                        });
                        menu = menu.menu_with_check_and_disabled(
                            label.clone(),
                            checked,
                            Box::new(ToggleListChoice {
                                key: key.clone(),
                                value: value.clone(),
                            }),
                            viewports && checked && selected.len() == 1,
                        );
                    }
                    menu
                })
                .into_any_element(),
        )
    }
}
