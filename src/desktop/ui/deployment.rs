use super::*;
use crate::desktop::{
    deployment::{DeployTarget, DeployTargets, DeploymentJob},
    runner::RunCommand,
};

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
pub(super) struct ChooseDeployTarget {
    id: String,
}

pub(super) struct DeploymentPanel {
    settings: DeployTargets,
    path: Option<PathBuf>,
    root: Option<PathBuf>,
    intent: Option<runs::RunIntent>,
    catalog_ids: Vec<String>,
    suite_ids: Vec<String>,
    partial: bool,
    confirmed: bool,
    editing: bool,
    removing: bool,
    editing_id: Option<String>,
    saving: bool,
    status: String,
    name: Entity<InputState>,
    token: Entity<InputState>,
    environment: Entity<InputState>,
}

impl DeploymentPanel {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            settings: DeployTargets::default(),
            path: None,
            root: None,
            intent: None,
            catalog_ids: Vec::new(),
            suite_ids: Vec::new(),
            partial: false,
            confirmed: false,
            editing: false,
            removing: false,
            editing_id: None,
            saving: false,
            status: String::new(),
            name: cx.new(|cx| InputState::new(window, cx).placeholder("QA reports")),
            token: cx.new(|cx| {
                InputState::new(window, cx).placeholder("AZURE_STATIC_WEB_APPS_API_TOKEN")
            }),
            environment: cx
                .new(|cx| InputState::new(window, cx).placeholder("Production (default)")),
        }
    }

    fn selected(&self) -> Option<&DeployTarget> {
        self.settings
            .targets
            .iter()
            .find(|target| Some(&target.id) == self.settings.selected.as_ref())
    }

    fn can_deploy(&self) -> bool {
        self.intent.is_some()
            && self.selected().is_some()
            && !self.editing
            && !self.removing
            && !self.saving
            && (!self.partial || self.confirmed)
    }
}

impl Workbench {
    pub(super) fn render_deploy_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled =
            self.busy || self.runs.read(cx).running() || self.session.workspace.is_none();
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                Button::new("deploy-report")
                    .small()
                    .icon(gpui_kit::assets::IconName::CloudUpload)
                    .label("Deploy")
                    .tooltip("Deploy selected suite reports")
                    .disabled(disabled || self.runs.read(cx).selected_count() == 0)
                    .on_click(cx.listener(|this, _, window, cx| this.open_deployment(window, cx))),
            )
            .child(
                Button::new("deploy-targets")
                    .small()
                    .ghost()
                    .icon(gpui_kit::assets::IconName::Settings2)
                    .tooltip("Deployment targets")
                    .accessibility_label("Deployment targets")
                    .disabled(disabled)
                    .on_click(cx.listener(|this, _, window, cx| this.open_deployment(window, cx))),
            )
    }

    pub(super) fn open_deployment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.runs.read(cx).running() {
            return;
        }
        let Some(root) = self
            .session
            .workspace
            .as_ref()
            .map(|workspace| workspace.root.clone())
        else {
            return;
        };
        let Some(preferences) = self.preferences_path.clone() else {
            self.status = "Deployment settings directory unavailable".into();
            cx.notify();
            return;
        };
        let intent = if self.runs.read(cx).selected_count() == 0 {
            None
        } else {
            match self
                .runs
                .update(cx, |panel, cx| panel.deployment_intent(cx))
            {
                Ok(intent) => Some(intent),
                Err(error) => {
                    self.status = format!("{error:#}");
                    cx.notify();
                    return;
                }
            }
        };
        let catalog_ids = self
            .session
            .suites
            .iter()
            .map(|suite| suite.output_id.clone())
            .collect::<Vec<_>>();
        let suite_ids = self
            .session
            .suites
            .iter()
            .filter(|suite| {
                intent
                    .as_ref()
                    .is_some_and(|intent| intent.suite_names.contains(&suite.name))
            })
            .map(|suite| suite.output_id.clone())
            .collect::<Vec<_>>();
        let loaded_root = root.clone();
        self.task(
            "Loading deployment targets...",
            move || {
                let path = DeployTargets::path_in(&loaded_root, &preferences)?;
                Ok((DeployTargets::load(&path)?, path))
            },
            move |this, (mut settings, path), window, cx| {
                if settings.selected.is_none() {
                    settings.selected = settings.targets.first().map(|target| target.id.clone());
                }
                this.deployment.update(cx, |state, cx| {
                    state.settings = settings;
                    state.path = Some(path);
                    state.root = Some(root);
                    state.intent = intent;
                    state.partial = suite_ids.len() != catalog_ids.len();
                    state.catalog_ids = catalog_ids;
                    state.suite_ids = suite_ids;
                    state.confirmed = false;
                    state.editing = false;
                    state.removing = false;
                    state.saving = false;
                    state.status.clear();
                    cx.notify();
                });
                this.status = "Deployment targets ready".into();
                this.show_deployment_dialog(window, cx);
            },
            window,
            cx,
        );
    }

    fn show_deployment_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = self.deployment.clone();
        let view = cx.entity();
        let focus = self.focus.clone();
        window.open_dialog(cx, move |dialog, window, cx| {
            let state = panel.read(cx);
            let targets = state.settings.targets.clone();
            let selected = state.selected();
            let editing = state.editing;
            let saving = state.saving;
            let mut body = div().flex().flex_col().gap_3().min_w_0()
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child("Azure Static Web Apps"))
                .child(div().flex().items_center().gap_2()
                    .child(Button::new("deploy-target-menu").flex_1().min_w_0().label(selected.map(|target| target.name.clone()).unwrap_or_else(|| "No deployment targets".into()))
                        .icon(gpui_kit::assets::IconName::ChevronDown).disabled(editing || saving || state.removing || targets.is_empty())
                        .dropdown_menu({ let focus = focus.clone(); move |menu, _, _| {
                            let mut menu = menu.action_context(focus.clone());
                            for target in &targets { menu = menu.menu(target.name.clone(), Box::new(ChooseDeployTarget { id: target.id.clone() })); }
                            menu
                        }}))
                    .child(Button::new("deploy-target-add").ghost().icon(IconName::Plus).tooltip("Add target").accessibility_label("Add target").disabled(saving || editing || state.removing)
                        .on_click(window.listener_for(&view, |this, _, window, cx| this.edit_deploy_target(false, window, cx))))
                    .child(Button::new("deploy-target-edit").ghost().icon(gpui_kit::assets::IconName::Pencil).tooltip("Edit target").accessibility_label("Edit target").disabled(selected.is_none() || saving || editing || state.removing)
                        .on_click(window.listener_for(&view, |this, _, window, cx| this.edit_deploy_target(true, window, cx))))
                    .child(Button::new("deploy-target-remove").ghost().icon(gpui_kit::assets::IconName::Trash).tooltip("Remove target").accessibility_label("Remove target").disabled(selected.is_none() || saving || editing || state.removing)
                        .on_click(window.listener_for(&view, |this, _, _, cx| this.deployment.update(cx, |state, cx| { state.removing = true; cx.notify(); })))));
            if state.removing {
                body = body.child(format!("Remove target '{}'? The Azure app will not be deleted.", selected.map(|target| target.name.as_str()).unwrap_or_default()))
                    .child(div().flex().justify_end().gap_2()
                        .child(Button::new("deploy-remove-cancel").label("Keep target").disabled(saving)
                            .on_click(window.listener_for(&view, |this, _, _, cx| this.deployment.update(cx, |state, cx| { state.removing = false; cx.notify(); }))))
                        .child(Button::new("deploy-remove-confirm").danger().label("Remove target").disabled(saving)
                            .on_click(window.listener_for(&view, |this, _, window, cx| this.remove_deploy_target(window, cx)))));
            } else if editing {
                for (label, id, input) in [("Target name", "deploy-name", &state.name), ("Deployment token environment variable", "deploy-token-env", &state.token), ("Azure environment", "deploy-environment", &state.environment)] {
                    body = body.child(div().flex().flex_col().gap_1().child(label).child(Input::new(input).id(id).aria_label(label).small().disabled(saving)));
                }
                body = body.child(div().flex().justify_end().gap_2()
                    .child(Button::new("deploy-target-cancel").label("Cancel edit").disabled(saving).on_click(window.listener_for(&view, |this, _, _, cx| {
                        this.deployment.update(cx, |state, cx| { state.editing = false; state.status.clear(); cx.notify(); });
                    })))
                    .child(Button::new("deploy-target-save").primary().icon(gpui_kit::assets::IconName::Save).label("Save target").disabled(saving)
                        .on_click(window.listener_for(&view, |this, _, window, cx| this.save_deploy_target(window, cx)))));
            } else {
                body = body.child(div().text_sm().child(selected.map(|target| format!("{} | Token: {}", if target.environment.is_empty() { "Production" } else { &target.environment }, target.token_env)).unwrap_or_default()));
                body = body.child(div().font_weight(FontWeight::SEMIBOLD).child(format!("Selected suites: {} of {}", state.suite_ids.len(), state.catalog_ids.len())));
                body = body.child(div().id("deployment-suite-list").max_h(px(112.)).overflow_y_scrollbar().text_sm()
                    .children(state.intent.iter().flat_map(|intent| intent.suite_names.iter()).map(|name| div().truncate().child(name.clone()))));
                if state.suite_ids.is_empty() {
                    body = body.child(div().text_color(cx.theme().warning).child("Select at least one test suite to deploy."));
                } else if state.partial {
                    body = body.child(div().text_color(cx.theme().warning).child("Not all test suites are selected. Is publishing only this subset intentional?"))
                        .child(Checkbox::new("deploy-confirm-subset").label(format!("Yes, deploy only these {} of {} suites", state.suite_ids.len(), state.catalog_ids.len()))
                            .checked(state.confirmed).disabled(saving)
                            .on_click(window.listener_for(&view, |this, checked, _, cx| {
                                this.deployment.update(cx, |state, cx| { state.confirmed = *checked; cx.notify(); });
                            })));
                }
                body = body.child(div().text_sm().text_color(cx.theme().muted_foreground).child("Publication: full latest reports for the selected suites."));
            }
            if !state.status.is_empty() { body = body.child(div().id("deploy-status").role(Role::Status).text_color(cx.theme().danger).child(state.status.clone())); }
            dialog.title("Deploy reports").keyboard(!saving).close_button(!saving).child(div().id("deploy-dialog-body").role(Role::Group).aria_label("Deployment options").child(body))
                .footer(div().flex().justify_end().gap_2()
                    .child(Button::new("deploy-close").label("Close").disabled(saving).on_click(|_, window, cx| window.close_dialog(cx)))
                    .child(Button::new("deploy-confirm").primary().icon(gpui_kit::assets::IconName::CloudUpload).label("Deploy")
                        .accessibility_label("Deploy selected reports")
                        .disabled(!state.can_deploy()).on_click(window.listener_for(&view, |this, _, window, cx| this.begin_deployment(window, cx)))))
        });
    }

    pub(super) fn choose_deploy_target(
        &mut self,
        action: &ChooseDeployTarget,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.deployment.update(cx, |state, cx| {
            if !state.saving
                && !state.editing
                && !state.removing
                && state
                    .settings
                    .targets
                    .iter()
                    .any(|target| target.id == action.id)
            {
                state.settings.selected = Some(action.id.clone());
                state.status.clear();
                cx.notify();
            }
        });
    }

    fn edit_deploy_target(&mut self, existing: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.deployment.update(cx, |state, cx| {
            if state.saving {
                return;
            }
            let target = existing.then(|| state.selected().cloned()).flatten();
            state.editing_id = target.as_ref().map(|target| target.id.clone());
            state.editing = true;
            state.status.clear();
            state.name.update(cx, |input, cx| {
                input.set_value(
                    target
                        .as_ref()
                        .map(|target| target.name.clone())
                        .unwrap_or_default(),
                    window,
                    cx,
                )
            });
            state.token.update(cx, |input, cx| {
                input.set_value(
                    target
                        .as_ref()
                        .map(|target| target.token_env.clone())
                        .unwrap_or_default(),
                    window,
                    cx,
                )
            });
            state.environment.update(cx, |input, cx| {
                input.set_value(
                    target
                        .as_ref()
                        .map(|target| target.environment.clone())
                        .unwrap_or_default(),
                    window,
                    cx,
                )
            });
            cx.notify();
        });
    }

    fn save_deploy_target(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.deployment.read(cx);
        if state.saving || !state.editing {
            return;
        }
        let target = DeployTarget {
            id: state
                .editing_id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            name: state.name.read(cx).value().trim().into(),
            token_env: state.token.read(cx).value().trim().into(),
            environment: state.environment.read(cx).value().trim().into(),
        };
        if let Err(error) = target.validate() {
            self.deployment.update(cx, |state, cx| {
                state.status = format!("{error:#}");
                cx.notify();
            });
            return;
        }
        let mut settings = state.settings.clone();
        settings.targets.retain(|previous| previous.id != target.id);
        settings.selected = Some(target.id.clone());
        settings.targets.push(target);
        self.persist_deploy_targets(settings, None, window, cx);
    }

    fn remove_deploy_target(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.deployment.read(cx);
        if state.saving || !state.removing {
            return;
        }
        let Some(target) = state.selected().cloned() else {
            return;
        };
        let mut settings = state.settings.clone();
        settings.targets.retain(|item| item.id != target.id);
        settings.selected = settings.targets.first().map(|target| target.id.clone());
        self.persist_deploy_targets(settings, None, window, cx);
    }

    fn persist_deploy_targets(
        &mut self,
        settings: DeployTargets,
        pending: Option<runs::RunIntent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.deployment.read(cx).path.clone() else {
            return;
        };
        self.busy = true;
        self.sync_modal(cx);
        self.deployment.update(cx, |state, cx| {
            state.saving = true;
            cx.notify();
        });
        let task = cx.background_executor().spawn(async move {
            settings.save(&path)?;
            Ok::<_, anyhow::Error>(settings)
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.busy = false;
                let succeeded = result.is_ok();
                this.deployment.update(cx, |state, cx| {
                    state.saving = false;
                    match result {
                        Ok(settings) => {
                            state.settings = settings;
                            state.editing = false;
                            state.removing = false;
                            state.status.clear();
                        }
                        Err(error) => state.status = format!("{error:#}"),
                    }
                    cx.notify();
                });
                this.sync_modal(cx);
                if succeeded && let Some(intent) = pending {
                    window.close_dialog(cx);
                    this.guard_run(Box::new(intent), window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn begin_deployment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.runs.read(cx).running() {
            return;
        }
        let state = self.deployment.read(cx);
        if !state.can_deploy()
            || self
                .session
                .workspace
                .as_ref()
                .map(|workspace| &workspace.root)
                != state.root.as_ref()
        {
            return;
        }
        let settings = state.settings.clone();
        let result = (|| -> Result<runs::RunIntent> {
            let mut intent = state
                .intent
                .clone()
                .ok_or_else(|| anyhow!("Select at least one test suite"))?;
            let target = state
                .selected()
                .cloned()
                .ok_or_else(|| anyhow!("Choose a deployment target"))?;
            let job = DeploymentJob {
                target,
                suite_ids: state.suite_ids.clone(),
                catalog_ids: state.catalog_ids.clone(),
                partial_confirmed: state.confirmed,
            };
            job.validate()?;
            intent.request.command = RunCommand::Deploy;
            intent.expected_output_ids = Some(job.suite_ids.clone());
            intent.request.options.deployment = Some(job);
            intent.resolve(&self.session.suites)?;
            Ok(intent)
        })();
        match result {
            Ok(intent) => {
                self.persist_deploy_targets(settings, Some(intent), window, cx);
            }
            Err(error) => self.deployment.update(cx, |state, cx| {
                state.status = format!("{error:#}");
                cx.notify();
            }),
        }
    }
}

#[cfg(all(test, feature = "gui-tests"))]
mod tests {
    use crate::desktop::deployment::{DeployTarget, DeployTargets};
    use gpui_kit::test::{TestAppContextExt, TestWindowExt};
    use gpui_kit::{AppContext, TestAppContext};
    use std::time::Duration;

    #[gpui_kit::test]
    async fn deployment_targets_can_be_added_edited_and_removed_without_secrets(
        cx: &mut TestAppContext,
    ) {
        let root = tempfile::tempdir().unwrap();
        let (handle, view) = super::super::tests::editor(cx, root.path());
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| this.open_deployment(window, cx));
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
            window.try_find("deploy-target-add").is_some()
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            assert_eq!(view.read(cx).status, "Deployment targets ready");
            window.click("deploy-target-add", cx);
            window.render_frame(cx);
            window.click("deploy-name", cx);
            window.input("QA reports", cx);
            window.click("deploy-token-env", cx);
            window.input("QA_SWA_TOKEN", cx);
            window.click("deploy-environment", cx);
            window.input("preview", cx);
            window.click("deploy-target-save", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            let state = view.read(cx).deployment.read(cx);
            !state.saving && state.settings.targets.len() == 1
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("deploy-target-edit", cx);
            window.render_frame(cx);
            window.click("deploy-name", cx);
            window.press("ctrl-a", cx);
            window.input("QA preview", cx);
            window.click("deploy-target-save", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            let state = view.read(cx).deployment.read(cx);
            !state.saving && state.settings.targets[0].name == "QA preview"
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            let state = view.read(cx).deployment.read(cx);
            let settings = DeployTargets::load(state.path.as_ref().unwrap()).unwrap();
            assert_eq!(settings.targets[0].token_env, "QA_SWA_TOKEN");
            window.click("deploy-target-remove", cx);
            window.render_frame(cx);
            assert_eq!(view.read(cx).deployment.read(cx).settings.targets.len(), 1);
            window.click("deploy-remove-cancel", cx);
            window.render_frame(cx);
            window.click("deploy-target-remove", cx);
            window.render_frame(cx);
            window.click("deploy-remove-confirm", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            let state = view.read(cx).deployment.read(cx);
            !state.saving && state.settings.targets.is_empty()
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            let state = view.read(cx).deployment.read(cx);
            assert!(
                DeployTargets::load(state.path.as_ref().unwrap())
                    .unwrap()
                    .targets
                    .is_empty()
            );
            window.click("deploy-close", cx);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    async fn deploy_dialog_requires_selection_confirmation_and_preserves_unsaved_guard(
        cx: &mut TestAppContext,
    ) {
        let root = tempfile::tempdir().unwrap();
        let (handle, view) = super::super::tests::editor(cx, root.path());
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                let suites = this.session.suites.clone();
                let intent = this.runs.update(cx, |panel, cx| {
                    panel.select_all(true, cx);
                    panel.deployment_intent(cx).unwrap()
                });
                this.deployment.update(cx, |state, cx| {
                    state.path = Some(root.path().join("deployment-targets.json"));
                    state.root = this
                        .session
                        .workspace
                        .as_ref()
                        .map(|workspace| workspace.root.clone());
                    state.settings = DeployTargets {
                        selected: Some("qa".into()),
                        targets: vec![DeployTarget {
                            id: "qa".into(),
                            name: "QA".into(),
                            token_env: "TEST_TOKEN".into(),
                            environment: String::new(),
                        }],
                    };
                    state.catalog_ids =
                        suites.iter().map(|suite| suite.output_id.clone()).collect();
                    state.suite_ids = state.catalog_ids.clone();
                    assert!(!state.can_deploy());
                    state.intent = Some(intent);
                    state.partial = true;
                    assert!(!state.can_deploy());
                    state.confirmed = true;
                    assert!(state.can_deploy());
                    state.confirmed = false;
                    cx.notify();
                });
                this.edit(
                    crate::desktop::session::Session::duplicate_selected,
                    window,
                    cx,
                );
                this.show_deployment_dialog(window, cx);
            });
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
            window.try_find("deploy-confirm-subset").is_some()
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("deploy-confirm", cx);
            assert!(!view.read(cx).runs.read(cx).running());
            window.click("deploy-confirm-subset", cx);
        })
        .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("deploy-confirm", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
            window.try_find("guard-cancel").is_some()
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("guard-cancel", cx);
            assert!(view.read(cx).unsaved());
            assert!(!view.read(cx).runs.read(cx).running());
        })
        .unwrap();
    }
}
