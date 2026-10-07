use super::*;
use crate::desktop::deployment::{
    DeploymentProvider,
    tokens::{NamedToken, TokenCatalog},
};
use zeroize::Zeroizing;

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
pub(super) struct ChooseNamedToken {
    id: String,
}

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
pub(super) struct ChooseNamedTokenProvider {
    provider: DeploymentProvider,
}

enum TokenChange {
    Save(NamedToken, Zeroizing<String>),
    Remove(String),
}

pub(super) struct TokenPanel {
    catalog: TokenCatalog,
    path: Option<PathBuf>,
    selected: Option<String>,
    editing: bool,
    editing_id: Option<String>,
    removing: bool,
    saving: bool,
    status: String,
    provider: DeploymentProvider,
    name: Entity<InputState>,
    secret: Entity<InputState>,
    account: Entity<InputState>,
}

impl TokenPanel {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            catalog: TokenCatalog::default(),
            path: None,
            selected: None,
            editing: false,
            editing_id: None,
            removing: false,
            saving: false,
            status: String::new(),
            provider: DeploymentProvider::Netlify,
            name: cx.new(|cx| InputState::new(window, cx).placeholder("Customer account")),
            secret: Self::secret_input(false, window, cx),
            account: cx.new(|cx| InputState::new(window, cx).placeholder("Cloudflare Account ID")),
        }
    }

    fn secret_input(
        saved: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder(if saved {
                    "Saved securely (unchanged)"
                } else {
                    "Deployment token"
                })
        })
    }

    fn selected(&self) -> Option<&NamedToken> {
        self.selected
            .as_deref()
            .and_then(|id| self.catalog.tokens.iter().find(|token| token.id == id))
    }
}

impl Workbench {
    pub(super) fn open_token_manager(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.runs.read(cx).running() {
            return;
        }
        let Some(preferences) = self.preferences_path.clone() else {
            return;
        };
        self.task(
            "Loading deployment tokens...",
            move || {
                let path = TokenCatalog::path_in(&preferences)?;
                Ok((TokenCatalog::load(&path)?, path))
            },
            |this, (catalog, path), window, cx| {
                this.tokens.update(cx, |state, cx| {
                    state.selected = catalog.tokens.first().map(|token| token.id.clone());
                    state.catalog = catalog;
                    state.path = Some(path);
                    state.editing = false;
                    state.removing = false;
                    state.saving = false;
                    state.status.clear();
                    state.secret = TokenPanel::secret_input(false, window, cx);
                    cx.notify();
                });
                this.show_token_manager(window, cx);
            },
            window,
            cx,
        );
    }

    fn show_token_manager(&self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = self.tokens.clone();
        let view = cx.entity();
        let focus = self.focus.clone();
        window.open_dialog(cx, move |dialog, window, cx| {
            let state = panel.read(cx);
            let selected = state.selected();
            let tokens = state.catalog.tokens.clone();
            let locked = state.saving || state.editing || state.removing;
            let mut body = div().id("token-manager-body").flex().flex_col().gap_3()
                .child(div().flex().gap_2()
                    .child(Button::new("named-token-menu").w(px(260.)).max_w(px(260.)).min_w_0().overflow_hidden()
                        .accessibility_label(selected.map(|entry| entry.name.clone()).unwrap_or_else(|| "No saved tokens".into()))
                        .child(div().min_w_0().truncate().child(selected.map(|entry| entry.name.clone()).unwrap_or_else(|| "No saved tokens".into())))
                        .icon(gpui_kit::assets::IconName::ChevronDown).disabled(locked || tokens.is_empty())
                        .dropdown_menu({ let focus = focus.clone(); move |menu, _, _| {
                            let mut menu = menu.action_context(focus.clone());
                            for token in &tokens { menu = menu.menu(token.name.clone(), Box::new(ChooseNamedToken { id: token.id.clone() })); }
                            menu
                        }}))
                    .child(Button::new("named-token-add").ghost().icon(IconName::Plus).tooltip("Add token").accessibility_label("Add named token").disabled(locked)
                        .on_click(window.listener_for(&view, |this, _, window, cx| this.edit_named_token(false, window, cx))))
                    .child(Button::new("named-token-edit").ghost().icon(gpui_kit::assets::IconName::Pencil).tooltip("Edit token").accessibility_label("Edit named token").disabled(locked || selected.is_none())
                        .on_click(window.listener_for(&view, |this, _, window, cx| this.edit_named_token(true, window, cx))))
                    .child(Button::new("named-token-remove").ghost().icon(gpui_kit::assets::IconName::Trash).tooltip("Remove token").accessibility_label("Remove named token").disabled(locked || selected.is_none())
                        .on_click(window.listener_for(&view, |this, _, _, cx| this.tokens.update(cx, |state, cx| { state.removing = true; cx.notify(); })))));
            if state.removing {
                body = body.child("Remove this saved token? All targets using it will need another token. Deployed websites will not be deleted.")
                    .child(div().flex().justify_end().gap_2()
                        .child(Button::new("named-token-keep").label("Keep token").disabled(state.saving).on_click(window.listener_for(&view, |this, _, _, cx| this.tokens.update(cx, |state, cx| { state.removing = false; cx.notify(); }))))
                        .child(Button::new("named-token-delete").danger().label("Remove token").disabled(state.saving).on_click(window.listener_for(&view, |this, _, window, cx| {
                            if let Some(id) = this.tokens.read(cx).selected.clone() { this.persist_named_token(TokenChange::Remove(id), window, cx); }
                        }))));
            } else if state.editing {
                let mut fields = Form::new().columns(2).child(
                    FormField::new().label("Provider").col_span(2)
                        .child(Button::new("named-token-provider").label(state.provider.label()).accessibility_label("Token provider").icon(gpui_kit::assets::IconName::ChevronDown).disabled(state.saving || state.editing_id.is_some())
                        .dropdown_menu({ let focus = focus.clone(); move |menu, _, _| {
                            menu.action_context(focus.clone())
                                .menu("Netlify", Box::new(ChooseNamedTokenProvider { provider: DeploymentProvider::Netlify }))
                                .menu("Cloudflare Workers", Box::new(ChooseNamedTokenProvider { provider: DeploymentProvider::CloudflareWorkers }))
                        }})),
                );
                for (label, id, input) in [("Token name", "named-token-name", &state.name), ("Token", "named-token-secret", &state.secret)] {
                    fields = fields.child(FormField::new().label(label)
                        .child(Input::new(input).id(id).aria_label(label).disabled(state.saving)
                            .when(id == "named-token-secret", |input| input.content_type(gpui_kit::component::input::InputContentType::Password)))
                        .when(id == "named-token-secret", |field| field.col_span(2)));
                }
                if state.provider == DeploymentProvider::CloudflareWorkers {
                    fields = fields.child(FormField::new().label("Cloudflare Account ID").col_span(2)
                        .child(Input::new(&state.account).id("named-token-account").aria_label("Cloudflare Account ID").disabled(state.saving || state.editing_id.is_some())));
                }
                body = body.child(fields).child(div().flex().justify_end().gap_2()
                    .child(Button::new("named-token-cancel").label("Cancel edit").disabled(state.saving).on_click(window.listener_for(&view, |this, _, window, cx| this.cancel_named_token(window, cx))))
                    .child(Button::new("named-token-save").primary().icon(gpui_kit::assets::IconName::Save).label("Save token").disabled(state.saving).on_click(window.listener_for(&view, |this, _, window, cx| this.save_named_token(window, cx)))));
            } else if let Some(token) = selected {
                body = body.child(div().text_sm().child(token.provider.label()));
                if !token.account_id.is_empty() { body = body.child(div().text_sm().child(format!("Account: {}", token.account_id))); }
            }
            if !state.status.is_empty() { body = body.child(div().id("named-token-status").role(Role::Status).aria_label(state.status.clone()).text_color(cx.theme().danger).child(state.status.clone())); }
            dialog.title("Deployment tokens").overlay_closable(false).keyboard(!state.saving).close_button(!state.saving).child(body)
                .on_close(window.listener_for(&view, |this, _, window, cx| this.cancel_named_token(window, cx)))
                .footer(Button::new("named-token-close").label("Close").disabled(state.saving).on_click(window.listener_for(&view, |this, _, window, cx| { this.cancel_named_token(window, cx); window.close_dialog(cx); })))
        });
    }

    pub(super) fn choose_named_token(
        &mut self,
        action: &ChooseNamedToken,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tokens.update(cx, |state, cx| {
            if !state.saving
                && !state.editing
                && !state.removing
                && state.catalog.find(&action.id).is_ok()
            {
                state.selected = Some(action.id.clone());
                state.status.clear();
                cx.notify();
            }
        });
    }

    pub(super) fn choose_named_token_provider(
        &mut self,
        action: &ChooseNamedTokenProvider,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tokens.update(cx, |state, cx| {
            if !state.saving
                && state.editing
                && state.editing_id.is_none()
                && action.provider != DeploymentProvider::AzureSwa
            {
                state.provider = action.provider;
                state.secret = TokenPanel::secret_input(false, window, cx);
                state.status.clear();
                cx.notify();
            }
        });
    }

    fn edit_named_token(&mut self, existing: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.tokens.update(cx, |state, cx| {
            if state.saving {
                return;
            }
            let entry = existing.then(|| state.selected().cloned()).flatten();
            state.editing_id = entry.as_ref().map(|entry| entry.id.clone());
            state.provider = entry
                .as_ref()
                .map(|entry| entry.provider)
                .unwrap_or(DeploymentProvider::Netlify);
            state.editing = true;
            state.status.clear();
            state.name.update(cx, |input, cx| {
                input.set_value(
                    entry
                        .as_ref()
                        .map(|entry| entry.name.clone())
                        .unwrap_or_default(),
                    window,
                    cx,
                )
            });
            state.account.update(cx, |input, cx| {
                input.set_value(
                    entry
                        .as_ref()
                        .map(|entry| entry.account_id.clone())
                        .unwrap_or_default(),
                    window,
                    cx,
                )
            });
            state.secret = TokenPanel::secret_input(existing, window, cx);
            cx.notify();
        });
    }

    fn cancel_named_token(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tokens.update(cx, |state, cx| {
            if state.saving {
                return;
            }
            state.editing = false;
            state.removing = false;
            state.secret = TokenPanel::secret_input(false, window, cx);
            cx.notify();
        });
    }

    fn save_named_token(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.tokens.read(cx);
        if state.saving || !state.editing {
            return;
        }
        let entry = NamedToken {
            id: state
                .editing_id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            name: state.name.read(cx).value().trim().into(),
            provider: state.provider,
            account_id: if state.provider == DeploymentProvider::CloudflareWorkers {
                state.account.read(cx).value().trim().to_ascii_lowercase()
            } else {
                String::new()
            },
        };
        if let Err(error) = entry.validate() {
            self.tokens.update(cx, |state, cx| {
                state.status = format!("{error:#}");
                cx.notify();
            });
            return;
        }
        let secret = Zeroizing::new(state.secret.read(cx).value().trim().to_string());
        self.persist_named_token(TokenChange::Save(entry, secret), window, cx);
    }

    fn persist_named_token(
        &mut self,
        change: TokenChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        let state = self.tokens.read(cx);
        let Some(path) = state.path.clone() else {
            return;
        };
        let mut catalog = state.catalog.clone();
        self.busy = true;
        self.tokens.update(cx, |state, cx| {
            state.saving = true;
            state.secret = TokenPanel::secret_input(false, window, cx);
            cx.notify();
        });
        self.sync_modal(cx);
        let task = cx.background_executor().spawn(async move {
            let selected = match change {
                TokenChange::Save(entry, secret) => {
                    let id = entry.id.clone();
                    catalog.save_token(&path, entry, &secret)?;
                    Some(id)
                }
                TokenChange::Remove(id) => {
                    catalog.remove(&path, &id)?;
                    catalog.tokens.first().map(|entry| entry.id.clone())
                }
            };
            Ok::<_, anyhow::Error>((catalog, selected))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |this, _, cx| {
                this.busy = false;
                this.tokens.update(cx, |state, cx| {
                    state.saving = false;
                    match result {
                        Ok((catalog, selected)) => {
                            state.catalog = catalog;
                            state.selected = selected;
                            state.editing = false;
                            state.removing = false;
                            state.status.clear();
                        }
                        Err(error) => state.status = format!("{error:#}"),
                    }
                    cx.notify();
                });
                this.sync_modal(cx);
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(all(test, feature = "gui-tests"))]
mod tests {
    use crate::desktop::deployment::tokens::TokenCatalog;
    use gpui_kit::test::{TestAppContextExt, TestWindowExt};
    use gpui_kit::{AppContext, TestAppContext};
    use std::time::Duration;

    #[gpui_kit::test]
    async fn named_token_manager_masks_saves_renames_and_confirms_removal(cx: &mut TestAppContext) {
        let root = tempfile::tempdir().unwrap();
        let (handle, view) = super::super::tests::editor(cx, root.path());
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| this.open_token_manager(window, cx))
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
            window.try_find("named-token-add").is_some()
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("named-token-add", cx);
            window.render_frame(cx);
            window.click("named-token-name", cx);
            window.input("Shared account", cx);
            window.click("named-token-secret", cx);
            window.input("synthetic-ui-secret", cx);
            window.render_frame(cx);
            assert!(window.find("named-token-secret").value().is_none());
            window.click("named-token-save", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            !view.read(cx).tokens.read(cx).saving
                && view.read(cx).tokens.read(cx).catalog.tokens.len() == 1
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            let panel = view.read(cx).tokens.read(cx);
            let path = panel.path.as_ref().unwrap();
            assert!(
                !std::fs::read_to_string(path)
                    .unwrap()
                    .contains("synthetic-ui-secret")
            );
            assert_eq!(
                TokenCatalog::load(path).unwrap().tokens[0]
                    .token()
                    .unwrap()
                    .as_str(),
                "synthetic-ui-secret"
            );
            window.click("named-token-edit", cx);
            window.render_frame(cx);
            window.click("named-token-name", cx);
            window.press("ctrl-a", cx);
            window.input("Renamed account", cx);
            window.click("named-token-save", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            !view.read(cx).tokens.read(cx).saving
                && view.read(cx).tokens.read(cx).catalog.tokens[0].name == "Renamed account"
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            assert_eq!(
                view.read(cx).tokens.read(cx).catalog.tokens[0]
                    .token()
                    .unwrap()
                    .as_str(),
                "synthetic-ui-secret"
            );
            window.click("named-token-add", cx);
            window.render_frame(cx);
            assert!(window.find("named-token-provider").visible());
            window.click("named-token-cancel", cx);
            window.render_frame(cx);
            window.click("named-token-remove", cx);
            window.render_frame(cx);
            window.click("named-token-keep", cx);
            window.render_frame(cx);
            assert_eq!(view.read(cx).tokens.read(cx).catalog.tokens.len(), 1);
            window.click("named-token-remove", cx);
            window.render_frame(cx);
            window.click("named-token-delete", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            !view.read(cx).tokens.read(cx).saving
                && view.read(cx).tokens.read(cx).catalog.tokens.is_empty()
        })
        .await;
    }
}
