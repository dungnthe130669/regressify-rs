use super::*;
use crate::desktop::landing::LandingFile;
use crate::desktop::{
    deployment::{
        DeployTarget, DeployTargets, DeploymentJob, DeploymentProvider,
        cloudflare::CloudflareTarget, tokens::TokenCatalog,
    },
    runner::RunCommand,
};
use zeroize::Zeroizing;

pub(super) const PROVIDER_CHOICES: [(DeploymentProvider, &str); 3] = [
    (DeploymentProvider::AzureSwa, "azureSwa"),
    (DeploymentProvider::Netlify, "netlify"),
    (DeploymentProvider::CloudflareWorkers, "cloudflareWorkers"),
];

enum TargetChange {
    Save(Box<DeployTarget>, Zeroizing<String>),
    Remove(String),
    Select,
}

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
pub(super) struct ChooseDeployTarget {
    id: String,
}

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
pub(super) struct ChooseDeployProvider {
    provider: DeploymentProvider,
}

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
pub(super) struct ChooseDeployToken {
    id: Option<String>,
}

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
pub(super) struct ChooseCloudflareSetting {
    setting: String,
    value: String,
}

pub(super) struct DeploymentPanel {
    settings: DeployTargets,
    tokens: TokenCatalog,
    tokens_path: Option<PathBuf>,
    named_token_id: Option<String>,
    path: Option<PathBuf>,
    root: Option<PathBuf>,
    intent: Option<runs::RunIntent>,
    landing: Option<LandingFile>,
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
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
    provider: DeploymentProvider,
    site_id: Entity<InputState>,
    account_id: Entity<InputState>,
    worker_name: Entity<InputState>,
    cloudflare: CloudflareTarget,
    cloudflare_options: bool,
    compatibility_date: Entity<InputState>,
    excludes: Entity<InputState>,
    target_picker: choices::ValueSelector,
    provider_picker: choices::ValueSelector,
    token_picker: choices::ValueSelector,
    routing_picker: choices::ValueSelector,
    html_picker: choices::ValueSelector,
    not_found_picker: choices::ValueSelector,
    selector_subscriptions: Vec<Subscription>,
}

impl DeploymentPanel {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            settings: DeployTargets::default(),
            tokens: TokenCatalog::default(),
            tokens_path: None,
            named_token_id: None,
            path: None,
            root: None,
            intent: None,
            landing: None,
            cancel: None,
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
            token: Self::token_input(false, window, cx),
            environment: cx
                .new(|cx| InputState::new(window, cx).placeholder("Production (default)")),
            provider: DeploymentProvider::AzureSwa,
            site_id: cx
                .new(|cx| InputState::new(window, cx).placeholder("Project ID from Netlify")),
            account_id: cx
                .new(|cx| InputState::new(window, cx).placeholder("Cloudflare Account ID")),
            worker_name: cx.new(|cx| InputState::new(window, cx).placeholder("reports")),
            cloudflare: CloudflareTarget::default(),
            cloudflare_options: false,
            compatibility_date: cx.new(|cx| InputState::new(window, cx).placeholder("YYYY-MM-DD")),
            excludes: cx
                .new(|cx| InputState::new(window, cx).placeholder("[\"*.map\", \"private/**\"]")),
            target_picker: choices::ValueSelector::new(window, cx),
            provider_picker: choices::ValueSelector::new(window, cx),
            token_picker: choices::ValueSelector::new(window, cx),
            routing_picker: choices::ValueSelector::new(window, cx),
            html_picker: choices::ValueSelector::new(window, cx),
            not_found_picker: choices::ValueSelector::new(window, cx),
            selector_subscriptions: Vec::new(),
        }
    }

    fn token_input(saved: bool, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
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

    fn selected(&self) -> Option<&DeployTarget> {
        self.settings
            .targets
            .iter()
            .find(|target| Some(&target.id) == self.settings.selected.as_ref())
    }

    fn sync_selectors(&mut self, window: &mut Window, cx: &mut App) {
        let targets = self
            .settings
            .targets
            .iter()
            .map(|target| choices::ValueChoice::new(target.id.clone(), target.name.clone()))
            .collect();
        self.target_picker
            .sync(targets, self.settings.selected.clone(), window, cx);
        let providers = PROVIDER_CHOICES
            .into_iter()
            .map(|(provider, value)| choices::ValueChoice::new(value, provider.label()))
            .collect();
        let selected_provider = PROVIDER_CHOICES
            .iter()
            .find(|(provider, _)| provider == &self.provider)
            .map(|(_, value)| (*value).into());
        self.provider_picker
            .sync(providers, selected_provider, window, cx);
        let mut tokens = vec![choices::ValueChoice::new("", "Target-specific token")];
        tokens.extend(
            self.tokens
                .tokens
                .iter()
                .filter(|entry| entry.provider == self.provider)
                .map(|entry| choices::ValueChoice::new(entry.id.clone(), entry.name.clone())),
        );
        self.token_picker.sync(
            tokens,
            Some(self.named_token_id.clone().unwrap_or_default()),
            window,
            cx,
        );
        let routing = match self.cloudflare.workers_dev {
            Some(true) => "public",
            Some(false) => "private",
            None => "preserve",
        };
        self.routing_picker.sync(
            [
                ("public", "Public (production)"),
                ("private", "Private"),
                ("preserve", "Preserve existing routing"),
            ]
            .into_iter()
            .map(|(value, label)| choices::ValueChoice::new(value, label))
            .collect(),
            Some(routing.into()),
            window,
            cx,
        );
        self.html_picker.sync(
            [
                "auto-trailing-slash",
                "force-trailing-slash",
                "drop-trailing-slash",
                "none",
            ]
            .into_iter()
            .map(|value| choices::ValueChoice::new(value, value))
            .collect(),
            Some(self.cloudflare.html_handling.clone()),
            window,
            cx,
        );
        self.not_found_picker.sync(
            ["none", "404-page", "single-page-application"]
                .into_iter()
                .map(|value| choices::ValueChoice::new(value, value))
                .collect(),
            Some(self.cloudflare.not_found_handling.clone()),
            window,
            cx,
        );
    }

    fn can_deploy(&self) -> bool {
        (self.intent.is_some() || self.landing.is_some())
            && self.selected().is_some_and(DeployTarget::has_token)
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
                    .icon(gpui_kit::assets::IconName::CloudUpload)
                    .accessibility_label("Deploy")
                    .tooltip("Deploy selected suite reports")
                    .disabled(disabled || self.runs.read(cx).selected_count() == 0)
                    .on_click(cx.listener(|this, _, window, cx| this.open_deployment(window, cx))),
            )
            .child(
                Button::new("deploy-targets")
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
                let tokens_path = TokenCatalog::path_in(&preferences)?;
                Ok((
                    DeployTargets::load(&path)?,
                    path,
                    TokenCatalog::load(&tokens_path)?,
                    tokens_path,
                ))
            },
            move |this, (mut settings, path, tokens, tokens_path), window, cx| {
                if settings.selected.is_none() {
                    settings.selected = settings.targets.first().map(|target| target.id.clone());
                }
                this.runs.update(cx, |panel, cx| {
                    panel.set_deployment_storage_path(path.clone(), cx);
                });
                this.deployment.update(cx, |state, cx| {
                    state.settings = settings;
                    state.tokens = tokens;
                    state.tokens_path = Some(tokens_path);
                    state.named_token_id = None;
                    state.path = Some(path);
                    state.root = Some(root);
                    state.intent = intent;
                    state.landing = None;
                    state.partial = suite_ids.len() != catalog_ids.len();
                    state.catalog_ids = catalog_ids;
                    state.suite_ids = suite_ids;
                    state.confirmed = false;
                    state.editing = false;
                    state.removing = false;
                    state.saving = false;
                    state.status.clear();
                    state.token = DeploymentPanel::token_input(false, window, cx);
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
        let picker = panel.read(cx).target_picker.state.clone();
        let subscription = cx.subscribe_in(
            &picker,
            window,
            |this, _, event: &SelectEvent<choices::ValueChoices>, window, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    this.choose_deploy_target(&ChooseDeployTarget { id: id.clone() }, window, cx);
                }
                this.deployment
                    .update(cx, |state, cx| state.sync_selectors(window, cx));
            },
        );
        let mut subscriptions = vec![subscription];
        for (kind, picker) in [
            ("provider", panel.read(cx).provider_picker.state.clone()),
            ("token", panel.read(cx).token_picker.state.clone()),
            ("routing", panel.read(cx).routing_picker.state.clone()),
            ("html", panel.read(cx).html_picker.state.clone()),
            ("notFound", panel.read(cx).not_found_picker.state.clone()),
        ] {
            subscriptions.push(cx.subscribe_in(
                &picker,
                window,
                move |this, _, event: &SelectEvent<choices::ValueChoices>, window, cx| {
                    if let SelectEvent::Confirm(Some(value)) = event {
                        match kind {
                            "provider" => {
                                if let Some((provider, _)) =
                                    PROVIDER_CHOICES.into_iter().find(|(_, key)| key == value)
                                {
                                    this.choose_deploy_provider(
                                        &ChooseDeployProvider { provider },
                                        window,
                                        cx,
                                    );
                                }
                            }
                            "token" => this.choose_deploy_token(
                                &ChooseDeployToken {
                                    id: (!value.is_empty()).then(|| value.clone()),
                                },
                                window,
                                cx,
                            ),
                            _ => this.choose_cloudflare_setting(
                                &ChooseCloudflareSetting {
                                    setting: kind.into(),
                                    value: value.clone(),
                                },
                                window,
                                cx,
                            ),
                        }
                    }
                    this.deployment
                        .update(cx, |state, cx| state.sync_selectors(window, cx));
                },
            ));
        }
        panel.update(cx, |state, _| state.selector_subscriptions = subscriptions);
        window.open_dialog(cx, move |dialog, window, cx| {
            panel.update(cx, |state, cx| state.sync_selectors(window, cx));
            let state = panel.read(cx);
            let targets = state.settings.targets.clone();
            let selected = state.selected();
            let target_label = selected.map(|target| target.name.clone()).or_else(|| state.settings.selected.clone()).unwrap_or_else(|| "No deployment targets".into());
            let missing_target = state.settings.selected.is_some() && selected.is_none();
            let editing = state.editing;
            let saving = state.saving;
            let mut body = div().flex().flex_col().gap_3().min_w_0()
                .when(!editing && selected.is_some(), |body| body.child(div().text_sm().text_color(cx.theme().muted_foreground)
                    .child(selected.map(|target| target.provider.label()).unwrap_or_default())))
                .child(div().flex().items_center().gap_2()
                    .child(gpui_kit::component::select::Select::new(&state.target_picker.state).id("deploy-target-menu").flex_1().min_w_0()
                        .accessibility_label(target_label.clone()).placeholder(target_label).search_placeholder("Search targets")
                        .when(missing_target, |select| select.border_color(cx.theme().danger))
                        .disabled(editing || saving || state.removing || targets.is_empty()))
                    .child(Button::new("deploy-target-add").ghost().icon(IconName::Plus).tooltip("Add target").accessibility_label("Add target").disabled(saving || editing || state.removing)
                        .on_click(window.listener_for(&view, |this, _, window, cx| this.edit_deploy_target(false, window, cx))))
                    .child(Button::new("deploy-target-edit").ghost().icon(gpui_kit::assets::IconName::Pencil).tooltip("Edit target").accessibility_label("Edit target").disabled(selected.is_none() || saving || editing || state.removing)
                        .on_click(window.listener_for(&view, |this, _, window, cx| this.edit_deploy_target(true, window, cx))))
                    .child(Button::new("deploy-target-remove").ghost().icon(gpui_kit::assets::IconName::Trash).tooltip("Remove target").accessibility_label("Remove target").disabled(selected.is_none() || saving || editing || state.removing)
                        .on_click(window.listener_for(&view, |this, _, _, cx| this.deployment.update(cx, |state, cx| { state.removing = true; cx.notify(); })))));
            if missing_target {
                body = body.child(div().id("deploy-target-unavailable").role(Role::Status).aria_label("Selected deployment target is unavailable")
                    .test_support().text_color(cx.theme().danger).child("Selected deployment target is unavailable"));
            }
            if state.removing {
                body = body.child(format!("Remove target '{}'? The hosted site will not be deleted.", selected.map(|target| target.name.as_str()).unwrap_or_default()))
                    .child(div().flex().justify_end().gap_2()
                        .child(Button::new("deploy-remove-cancel").label("Keep target").disabled(saving)
                            .on_click(window.listener_for(&view, |this, _, _, cx| this.deployment.update(cx, |state, cx| { state.removing = false; cx.notify(); }))))
                        .child(Button::new("deploy-remove-confirm").danger().label("Remove target").accessibility_label("Confirm remove target").disabled(saving)
                            .on_click(window.listener_for(&view, |this, _, window, cx| this.remove_deploy_target(window, cx)))));
            } else if editing {
                let mut target_fields = Form::new().columns(2);
                target_fields = target_fields.child(FormField::new().label("Provider")
                    .child(gpui_kit::component::select::Select::new(&state.provider_picker.state).id("deploy-provider")
                        .accessibility_label("Deployment provider").disabled(saving)));
                if state.provider == DeploymentProvider::CloudflareWorkers {
                    target_fields = target_fields.child(FormField::new().label("workers.dev").col_span(2)
                        .child(div().flex().items_center().gap_2().min_w_0()
                        .child(gpui_kit::component::select::Select::new(&state.routing_picker.state).id("deploy-cloudflare-routing").flex_1().min_w_0()
                            .accessibility_label("Cloudflare public routing").disabled(saving))
                        .child(Button::new("deploy-cloudflare-options").ghost().icon(gpui_kit::assets::IconName::Settings2)
                            .tooltip("Cloudflare asset settings").accessibility_label("Cloudflare asset settings").disabled(saving)
                            .on_click(window.listener_for(&view, |this, _, _, cx| {
                                this.deployment.update(cx, |state, cx| { state.cloudflare_options = !state.cloudflare_options; cx.notify(); });
                            })))));
                    if state.cloudflare_options {
                        for (label, id, picker, value) in [
                            ("HTML handling", "deploy-cloudflare-html", &state.html_picker.state, &state.cloudflare.html_handling),
                            ("Not-found handling", "deploy-cloudflare-not-found", &state.not_found_picker.state, &state.cloudflare.not_found_handling),
                        ] {
                            target_fields = target_fields.child(FormField::new().label(label)
                                .child(gpui_kit::component::select::Select::new(picker).id(id).flex_1().min_w_0()
                                    .accessibility_label(label).placeholder(value.clone()).disabled(saving)
                                    .when(picker.read(cx).selected_value().is_none(), |select| select.border_color(cx.theme().danger))));
                        }
                        for (label, id, input) in [("Compatibility date", "deploy-cloudflare-date", &state.compatibility_date), ("Excluded paths (JSON array)", "deploy-cloudflare-excludes", &state.excludes)] {
                            target_fields = target_fields.child(FormField::new().label(label)
                                .child(Input::new(input).id(id).aria_label(label).disabled(saving))
                                .when(id == "deploy-cloudflare-excludes", |field| field.col_span(2)));
                        }
                    }
                }
                let token_label = match state.provider {
                    DeploymentProvider::AzureSwa => "Deployment token",
                    DeploymentProvider::Netlify => "Personal access token",
                    DeploymentProvider::CloudflareWorkers => "API token",
                };
                if state.provider != DeploymentProvider::AzureSwa {
                    let label = state.named_token_id.as_deref().map(|id| state.tokens.find(id).map(|entry| entry.name.clone()).unwrap_or_else(|_| "Missing named token".into())).unwrap_or_else(|| "Target-specific token".into());
                    target_fields = target_fields.child(FormField::new().label("Token source")
                        .child(gpui_kit::component::select::Select::new(&state.token_picker.state).id("deploy-token-source")
                            .accessibility_label(label.clone()).placeholder(label).search_placeholder("Search tokens").disabled(saving)
                            .when(state.token_picker.state.read(cx).selected_value().is_none(), |select| select.border_color(cx.theme().danger))));
                }
                let mut fields = vec![("Target name", "deploy-name", &state.name)];
                if state.named_token_id.is_none() { fields.push((token_label, "deploy-token", &state.token)); }
                match state.provider {
                    DeploymentProvider::AzureSwa => fields.push(("Azure environment", "deploy-environment", &state.environment)),
                    DeploymentProvider::Netlify => fields.push(("Netlify Site ID", "deploy-site-id", &state.site_id)),
                    DeploymentProvider::CloudflareWorkers => {
                        if state.named_token_id.is_none() { fields.push(("Cloudflare Account ID", "deploy-account-id", &state.account_id)); }
                        fields.push(("Worker name", "deploy-worker-name", &state.worker_name));
                    }
                }
                for (label, id, input) in fields {
                    target_fields = target_fields.child(FormField::new().label(label)
                        .child(Input::new(input).id(id).aria_label(label).disabled(saving)
                            .when(id == "deploy-token", |input| input.content_type(gpui_kit::component::input::InputContentType::Password))));
                }
                body = body.child(target_fields);
                body = body.child(div().flex().justify_end().gap_2()
                    .child(Button::new("deploy-target-cancel").label("Cancel edit").disabled(saving).on_click(window.listener_for(&view, |this, _, window, cx| this.cancel_deploy_edit(window, cx))))
                    .child(Button::new("deploy-target-save").primary().icon(gpui_kit::assets::IconName::Save).label("Save target").disabled(saving)
                        .on_click(window.listener_for(&view, |this, _, window, cx| this.save_deploy_target(window, cx)))));
            } else {
                body = body.child(div().flex().items_center().gap_2()
                    .child(div().flex_1().min_w_0().text_sm().child(selected.map(|target| format!("{} | {}", match target.provider {
                    DeploymentProvider::AzureSwa => if target.environment.is_empty() { "Production".into() } else { target.environment.clone() },
                    DeploymentProvider::Netlify => format!("Production | {}", target.site_id),
                    DeploymentProvider::CloudflareWorkers => target.cloudflare.as_ref().map(|settings| format!("Worker | {}", settings.worker_name)).unwrap_or_default(),
                }, if target.has_token() { "Token saved securely" } else { "Deployment token required" })).unwrap_or_default()))
                    .when(selected.is_some_and(|target| !target.has_token()), |row| row.child(
                        Button::new("deploy-add-token").icon(gpui_kit::assets::IconName::Pencil).label("Add token")
                            .accessibility_label("Add deployment token").disabled(saving)
                            .on_click(window.listener_for(&view, |this, _, window, cx| this.edit_deploy_target(true, window, cx))))));
                if let Some(url) = selected.and_then(|target| target.published_url.clone()) {
                    let open_url = url.clone();
                    body = body.child(div().id("deploy-published-url").flex().items_center().gap_2().min_w_0()
                        .child(Button::new("deploy-open-published").link().flex_initial().min_w_0().overflow_hidden()
                            .label(url.clone()).tooltip(url.clone()).accessibility_label("Open deployed site")
                            .on_click(window.listener_for(&view, move |this, _, _, cx| {
                                if let Err(error) = open::that_detached(&open_url) { this.deployment.update(cx, |state, cx| { state.status = format!("Cannot open deployed site: {error}"); cx.notify(); }); }
                            })))
                        .child(design::CopyText::new("deploy-copy-published", url, "Copy deployed URL")));
                }
                if let Some(landing) = &state.landing {
                    body = body.child(div().font_weight(FontWeight::SEMIBOLD).child(landing.app.heading.clone()))
                        .child(format!("Sites: {}", landing.app.sites.len()));
                } else {
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
            }
            if !state.status.is_empty() { body = body.child(div().id("deploy-status").role(Role::Status).aria_label(state.status.clone()).text_color(cx.theme().danger).child(state.status.clone())); }
            let starting = saving && !editing && !state.removing;
            dialog.title(if state.landing.is_some() { "Deploy landing page" } else { "Deploy reports" }).overlay_closable(!saving && state.landing.is_none()).keyboard(!saving && state.landing.is_none()).close_button(!saving && state.landing.is_none()).child(div().id("deploy-dialog-body").role(Role::Group).aria_label("Deployment options").child(if state.cloudflare_options && editing && state.provider == DeploymentProvider::CloudflareWorkers { div().max_h(px(390.)).overflow_y_scrollbar().child(body).into_any_element() } else { body.into_any_element() }))
                .on_close(window.listener_for(&view, |this, _, window, cx| this.cancel_deploy_edit(window, cx)))
                .footer(gpui_kit::component::dialog::DialogFooter::new()
                    .child(Button::new("deploy-close").label("Close").disabled(saving).on_click(window.listener_for(&view, |this, _, window, cx| {
                        this.cancel_deploy_edit(window, cx);
                        window.close_dialog(cx);
                        if this.deployment.read(cx).landing.is_some() { this.show_landing_manager(window, cx); }
                    })))
                    .when(state.cancel.is_some(), |footer| footer.child(Button::new("landing-cancel-deployment").danger().label("Cancel deployment").on_click(window.listener_for(&view, |this, _, _, cx| {
                        this.deployment.update(cx, |state, cx| { if let Some(cancel) = state.cancel.take() { let _ = cancel.send(()); state.status = "Cancelling deployment...".into(); cx.notify(); } });
                    }))))
                    .child(Button::new("deploy-confirm").primary().min_w(px(120.)).icon(gpui_kit::assets::IconName::CloudUpload).loading(starting)
                        .label(if starting { if state.landing.is_some() { "Deploying..." } else { "Starting..." } } else { "Deploy" })
                        .accessibility_label(if starting { "Starting deployment" } else if state.landing.is_some() { "Deploy landing page" } else { "Deploy selected reports" })
                        .when(selected.is_some_and(|target| !target.has_token()), |button| button.tooltip("Deployment token required"))
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

    pub(super) fn choose_deploy_provider(
        &mut self,
        action: &ChooseDeployProvider,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.deployment.update(cx, |state, cx| {
            if state.saving || !state.editing || state.provider == action.provider {
                return;
            }
            state.provider = action.provider;
            state.named_token_id = None;
            let saved = state
                .settings
                .targets
                .iter()
                .find(|target| Some(&target.id) == state.editing_id.as_ref())
                .is_some_and(|target| {
                    target.provider == state.provider && target.credential_id.is_some()
                });
            state.token = DeploymentPanel::token_input(saved, window, cx);
            state.status.clear();
            cx.notify();
        });
    }

    pub(super) fn choose_cloudflare_setting(
        &mut self,
        action: &ChooseCloudflareSetting,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.deployment.update(cx, |state, cx| {
            if state.saving
                || !state.editing
                || state.provider != DeploymentProvider::CloudflareWorkers
            {
                return;
            }
            match (action.setting.as_str(), action.value.as_str()) {
                ("routing", "public") => state.cloudflare.workers_dev = Some(true),
                ("routing", "private") => state.cloudflare.workers_dev = Some(false),
                ("routing", "preserve") => state.cloudflare.workers_dev = None,
                (
                    "html",
                    "auto-trailing-slash" | "force-trailing-slash" | "drop-trailing-slash" | "none",
                ) => state.cloudflare.html_handling = action.value.clone(),
                ("notFound", "none" | "404-page" | "single-page-application") => {
                    state.cloudflare.not_found_handling = action.value.clone()
                }
                _ => return,
            }
            state.status.clear();
            cx.notify();
        });
    }

    fn cancel_deploy_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.deployment.update(cx, |state, cx| {
            state.editing = false;
            state.status.clear();
            state.token = DeploymentPanel::token_input(false, window, cx);
            cx.notify();
        });
    }

    pub(super) fn choose_deploy_token(
        &mut self,
        action: &ChooseDeployToken,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.deployment.update(cx, |state, cx| {
            if state.saving
                || !state.editing
                || state.provider == DeploymentProvider::AzureSwa
                || state.named_token_id == action.id
            {
                return;
            }
            if let Some(id) = &action.id {
                let Ok(entry) = state.tokens.find(id) else {
                    return;
                };
                if entry.provider != state.provider {
                    return;
                }
                let account = entry.account_id.clone();
                state
                    .account_id
                    .update(cx, |input, cx| input.set_value(account, window, cx));
            }
            state.named_token_id = action.id.clone();
            let saved = state
                .settings
                .targets
                .iter()
                .find(|entry| Some(&entry.id) == state.editing_id.as_ref())
                .is_some_and(|entry| {
                    entry.provider == state.provider && entry.credential_id.is_some()
                });
            state.token = DeploymentPanel::token_input(saved, window, cx);
            state.status.clear();
            cx.notify();
        });
    }

    fn edit_deploy_target(&mut self, existing: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.deployment.update(cx, |state, cx| {
            if state.saving {
                return;
            }
            let target = existing.then(|| state.selected().cloned()).flatten();
            state.cloudflare = target
                .as_ref()
                .and_then(|target| target.cloudflare.clone())
                .unwrap_or_default();
            state.cloudflare_options = false;
            state.compatibility_date.update(cx, |input, cx| {
                input.set_value(state.cloudflare.compatibility_date.clone(), window, cx)
            });
            state.excludes.update(cx, |input, cx| {
                input.set_value(
                    serde_json::to_string(&state.cloudflare.exclude)
                        .unwrap_or_else(|_| "[]".into()),
                    window,
                    cx,
                )
            });
            state.editing_id = target.as_ref().map(|target| target.id.clone());
            state.named_token_id = target
                .as_ref()
                .and_then(|target| target.named_token_id.clone());
            state.provider = target
                .as_ref()
                .map(|target| target.provider)
                .unwrap_or_default();
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
            state.token = DeploymentPanel::token_input(
                target
                    .as_ref()
                    .is_some_and(|target| target.credential_id.is_some()),
                window,
                cx,
            );
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
            state.site_id.update(cx, |input, cx| {
                input.set_value(
                    target
                        .as_ref()
                        .map(|target| target.site_id.clone())
                        .unwrap_or_default(),
                    window,
                    cx,
                );
            });
            state.account_id.update(cx, |input, cx| {
                input.set_value(
                    target
                        .as_ref()
                        .and_then(|target| target.cloudflare.as_ref())
                        .map(|settings| settings.account_id.clone())
                        .unwrap_or_default(),
                    window,
                    cx,
                );
            });
            state.worker_name.update(cx, |input, cx| {
                input.set_value(
                    target
                        .as_ref()
                        .and_then(|target| target.cloudflare.as_ref())
                        .map(|settings| settings.worker_name.clone())
                        .unwrap_or_default(),
                    window,
                    cx,
                );
            });
            cx.notify();
        });
    }

    fn save_deploy_target(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.deployment.read(cx);
        if state.saving || !state.editing {
            return;
        }
        let mut cloudflare = state.cloudflare.clone();
        if state.provider == DeploymentProvider::CloudflareWorkers {
            cloudflare.compatibility_date = state.compatibility_date.read(cx).value().trim().into();
            match serde_json::from_str::<Vec<String>>(&state.excludes.read(cx).value()) {
                Ok(exclude) => cloudflare.exclude = exclude,
                Err(_) => {
                    self.deployment.update(cx, |state, cx| {
                        state.status =
                            "Cloudflare excluded paths must be a JSON array of glob strings".into();
                        cx.notify();
                    });
                    return;
                }
            }
        }
        let target = DeployTarget {
            id: state
                .editing_id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            name: state.name.read(cx).value().trim().into(),
            published_url: None,
            credential_id: None,
            named_token_id: state.named_token_id.clone(),
            environment: if state.provider == DeploymentProvider::AzureSwa {
                state.environment.read(cx).value().trim().into()
            } else {
                String::new()
            },
            provider: state.provider,
            site_id: if state.provider == DeploymentProvider::Netlify {
                state.site_id.read(cx).value().trim().into()
            } else {
                String::new()
            },
            cloudflare: (state.provider == DeploymentProvider::CloudflareWorkers).then(|| {
                CloudflareTarget {
                    account_id: state
                        .account_id
                        .read(cx)
                        .value()
                        .trim()
                        .to_ascii_lowercase(),
                    worker_name: state
                        .worker_name
                        .read(cx)
                        .value()
                        .trim()
                        .to_ascii_lowercase(),
                    ..cloudflare
                }
            }),
        };
        if let Err(error) = target.validate() {
            self.deployment.update(cx, |state, cx| {
                state.status = format!("{error:#}");
                cx.notify();
            });
            return;
        }
        let token = Zeroizing::new(state.token.read(cx).value().trim().to_owned());
        self.persist_deploy_targets(
            TargetChange::Save(Box::new(target), token),
            None,
            window,
            cx,
        );
    }

    fn remove_deploy_target(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.deployment.read(cx);
        if state.saving || !state.removing {
            return;
        }
        let Some(target) = state.selected().cloned() else {
            return;
        };
        self.persist_deploy_targets(TargetChange::Remove(target.id), None, window, cx);
    }

    fn persist_deploy_targets(
        &mut self,
        change: TargetChange,
        pending: Option<runs::RunIntent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = self.deployment.read(cx);
        let (Some(path), Some(root)) = (state.path.clone(), state.root.clone()) else {
            return;
        };
        let mut settings = state.settings.clone();
        let tokens_path = state.tokens_path.clone();
        let deployment_path = path.clone();
        self.busy = true;
        self.sync_modal(cx);
        self.deployment.update(cx, |state, cx| {
            state.saving = true;
            state.token = DeploymentPanel::token_input(false, window, cx);
            cx.notify();
        });
        let task = cx.background_executor().spawn(async move {
            match change {
                TargetChange::Save(target, token) => {
                    if target.named_token_id.is_some() {
                        let tokens_path = tokens_path
                            .ok_or_else(|| anyhow!("Named token settings unavailable"))?;
                        settings.save_linked_target(
                            &root,
                            &path,
                            *target,
                            &TokenCatalog::load(&tokens_path)?,
                        )?;
                    } else {
                        settings.save_target(&root, &path, *target, &token)?;
                    }
                }
                TargetChange::Remove(id) => settings.remove_target(&root, &path, &id)?,
                TargetChange::Select => settings.save(&path)?,
            }
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
                if succeeded && this.deployment.read(cx).landing.is_none() {
                    this.runs.update(cx, |panel, cx| {
                        panel.set_deployment_storage_path(deployment_path, cx);
                    });
                }
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
        if self.deployment.read(cx).landing.is_some() {
            self.begin_landing_deployment(window, cx);
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
                self.persist_deploy_targets(TargetChange::Select, Some(intent), window, cx);
            }
            Err(error) => self.deployment.update(cx, |state, cx| {
                state.status = format!("{error:#}");
                cx.notify();
            }),
        }
    }

    pub(super) fn open_landing_deployment(
        &mut self,
        file: LandingFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.runs.read(cx).running() {
            return;
        }
        let Some(preferences) = self.preferences_path.clone() else {
            return;
        };
        self.task(
            "Loading landing deployment targets...",
            move || {
                file.ensure_unchanged()?;
                let root = file.app.deployment_root(&preferences)?;
                let path = DeployTargets::path_in(&root, &preferences)?;
                let tokens_path = TokenCatalog::path_in(&preferences)?;
                Ok((
                    file,
                    root,
                    DeployTargets::load(&path)?,
                    path,
                    TokenCatalog::load(&tokens_path)?,
                    tokens_path,
                ))
            },
            |this, (file, root, mut settings, path, tokens, tokens_path), window, cx| {
                if settings.selected.is_none() {
                    settings.selected = settings.targets.first().map(|entry| entry.id.clone());
                }
                this.deployment.update(cx, |state, cx| {
                    state.settings = settings;
                    state.root = Some(root);
                    state.path = Some(path);
                    state.tokens = tokens;
                    state.tokens_path = Some(tokens_path);
                    state.named_token_id = None;
                    state.intent = None;
                    state.landing = Some(file);
                    state.catalog_ids.clear();
                    state.suite_ids.clear();
                    state.partial = false;
                    state.confirmed = false;
                    state.editing = false;
                    state.removing = false;
                    state.saving = false;
                    state.status.clear();
                    state.token = DeploymentPanel::token_input(false, window, cx);
                    cx.notify();
                });
                window.close_dialog(cx);
                this.show_deployment_dialog(window, cx);
            },
            window,
            cx,
        );
    }

    fn begin_landing_deployment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.deployment.read(cx);
        if !state.can_deploy() {
            return;
        }
        let (Some(file), Some(target), Some(root), Some(path), Some(tokens_path)) = (
            state.landing.clone(),
            state.selected().cloned(),
            state.root.clone(),
            state.path.clone(),
            state.tokens_path.clone(),
        ) else {
            return;
        };
        let settings = state.settings.clone();
        let target_id = target.id.clone();
        let (cancel, cancelled) = tokio::sync::oneshot::channel();
        self.busy = true;
        self.deployment.update(cx, |state, cx| {
            state.saving = true;
            state.cancel = Some(cancel);
            state.status.clear();
            cx.notify();
        });
        self.sync_modal(cx);
        let task = cx.background_executor().spawn(async move {
            file.ensure_unchanged()?;
            target.validate()?;
            let token = if target.named_token_id.is_some() { target.named_token(&TokenCatalog::load(&tokens_path)?)? } else { target.token(&root)? };
            let client = crate::desktop::deployment::DeploymentClient::new(&target, &token)?;
            drop(token);
            settings.save(&path)?;
            for warning in crate::run_history::staging::cleanup_abandoned(&root) {
                eprintln!("{warning}");
            }
            let staging = crate::run_history::staging::StagingDirectory::create(
                &root, crate::run_history::staging::StagingKind::Deployment,
            )?;
            let report = staging.path().join("report");
            std::fs::create_dir(&report)?;
            file.app.stage(&report)?;
            let url = tokio::runtime::Builder::new_multi_thread().enable_all().build()?.block_on(async {
                tokio::select! {
                    biased;
                    _ = cancelled => Err(anyhow!("Deployment cancelled locally. A provider may already have published the upload; check the hosted site before retrying")),
                    result = client.deploy(&report, &target) => result,
                }
            })?;
            let saved = DeployTargets::record_publication(&path, &target, url.as_deref());
            Ok::<_, anyhow::Error>((url, saved))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |this, _, cx| {
                this.busy = false;
                this.deployment.update(cx, |state, cx| {
                    state.saving = false;
                    state.cancel = None;
                    match result {
                        Ok((url, saved)) => {
                            if let Some(target) = state.settings.targets.iter_mut().find(|target| target.id == target_id) {
                                target.published_url = url;
                            }
                            match saved {
                                Ok(settings) => {
                                    state.settings = settings;
                                    state.status.clear();
                                }
                                Err(error) => state.status = format!("Site deployed, but its URL could not be saved to the target: {error:#}"),
                            }
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
    use crate::desktop::deployment::{DeployTarget, DeployTargets, DeploymentProvider};
    use gpui_kit::component::WindowExt;
    use gpui_kit::test::{TestAppContextExt, TestWindowExt};
    use gpui_kit::{AppContext, TestAppContext};
    use std::time::Duration;

    #[gpui_kit::test]
    async fn clipboard_copy_keeps_exact_text_and_refreshes_value_identity(cx: &mut TestAppContext) {
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
        super::super::tests::settle_dialog(cx, handle).await;
        let first = "https://reports.example/odd%20name/%E6%B5%8B/";
        let second = "https://reports.example/another%20target/";
        cx.update_window(handle.into(), |_, window, cx| {
            view.read(cx).deployment.clone().update(cx, |state, cx| {
                state.settings = serde_json::from_value(serde_json::json!({
                    "targets": [{"id": "copy", "name": "Copy fixture", "publishedUrl": first}],
                    "selected": "copy"
                }))
                .unwrap();
                cx.notify();
            });
            window.render_frame(cx);
            for _ in 0..24 {
                if window
                    .within("deploy-copy-published")
                    .find(("clipboard-value", 1_u64))
                    .focused()
                    == Some(true)
                {
                    break;
                }
                window.press("tab", cx);
                window.render_frame(cx);
            }
            assert_eq!(
                window
                    .within("deploy-copy-published")
                    .find(("clipboard-value", 1_u64))
                    .focused(),
                Some(true)
            );
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                "synthetic-sentinel".into(),
            ));
            window.press("space", cx);
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().as_deref(),
                Some(first)
            );
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                "unchanged-copy-suppressed".into(),
            ));
            window.click("deploy-copy-published", cx);
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().as_deref(),
                Some("unchanged-copy-suppressed")
            );
            for value in [second, first] {
                view.read(cx).deployment.clone().update(cx, |state, cx| {
                    state.settings.targets[0].published_url = Some(value.into());
                    cx.notify();
                });
                window.click("deploy-copy-published", cx);
                assert_eq!(
                    cx.read_from_clipboard().unwrap().text().as_deref(),
                    Some(value)
                );
            }
            view.read(cx).deployment.clone().update(cx, |state, cx| {
                state.settings.targets[0].published_url = None;
                cx.notify();
            });
            window.render_frame(cx);
            assert!(window.try_find("deploy-copy-published").is_none());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    async fn value_selector_deployment_provider_and_token_source_keep_guarded_drafts(
        cx: &mut TestAppContext,
    ) {
        let root = tempfile::tempdir().unwrap();
        let account = "a".repeat(32);
        let token_id = uuid::Uuid::new_v4().to_string();
        let (handle, view) = super::super::tests::editor(cx, root.path());
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| this.open_deployment(window, cx));
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
            window.try_find("deploy-target-add").is_some()
        })
        .await;
        super::super::tests::settle_dialog(cx, handle).await;
        cx.update_window(handle.into(), |_, window, cx| {
            view.read(cx).deployment.clone().update(cx, |state, _| {
                state
                    .tokens
                    .tokens
                    .push(crate::desktop::deployment::tokens::NamedToken {
                        id: token_id.clone(),
                        name: "Shared account".into(),
                        provider: DeploymentProvider::CloudflareWorkers,
                        account_id: account.clone(),
                    });
            });
            window.click("deploy-target-add", cx);
            window.click("deploy-token", cx);
            window.input("synthetic-unsaved-secret", cx);
            window.click("deploy-provider", cx);
            window.input("cloudflare", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| window.press("enter", cx))
            .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            view.read(cx).deployment.read(cx).provider == DeploymentProvider::CloudflareWorkers
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            assert!(
                view.read(cx)
                    .deployment
                    .read(cx)
                    .token
                    .read(cx)
                    .value()
                    .is_empty()
            );
            assert_eq!(
                view.read(cx).deployment.read(cx).cloudflare.workers_dev,
                Some(true)
            );
            window.click("deploy-token", cx);
            window.input("synthetic-unsaved-direct-token", cx);
            window.click("deploy-token-source", cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            assert_eq!(
                view.read(cx)
                    .deployment
                    .read(cx)
                    .token
                    .read(cx)
                    .value()
                    .as_str(),
                "synthetic-unsaved-direct-token"
            );
            window.click("deploy-token-source", cx);
            window.input("shared", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| window.press("enter", cx))
            .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            view.read(cx).deployment.read(cx).named_token_id.as_deref() == Some(token_id.as_str())
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("deploy-token").is_none());
            assert!(window.try_find("deploy-account-id").is_none());
            assert_eq!(
                view.read(cx)
                    .deployment
                    .read(cx)
                    .account_id
                    .read(cx)
                    .value()
                    .as_str(),
                account
            );
            window.click("deploy-cloudflare-options", cx);
        })
        .unwrap();
        for (control, query) in [
            ("deploy-cloudflare-routing", "private"),
            ("deploy-cloudflare-html", "none"),
            ("deploy-cloudflare-not-found", "404-page"),
        ] {
            cx.update_window(handle.into(), |_, window, cx| {
                window.click(control, cx);
                window.input(query, cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| window.press("enter", cx))
                .unwrap();
            cx.run_until_parked();
        }
        cx.update_window(handle.into(), |_, window, cx| {
            let state = view.read(cx).deployment.read(cx);
            assert_eq!(state.cloudflare.workers_dev, Some(false));
            assert_eq!(state.cloudflare.html_handling, "none");
            assert_eq!(state.cloudflare.not_found_handling, "404-page");
            window.click("deploy-cloudflare-options", cx);
            window.click("deploy-token-source", cx);
            window.input("target-specific", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| window.press("enter", cx))
            .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            view.read(cx).deployment.read(cx).named_token_id.is_none()
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("deploy-token").visible());
            assert!(
                view.read(cx)
                    .deployment
                    .read(cx)
                    .token
                    .read(cx)
                    .value()
                    .is_empty()
            );
            window.click("deploy-provider", cx);
            window.input("netlify", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| window.press("enter", cx))
            .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            view.read(cx).deployment.read(cx).provider == DeploymentProvider::Netlify
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            view.read(cx).deployment.clone().update(cx, |state, cx| {
                state.token_picker.state.update(cx, |_, cx| {
                    cx.emit(super::SelectEvent::<super::choices::ValueChoices>::Confirm(
                        Some(token_id.clone()),
                    ))
                });
                state.saving = true;
                state.provider_picker.state.update(cx, |select, cx| {
                    select.set_selected_value(&"azureSwa".into(), window, cx);
                    cx.emit(super::SelectEvent::<super::choices::ValueChoices>::Confirm(
                        Some("azureSwa".into()),
                    ));
                });
            });
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let state = view.read(cx).deployment.read(cx);
            assert_eq!(state.provider, DeploymentProvider::Netlify);
            assert!(state.named_token_id.is_none());
            assert_eq!(
                state
                    .provider_picker
                    .state
                    .read(cx)
                    .selected_value()
                    .map(String::as_str),
                Some("netlify")
            );
            assert_eq!(
                state
                    .token_picker
                    .state
                    .read(cx)
                    .selected_value()
                    .map(String::as_str),
                Some("")
            );
            assert!(state.settings.targets.is_empty());
        });
    }

    #[gpui_kit::test]
    async fn value_selector_deployment_uses_ids_and_rejects_stale_selection(
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
        super::super::tests::settle_dialog(cx, handle).await;
        cx.update_window(handle.into(), |_, window, cx| {
            view.read(cx).deployment.clone().update(cx, |state, cx| {
                state.settings = serde_json::from_value(serde_json::json!({
                    "targets": [
                        {"id": "first", "name": "Shared target"},
                        {"id": "second", "name": "Shared target"},
                        {"id": "other", "name": "Other target"}
                    ],
                    "selected": "first"
                }))
                .unwrap();
                cx.notify();
            });
            window.render_frame(cx);
            window.click("deploy-target-menu", cx);
            window.input("shared", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            view.read(cx)
                .deployment
                .clone()
                .update(cx, |_, cx| cx.notify());
            window.render_frame(cx);
            window.press("down", cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            view.read(cx)
                .deployment
                .read(cx)
                .settings
                .selected
                .as_deref()
                == Some("second")
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            view.read(cx).deployment.clone().update(cx, |state, cx| {
                state.settings.targets[1].name = "Renamed target".into();
                state.settings.targets.swap(0, 1);
                cx.notify();
            });
            window.render_frame(cx);
            assert_eq!(
                window.find("deploy-target-menu").label(),
                Some("Renamed target")
            );
            assert_eq!(
                view.read(cx)
                    .deployment
                    .read(cx)
                    .target_picker
                    .state
                    .read(cx)
                    .selected_value()
                    .map(String::as_str),
                Some("second")
            );
            view.read(cx).deployment.clone().update(cx, |state, cx| {
                state.settings.selected = Some("removed-target".into());
                cx.notify();
            });
            window.render_frame(cx);
            assert_eq!(
                window.find("deploy-target-menu").label(),
                Some("removed-target")
            );
            assert!(window.find("deploy-target-unavailable").visible());
            assert!(!view.read(cx).deployment.read(cx).can_deploy());
            view.read(cx).deployment.clone().update(cx, |state, cx| {
                state.saving = true;
                state.target_picker.state.update(cx, |select, cx| {
                    select.set_selected_value(&"second".into(), window, cx);
                    cx.emit(super::SelectEvent::<super::choices::ValueChoices>::Confirm(
                        Some("second".into()),
                    ));
                });
            });
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let state = view.read(cx).deployment.read(cx);
            assert_eq!(state.settings.selected.as_deref(), Some("removed-target"));
            assert!(
                state
                    .target_picker
                    .state
                    .read(cx)
                    .selected_value()
                    .is_none()
            );
        });
    }

    #[gpui_kit::test]
    async fn saved_deployment_urls_reopen_and_copy_for_reports_and_landings(
        cx: &mut TestAppContext,
    ) {
        use crate::desktop::landing::{LandingApp, LandingFile};

        for landing in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let preferences = root.path().join("desktop-preferences.json");
            let file = landing
                .then(|| LandingFile::create_managed(&preferences, LandingApp::default()).unwrap());
            let deployment_root = file
                .as_ref()
                .map(|file| file.app.deployment_root(&preferences).unwrap())
                .unwrap_or_else(|| root.path().to_path_buf());
            let long_url = format!(
                "https://qa.example.workers.dev/{}",
                "long-path/".repeat(200)
            );
            let settings: DeployTargets = serde_json::from_value(serde_json::json!({
                "targets": [
                    {"id": "azure", "name": "Azure", "publishedUrl": "https://qa.azurestaticapps.net/"},
                    {"id": "netlify", "name": "Netlify", "provider": "netlify", "siteId": "12345678-1234-1234-1234-123456789abc", "publishedUrl": "https://qa.netlify.app/"},
                    {"id": "cloudflare", "name": "Cloudflare", "provider": "cloudflareWorkers", "cloudflare": {"accountId": "a".repeat(32), "workerName": "qa"}, "publishedUrl": long_url},
                    {"id": "unpublished", "name": "Unpublished"}
                ],
                "selected": "azure"
            })).unwrap();
            settings
                .save(&DeployTargets::path_in(&deployment_root, &preferences).unwrap())
                .unwrap();
            let (handle, view) = super::super::tests::editor_at_size(
                cx,
                root.path(),
                gpui_kit::size(gpui_kit::px(1000.), gpui_kit::px(640.)),
            );
            for _ in 0..2 {
                cx.update_window(handle.into(), |_, window, cx| {
                    view.update(cx, |this, cx| {
                        this.deployment = cx.new(|cx| super::DeploymentPanel::new(window, cx));
                        if let Some(file) = &file {
                            this.open_landing_deployment(file.clone(), window, cx);
                        } else {
                            this.open_deployment(window, cx);
                        }
                    });
                })
                .unwrap();
                cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
                    window.try_find("deploy-copy-published").is_some()
                })
                .await;
                cx.update_window(handle.into(), |_, window, cx| {
                    assert_eq!(
                        view.read(cx)
                            .deployment
                            .read(cx)
                            .settings
                            .selected
                            .as_deref(),
                        Some("azure")
                    );
                    assert!(!view.read(cx).deployment.read(cx).can_deploy());
                    assert!(window.try_find("deployment-suite-list").is_none());
                    for dark in [false, true] {
                        super::super::design::apply_theme(dark, Some(window), cx);
                        for target in &settings.targets {
                            view.update(cx, |this, cx| {
                                this.choose_deploy_target(
                                    &super::ChooseDeployTarget {
                                        id: target.id.clone(),
                                    },
                                    window,
                                    cx,
                                )
                            });
                            window.render_frame(cx);
                            if let Some(url) = &target.published_url {
                                assert_eq!(
                                    window.find("deploy-open-published").label(),
                                    Some("Open deployed site")
                                );
                                let open = window.find("deploy-open-published").bounds();
                                let copy = window.find("deploy-copy-published").bounds();
                                assert!(open.origin.x + open.size.width <= copy.origin.x);
                                assert!(
                                    copy.origin.x - open.origin.x - open.size.width
                                        <= gpui_kit::px(8.)
                                );
                                if url.len() < 50 {
                                    assert!(open.size.width <= gpui_kit::px(280.));
                                }
                                assert!(copy.origin.x + copy.size.width <= gpui_kit::px(1000.));
                                assert!(copy.origin.y + copy.size.height <= gpui_kit::px(640.));
                                window.click("deploy-copy-published", cx);
                                assert_eq!(
                                    cx.read_from_clipboard().unwrap().text().as_deref(),
                                    Some(url.as_str())
                                );
                            } else {
                                assert!(window.try_find("deploy-open-published").is_none());
                                assert!(window.try_find("deploy-copy-published").is_none());
                            }
                        }
                    }
                    window.close_dialog(cx);
                })
                .unwrap();
            }
        }
    }

    #[gpui_kit::test]
    async fn deployment_targets_use_masked_input_and_secure_storage(cx: &mut TestAppContext) {
        let root = tempfile::tempdir().unwrap();
        let token = format!("{}-{}000", "a".repeat(64), "b".repeat(36));
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
            window.click("deploy-token", cx);
            window.input(&token, cx);
            window.render_frame(cx);
            assert!(window.find("deploy-token").value().is_none());
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
            assert!(
                view.read(cx)
                    .deployment
                    .read(cx)
                    .token
                    .read(cx)
                    .value()
                    .is_empty()
            );
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
            assert_eq!(
                settings.targets[0].token(root.path()).unwrap().as_str(),
                token
            );
            assert!(
                !std::fs::read_to_string(state.path.as_ref().unwrap())
                    .unwrap()
                    .contains(&token)
            );
            assert!(state.token.read(cx).value().is_empty());
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
    async fn netlify_target_fields_and_tokens_stay_provider_scoped(cx: &mut TestAppContext) {
        let root = tempfile::tempdir().unwrap();
        let site_id = uuid::Uuid::new_v4().to_string();
        let (handle, view) = super::super::tests::editor_at_size(
            cx,
            root.path(),
            gpui_kit::size(gpui_kit::px(1000.), gpui_kit::px(640.)),
        );
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| this.open_deployment(window, cx))
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
            window.try_find("deploy-target-add").is_some()
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("deploy-target-add", cx);
            view.update(cx, |this, cx| {
                this.choose_deploy_provider(
                    &super::ChooseDeployProvider {
                        provider: DeploymentProvider::Netlify,
                    },
                    window,
                    cx,
                )
            });
            window.render_frame(cx);
            assert!(window.try_find("deploy-environment").is_none());
            assert!(window.find("deploy-site-id").visible());
            window.click("deploy-name", cx);
            window.input("Netlify QA", cx);
            window.click("deploy-site-id", cx);
            window.input(&site_id, cx);
            window.click("deploy-token", cx);
            window.input("synthetic-netlify-token", cx);
            window.render_frame(cx);
            assert!(window.find("deploy-token").value().is_none());
            for dark in [false, true] {
                super::super::design::apply_theme(dark, Some(window), cx);
                window.render_frame(cx);
                let close = window.find("deploy-close").bounds();
                assert!(close.origin.y + close.size.height <= gpui_kit::px(640.));
            }
            window.click("deploy-target-save", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            let state = view.read(cx).deployment.read(cx);
            !state.saving && state.settings.targets.len() == 1
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            let state = view.read(cx).deployment.read(cx);
            let target = &state.settings.targets[0];
            assert_eq!(target.provider, DeploymentProvider::Netlify);
            assert_eq!(target.site_id, site_id);
            assert!(target.environment.is_empty());
            assert_eq!(
                target.token(root.path()).unwrap().as_str(),
                "synthetic-netlify-token"
            );
            window.click("deploy-target-edit", cx);
            window.render_frame(cx);
            assert_eq!(
                window.find("deploy-site-id").value(),
                Some(site_id.as_str())
            );
            window.click("deploy-token", cx);
            window.input("unsaved-netlify-token", cx);
            view.update(cx, |this, cx| {
                this.choose_deploy_provider(
                    &super::ChooseDeployProvider {
                        provider: DeploymentProvider::AzureSwa,
                    },
                    window,
                    cx,
                )
            });
            window.render_frame(cx);
            assert!(window.try_find("deploy-site-id").is_none());
            assert!(window.find("deploy-environment").visible());
            assert!(
                view.read(cx)
                    .deployment
                    .read(cx)
                    .token
                    .read(cx)
                    .value()
                    .is_empty()
            );
            window.click("deploy-target-save", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            view.read(cx)
                .deployment
                .read(cx)
                .status
                .contains("when changing providers")
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            let state = view.read(cx).deployment.read(cx);
            assert_eq!(
                state.settings.targets[0].provider,
                DeploymentProvider::Netlify
            );
            assert_eq!(
                state.settings.targets[0]
                    .token(root.path())
                    .unwrap()
                    .as_str(),
                "synthetic-netlify-token"
            );
            window.click("deploy-target-cancel", cx);
            window.click("deploy-close", cx);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    async fn cloudflare_target_fields_save_and_preserve_tokens(cx: &mut TestAppContext) {
        let root = tempfile::tempdir().unwrap();
        let account = "0123456789abcdef0123456789abcdef";
        let (handle, view) = super::super::tests::editor_at_size(
            cx,
            root.path(),
            gpui_kit::size(gpui_kit::px(1000.), gpui_kit::px(640.)),
        );
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| this.open_deployment(window, cx));
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
            window.try_find("deploy-target-add").is_some()
        })
        .await;
        super::super::tests::settle_dialog(cx, handle).await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("deploy-target-add", cx);
            view.update(cx, |this, cx| {
                this.choose_deploy_provider(
                    &super::ChooseDeployProvider {
                        provider: DeploymentProvider::CloudflareWorkers,
                    },
                    window,
                    cx,
                )
            });
            window.render_frame(cx);
            assert!(window.try_find("deploy-environment").is_none());
            assert!(window.try_find("deploy-site-id").is_none());
            assert_eq!(
                window.find("deploy-cloudflare-routing").label(),
                Some("Cloudflare public routing")
            );
            assert_eq!(
                view.read(cx).deployment.read(cx).cloudflare.workers_dev,
                Some(true)
            );
            window.click("deploy-name", cx);
            window.input("Cloudflare QA", cx);
            window.click("deploy-account-id", cx);
            window.input(account, cx);
            window.click("deploy-worker-name", cx);
            window.input("reports", cx);
            window.click("deploy-token", cx);
            window.input("synthetic-cloudflare-token", cx);
            for dark in [false, true] {
                super::super::design::apply_theme(dark, Some(window), cx);
                window.render_frame(cx);
                assert!(window.find("deploy-token").value().is_none());
                let close = window.find("deploy-close").bounds();
                assert!(close.origin.y + close.size.height <= gpui_kit::px(640.));
            }
            window.click("deploy-target-save", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            let state = view.read(cx).deployment.read(cx);
            !state.saving && state.settings.targets.len() == 1
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            let state = view.read(cx).deployment.read(cx);
            let target = &state.settings.targets[0];
            assert_eq!(target.provider, DeploymentProvider::CloudflareWorkers);
            assert_eq!(target.cloudflare.as_ref().unwrap().account_id, account);
            assert_eq!(target.cloudflare.as_ref().unwrap().worker_name, "reports");
            assert_eq!(target.cloudflare.as_ref().unwrap().workers_dev, Some(true));
            assert!(target.environment.is_empty() && target.site_id.is_empty());
            assert!(
                !std::fs::read_to_string(state.path.as_ref().unwrap())
                    .unwrap()
                    .contains("synthetic-cloudflare-token")
            );
            window.click("deploy-target-edit", cx);
            window.render_frame(cx);
            assert_eq!(window.find("deploy-account-id").value(), Some(account));
            assert_eq!(window.find("deploy-worker-name").value(), Some("reports"));
            window.click("deploy-cloudflare-options", cx);
            window.render_frame(cx);
            assert!(window.find("deploy-cloudflare-html").visible());
            view.update(cx, |this, cx| {
                this.choose_cloudflare_setting(
                    &super::ChooseCloudflareSetting {
                        setting: "routing".into(),
                        value: "private".into(),
                    },
                    window,
                    cx,
                );
                this.choose_cloudflare_setting(
                    &super::ChooseCloudflareSetting {
                        setting: "html".into(),
                        value: "none".into(),
                    },
                    window,
                    cx,
                );
                this.choose_cloudflare_setting(
                    &super::ChooseCloudflareSetting {
                        setting: "notFound".into(),
                        value: "404-page".into(),
                    },
                    window,
                    cx,
                );
                this.deployment.update(cx, |state, cx| {
                    state
                        .compatibility_date
                        .update(cx, |input, cx| input.set_value("2026-09-28", window, cx));
                    state
                        .excludes
                        .update(cx, |input, cx| input.set_value("[\"*.map\"]", window, cx));
                });
            });
            window.click("deploy-cloudflare-options", cx);
            window.render_frame(cx);
            assert!(
                view.read(cx)
                    .deployment
                    .read(cx)
                    .token
                    .read(cx)
                    .value()
                    .is_empty()
            );
            window.click("deploy-target-save", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            !view.read(cx).deployment.read(cx).saving
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("deploy-target-edit", cx);
            let saved = view.read(cx).deployment.read(cx).settings.targets[0]
                .cloudflare
                .as_ref()
                .unwrap()
                .clone();
            assert_eq!(saved.workers_dev, Some(false));
            assert_eq!(saved.html_handling, "none");
            assert_eq!(saved.not_found_handling, "404-page");
            assert_eq!(saved.compatibility_date, "2026-09-28");
            assert_eq!(saved.exclude, ["*.map"]);
            assert_eq!(view.read(cx).deployment.read(cx).cloudflare, saved);
            view.update(cx, |this, cx| {
                this.choose_deploy_provider(
                    &super::ChooseDeployProvider {
                        provider: DeploymentProvider::AzureSwa,
                    },
                    window,
                    cx,
                )
            });
            window.render_frame(cx);
            assert!(window.try_find("deploy-account-id").is_none());
            assert!(window.try_find("deploy-worker-name").is_none());
            window.click("deploy-target-save", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            view.read(cx)
                .deployment
                .read(cx)
                .status
                .contains("when changing providers")
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            let target = &view.read(cx).deployment.read(cx).settings.targets[0];
            assert_eq!(target.provider, DeploymentProvider::CloudflareWorkers);
            assert_eq!(
                target.token(root.path()).unwrap().as_str(),
                "synthetic-cloudflare-token"
            );
            window.click("deploy-target-cancel", cx);
            window.click("deploy-close", cx);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    async fn deployment_token_drafts_are_cleared_on_cancel_close_and_escape(
        cx: &mut TestAppContext,
    ) {
        let root = tempfile::tempdir().unwrap();
        let (handle, view) = super::super::tests::editor(cx, root.path());
        for dismissal in ["cancel", "close", "escape"] {
            cx.update_window(handle.into(), |_, window, cx| {
                view.update(cx, |this, cx| this.open_deployment(window, cx));
            })
            .unwrap();
            cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
                window.try_find("deploy-target-add").is_some()
            })
            .await;
            super::super::tests::settle_dialog(cx, handle).await;
            cx.update_window(handle.into(), |_, window, cx| {
                window.click("deploy-target-add", cx);
                window.render_frame(cx);
                window.click("deploy-token", cx);
                window.input("synthetic-unsaved-token", cx);
                window.render_frame(cx);
                assert!(window.find("deploy-token").value().is_none());
                match dismissal {
                    "cancel" => window.click("deploy-target-cancel", cx),
                    "close" => window.click("deploy-close", cx),
                    _ => window.press("escape", cx),
                }
                let state = view.read(cx).deployment.read(cx);
                assert!(state.token.read(cx).value().is_empty());
                assert!(state.settings.targets.is_empty());
                assert!(!state.editing);
                if dismissal == "cancel" {
                    window.click("deploy-close", cx);
                }
            })
            .unwrap();
        }
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
                            published_url: None,
                            credential_id: None,
                            named_token_id: None,
                            environment: String::new(),
                            provider: Default::default(),
                            site_id: String::new(),
                            cloudflare: None,
                        }],
                        ..Default::default()
                    };
                    state.catalog_ids =
                        suites.iter().map(|suite| suite.output_id.clone()).collect();
                    state.suite_ids = state.catalog_ids.clone();
                    assert!(!state.can_deploy());
                    state.intent = Some(intent);
                    state.partial = true;
                    assert!(!state.can_deploy());
                    state.confirmed = true;
                    assert!(!state.can_deploy());
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
            window.render_frame(cx);
            assert!(!view.read(cx).deployment.read(cx).can_deploy());
            assert!(window.try_find("deploy-add-token").is_some());
            window.click("deploy-add-token", cx);
            window.render_frame(cx);
            assert!(view.read(cx).deployment.read(cx).editing);
            assert!(!view.read(cx).deployment.read(cx).can_deploy());
            window.click("deploy-token", cx);
            window.input(&format!("{}-{}000", "a".repeat(64), "b".repeat(36)), cx);
            window.click("deploy-target-save", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            view.read(cx).deployment.read(cx).can_deploy()
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("deploy-add-token").is_none());
            view.update(cx, |this, cx| {
                this.deployment.update(cx, |state, cx| {
                    state.saving = true;
                    cx.notify();
                });
            });
            window.render_frame(cx);
            assert_eq!(
                window.find("deploy-confirm").label(),
                Some("Starting deployment")
            );
            window.click("deploy-confirm", cx);
            assert!(!view.read(cx).runs.read(cx).running());
            assert!(!view.read(cx).deployment.read(cx).can_deploy());
            view.update(cx, |this, cx| {
                this.deployment.update(cx, |state, cx| {
                    state.saving = false;
                    cx.notify();
                });
            });
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
            let state = view.read(cx).deployment.read(cx);
            let mut settings = state.settings.clone();
            settings
                .remove_target(root.path(), state.path.as_ref().unwrap(), "qa")
                .unwrap();
        })
        .unwrap();
    }
}
