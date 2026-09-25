use super::*;
use crate::desktop::landing::{
    LandingApp, LandingCatalog, LandingFile, LandingPreview, LandingSite,
};

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
pub(super) struct ChooseLandingApp {
    path: PathBuf,
}

#[derive(Clone)]
enum LandingAction {
    New,
    Open(PathBuf),
    Close,
    Quit,
    Remove,
}

struct SiteInputs {
    id: String,
    heading: Entity<InputState>,
    subheading: Entity<TextareaState>,
    url: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

pub(super) struct LandingPanel {
    catalog_path: Option<PathBuf>,
    files: Vec<(PathBuf, String)>,
    open: Option<LandingFile>,
    baseline: LandingApp,
    heading: Entity<InputState>,
    subheading: Entity<TextareaState>,
    sites: Vec<SiteInputs>,
    pending: Option<LandingAction>,
    saving: bool,
    visible: bool,
    preview: Option<LandingPreview>,
    scroll: ScrollHandle,
    status: String,
    subscriptions: Vec<Subscription>,
}

impl LandingPanel {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let app = LandingApp::default();
        let heading = cx.new(|cx| InputState::new(window, cx).default_value(app.heading.clone()));
        let subheading = cx.new(|cx| TextareaState::new(window, cx).rows(3));
        let subscriptions = vec![
            cx.subscribe(&heading, |_, _, _: &InputEvent, cx| cx.notify()),
            cx.subscribe(&subheading, |_, _, _: &InputEvent, cx| cx.notify()),
        ];
        Self {
            catalog_path: None,
            files: Vec::new(),
            open: None,
            baseline: app,
            heading,
            subheading,
            sites: Vec::new(),
            pending: None,
            saving: false,
            visible: false,
            preview: None,
            scroll: ScrollHandle::new(),
            status: String::new(),
            subscriptions,
        }
    }

    fn app(&self, cx: &App) -> LandingApp {
        LandingApp {
            version: 1,
            id: self.baseline.id.clone(),
            heading: self.heading.read(cx).value().to_string(),
            subheading: self.subheading.read(cx).value().to_string(),
            sites: self
                .sites
                .iter()
                .map(|site| LandingSite {
                    heading: site.heading.read(cx).value().to_string(),
                    subheading: site.subheading.read(cx).value().to_string(),
                    url: site.url.read(cx).value().to_string(),
                })
                .collect(),
        }
    }

    pub(super) fn dirty(&self, cx: &App) -> bool {
        self.app(cx) != self.baseline
    }

    fn set_app(&mut self, file: Option<LandingFile>, window: &mut Window, cx: &mut Context<Self>) {
        let app = file
            .as_ref()
            .map(|file| file.app.clone())
            .unwrap_or_default();
        self.baseline = app.clone();
        self.open = file;
        self.heading
            .update(cx, |input, cx| input.set_value(app.heading, window, cx));
        self.subheading
            .update(cx, |input, cx| input.set_value(app.subheading, window, cx));
        self.sites.clear();
        self.subscriptions.truncate(2);
        for site in app.sites {
            self.add_site(site, window, cx);
        }
        self.status.clear();
        cx.notify();
    }

    fn add_site(&mut self, site: LandingSite, window: &mut Window, cx: &mut Context<Self>) {
        let heading = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(site.heading)
                .placeholder("Site heading")
        });
        let subheading = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(2)
                .default_value(site.subheading)
        });
        let url = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(site.url)
                .placeholder("https://reports.example.com/")
        });
        let subscriptions = vec![
            cx.subscribe(&heading, |_, _, _: &InputEvent, cx| cx.notify()),
            cx.subscribe(&subheading, |_, _, _: &InputEvent, cx| cx.notify()),
            cx.subscribe(&url, |_, _, _: &InputEvent, cx| cx.notify()),
        ];
        self.sites.push(SiteInputs {
            id: uuid::Uuid::new_v4().to_string(),
            heading,
            subheading,
            url,
            _subscriptions: subscriptions,
        });
        cx.notify();
    }
}

fn catalog_entries(catalog: LandingCatalog) -> Vec<(PathBuf, String)> {
    catalog
        .files
        .into_iter()
        .map(|path| {
            let label = LandingFile::open(&path)
                .map(|file| file.app.heading)
                .unwrap_or_else(|_| format!("Unavailable: {}", display_path(&path)));
            (path, label)
        })
        .collect()
}

impl Workbench {
    pub(super) fn guard_landing_close(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.landing.read(cx).dirty(cx) {
            return false;
        }
        window.close_dialog(cx);
        self.landing.update(cx, |state, cx| {
            state.pending = Some(LandingAction::Quit);
            state.visible = true;
            cx.notify();
        });
        self.show_landing_manager(window, cx);
        true
    }

    pub(super) fn open_landing_manager(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.runs.read(cx).running() {
            return;
        }
        let Some(preferences) = self.preferences_path.clone() else {
            return;
        };
        self.task(
            "Loading landing apps...",
            move || {
                let path = LandingCatalog::path_in(&preferences)?;
                Ok((catalog_entries(LandingCatalog::load(&path)?), path))
            },
            |this, (files, path), window, cx| {
                this.landing.update(cx, |state, cx| {
                    state.files = files;
                    state.catalog_path = Some(path);
                    state.visible = true;
                    state.pending = None;
                    cx.notify();
                });
                this.show_landing_manager(window, cx);
            },
            window,
            cx,
        );
    }

    pub(super) fn show_landing_manager(&self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = self.landing.clone();
        let view = cx.entity();
        let focus = self.focus.clone();
        window.open_dialog(cx, move |dialog, window, cx| {
            let state = panel.read(cx);
            let files = state.files.clone();
            let locked = state.saving || state.pending.is_some();
            let mut body = div().id("landing-editor-body").test_support().h(px(420.)).overflow_y_scroll().track_scroll(&state.scroll).flex().flex_col().gap_3().min_w_0();
            body = body.child(div().flex().flex_shrink_0().gap_2().items_center()
                .child(Button::new("landing-app-menu").flex_1().min_w_0().label(state.open.as_ref().map(|file| file.app.heading.clone()).unwrap_or_else(|| "New landing app".into()))
                    .icon(gpui_kit::assets::IconName::ChevronDown).disabled(locked || files.is_empty())
                    .dropdown_menu({ let focus = focus.clone(); move |menu, _, _| {
                        let mut menu = menu.action_context(focus.clone());
                        for (path, label) in &files { menu = menu.menu(label.clone(), Box::new(ChooseLandingApp { path: path.clone() })); }
                        menu
                    }}))
                .child(Button::new("landing-new").ghost().icon(IconName::Plus).tooltip("New landing app").accessibility_label("New landing app").disabled(locked)
                    .on_click(window.listener_for(&view, |this, _, window, cx| this.request_landing_action(LandingAction::New, window, cx))))
                .child(Button::new("landing-remove").ghost().icon(gpui_kit::assets::IconName::Trash).tooltip("Remove landing app").accessibility_label("Remove landing app").disabled(locked || state.open.is_none())
                    .on_click(window.listener_for(&view, |this, _, window, cx| this.request_landing_action(LandingAction::Remove, window, cx)))));
            if let Some(pending) = state.pending.clone() {
                let removing = matches!(pending, LandingAction::Remove);
                body = body.child(if removing { "Remove this local landing definition and its deployment settings? Hosted websites will not be deleted." } else { "Save changes to this landing app?" })
                    .child(div().flex().flex_wrap().justify_end().gap_2()
                        .child(Button::new("landing-keep-editing").label("Keep editing").disabled(state.saving).on_click(window.listener_for(&view, |this, _, _, cx| this.landing.update(cx, |state, cx| { state.pending = None; cx.notify(); }))))
                        .when(!removing, |row| row.child(Button::new("landing-save-pending").primary().label("Save changes").disabled(state.saving).on_click(window.listener_for(&view, |this, _, window, cx| this.save_landing(window, cx)))))
                        .child(Button::new("landing-discard").danger().label(if removing { "Remove locally" } else { "Discard changes" }).disabled(state.saving).on_click(window.listener_for(&view, move |this, _, window, cx| {
                            this.landing.update(cx, |state, cx| { state.pending = None; cx.notify(); });
                            this.perform_landing_action(pending.clone(), window, cx);
                        }))));
            } else {
                body = body.child(div().flex().flex_col().flex_shrink_0().gap_1().child("Heading").child(Input::new(&state.heading).id("landing-heading").aria_label("Landing heading").disabled(locked)))
                    .child(div().flex().flex_col().flex_shrink_0().gap_1().child("Subheading (Markdown)").child(div().id("landing-subheading").test_support().child(Textarea::new(&state.subheading).aria_label("Landing subheading Markdown").h(px(72.)).disabled(locked))));
                for (index, site) in state.sites.iter().enumerate() {
                    let id = site.id.clone();
                    let up = id.clone();
                    let down = id.clone();
                    let remove = id.clone();
                    body = body.child(div().id(SharedString::from(format!("landing-site-{id}"))).flex().flex_col().flex_shrink_0().gap_2().pt_3().border_t_1().border_color(cx.theme().border)
                        .child(div().flex().items_center().gap_1().child(div().flex_1().child(format!("Site {}", index + 1)))
                            .child(Button::new(SharedString::from(format!("landing-site-up-{id}"))).small().ghost().icon(IconName::ArrowUp).tooltip("Move site up").accessibility_label("Move site up").disabled(locked || index == 0)
                                .on_click(window.listener_for(&view, move |this, _, _, cx| this.move_landing_site(&up, -1, cx))))
                            .child(Button::new(SharedString::from(format!("landing-site-down-{id}"))).small().ghost().icon(IconName::ArrowDown).tooltip("Move site down").accessibility_label("Move site down").disabled(locked || index + 1 == state.sites.len())
                                .on_click(window.listener_for(&view, move |this, _, _, cx| this.move_landing_site(&down, 1, cx))))
                            .child(Button::new(SharedString::from(format!("landing-site-remove-{id}"))).small().ghost().icon(gpui_kit::assets::IconName::Trash).tooltip("Remove site").accessibility_label("Remove site").disabled(locked)
                                .on_click(window.listener_for(&view, move |this, _, _, cx| this.landing.update(cx, |state, cx| { if !state.saving { state.sites.retain(|site| site.id != remove); cx.notify(); } })))))
                        .child(div().flex().flex_col().gap_1().child("Site heading").child(Input::new(&site.heading).id(SharedString::from(format!("landing-site-heading-{id}"))).aria_label(format!("Site {} heading", index + 1)).disabled(locked)))
                        .child(div().flex().flex_col().gap_1().child("Subheading (Markdown)").child(div().id(SharedString::from(format!("landing-site-subheading-{id}"))).test_support().child(Textarea::new(&site.subheading).aria_label(format!("Site {} subheading Markdown", index + 1)).h(px(60.)).disabled(locked))))
                        .child(div().flex().flex_col().gap_1().child("URL").child(Input::new(&site.url).id(SharedString::from(format!("landing-site-url-{id}"))).aria_label(format!("Site {} URL", index + 1)).disabled(locked))));
                }
                body = body.child(Button::new("landing-add-site").small().flex_shrink_0().icon(IconName::Plus).label("Add site").disabled(locked || state.sites.len() >= 100)
                    .on_click(window.listener_for(&view, |this, _, window, cx| this.landing.update(cx, |state, cx| {
                        if !state.saving && state.sites.len() < 100 { state.add_site(LandingSite { heading: String::new(), subheading: String::new(), url: String::new() }, window, cx); }
                    }))));
            }
            if !state.status.is_empty() { body = body.child(div().id("landing-status").role(Role::Status).aria_label(state.status.clone()).text_color(cx.theme().danger).child(state.status.clone())); }
            dialog.title("Landing apps").w(px(700.)).overlay_closable(false).keyboard(false).close_button(false)
                .child(div().id("landing-editor-viewport").role(Role::Group).aria_label("Landing editor").child(body.vertical_scrollbar(&state.scroll)))
                .footer(div().flex().flex_wrap().justify_end().gap_2()
                    .child(Button::new("landing-close").label("Close").disabled(locked).on_click(window.listener_for(&view, |this, _, window, cx| this.request_landing_action(LandingAction::Close, window, cx))))
                    .child(Button::new("landing-preview").icon(gpui_kit::assets::IconName::ExternalLink).label("Preview").disabled(locked).on_click(window.listener_for(&view, |this, _, window, cx| this.preview_landing(window, cx))))
                    .child(Button::new("landing-deploy").icon(gpui_kit::assets::IconName::CloudUpload).label("Deploy").accessibility_label("Landing deployment settings").tooltip("Deploy the saved landing page").disabled(locked || state.open.is_none() || state.dirty(cx))
                        .on_click(window.listener_for(&view, |this, _, window, cx| {
                            let state = this.landing.read(cx);
                            if !state.saving && !state.dirty(cx) && let Some(file) = state.open.clone() { this.open_landing_deployment(file, window, cx); }
                        })))
                    .child(Button::new("landing-save").primary().icon(gpui_kit::assets::IconName::Save).label("Save").disabled(locked || state.open.is_some() && !state.dirty(cx)).on_click(window.listener_for(&view, |this, _, window, cx| this.save_landing(window, cx)))))
        });
    }

    pub(super) fn choose_landing_app(
        &mut self,
        action: &ChooseLandingApp,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_landing_action(LandingAction::Open(action.path.clone()), window, cx);
    }

    fn request_landing_action(
        &mut self,
        action: LandingAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.landing.read(cx).saving {
            return;
        }
        if self.landing.read(cx).dirty(cx) || matches!(action, LandingAction::Remove) {
            self.landing.update(cx, |state, cx| {
                state.pending = Some(action);
                cx.notify();
            });
        } else {
            self.perform_landing_action(action, window, cx);
        }
    }

    fn perform_landing_action(
        &mut self,
        action: LandingAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            LandingAction::New => self
                .landing
                .update(cx, |state, cx| state.set_app(None, window, cx)),
            LandingAction::Close | LandingAction::Quit => {
                self.landing.update(cx, |state, cx| {
                    let open = state.open.clone();
                    state.set_app(open, window, cx);
                    state.visible = false;
                    cx.notify();
                });
                window.close_dialog(cx);
                if matches!(action, LandingAction::Quit) {
                    self.guard(files::Pending::Close, window, cx);
                }
            }
            LandingAction::Open(path) => self.load_landing_file(path, window, cx),
            LandingAction::Remove => self.remove_landing_file(window, cx),
        }
    }

    fn load_landing_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let Some(catalog_path) = self.landing.read(cx).catalog_path.clone() else {
            return;
        };
        self.landing.update(cx, |state, cx| {
            state.saving = true;
            cx.notify();
        });
        self.task(
            "Opening landing definition...",
            move || {
                Ok((|| -> Result<_> {
                    let file = LandingFile::open(&path)?;
                    let files =
                        catalog_entries(LandingCatalog::update(&catalog_path, &file.path, true)?);
                    Ok((file, files))
                })())
            },
            |this, result, window, cx| {
                this.landing.update(cx, |state, cx| {
                    state.saving = false;
                    match result {
                        Ok((file, files)) => {
                            state.files = files;
                            state.set_app(Some(file), window, cx);
                        }
                        Err(error) => state.status = format!("{error:#}"),
                    }
                    cx.notify();
                });
            },
            window,
            cx,
        );
    }

    fn move_landing_site(&mut self, id: &str, delta: isize, cx: &mut Context<Self>) {
        self.landing.update(cx, |state, cx| {
            if state.saving {
                return;
            }
            if let Some(index) = state.sites.iter().position(|site| site.id == id)
                && let Some(next) = index
                    .checked_add_signed(delta)
                    .filter(|next| *next < state.sites.len())
            {
                state.sites.swap(index, next);
                cx.notify();
            }
        });
    }

    fn save_landing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.landing.read(cx).saving {
            return;
        }
        let state = self.landing.read(cx);
        let app = state.app(cx);
        if let Err(error) = app.validate() {
            self.landing.update(cx, |state, cx| {
                state.status = format!("{error:#}");
                cx.notify();
            });
            return;
        }
        self.persist_landing(app, window, cx);
    }

    fn preview_landing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let app = self.landing.read(cx).app(cx);
        self.landing.update(cx, |state, cx| {
            state.saving = true;
            cx.notify();
        });
        self.task(
            "Preparing landing preview...",
            move || Ok(LandingPreview::start(&app)),
            |this, result, _, cx| {
                this.landing.update(cx, |state, cx| {
                    state.saving = false;
                    match result {
                        Ok(preview) => match open::that_detached(&preview.url) {
                            Ok(()) => {
                                state.preview = Some(preview);
                                state.status.clear();
                            }
                            Err(error) => {
                                state.status = format!("Cannot open landing preview: {error}")
                            }
                        },
                        Err(error) => state.status = format!("{error:#}"),
                    }
                    cx.notify();
                });
            },
            window,
            cx,
        );
    }

    fn persist_landing(&mut self, app: LandingApp, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.landing.read(cx);
        let Some(catalog_path) = state.catalog_path.clone() else {
            return;
        };
        let Some(preferences) = self.preferences_path.clone() else {
            return;
        };
        let open = state.open.clone();
        self.landing.update(cx, |state, cx| {
            state.saving = true;
            cx.notify();
        });
        self.task("Saving landing definition...", move || Ok((|| -> Result<_> {
            let file = if let Some(mut file) = open {
                file.app = app;
                file.save()?;
                file
            } else { LandingFile::create_managed(&preferences, app)? };
            let catalog = LandingCatalog::update(&catalog_path, &file.path, true).map(catalog_entries);
            Ok((file, catalog))
        })()), |this, result, window, cx| {
            let mut pending = None;
            this.landing.update(cx, |state, cx| {
                state.saving = false;
                match result {
                    Ok((file, catalog)) => {
                        state.set_app(Some(file), window, cx);
                        match catalog { Ok(files) => state.files = files, Err(error) => state.status = format!("Definition saved, but the app list could not be updated: {error:#}") }
                        pending = state.pending.take();
                    }
                    Err(error) => state.status = format!("{error:#}"),
                }
                cx.notify();
            });
            if let Some(action) = pending { this.perform_landing_action(action, window, cx); }
        }, window, cx);
    }

    fn remove_landing_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.landing.read(cx);
        let (Some(file), Some(catalog_path), Some(preferences)) = (
            state.open.clone(),
            state.catalog_path.clone(),
            self.preferences_path.clone(),
        ) else {
            return;
        };
        self.landing.update(cx, |state, cx| {
            state.saving = true;
            cx.notify();
        });
        self.task(
            "Removing local landing app...",
            move || {
                Ok((|| -> Result<_> {
                    file.ensure_unchanged()?;
                    let root = file.app.deployment_root(&preferences)?;
                    let path =
                        crate::desktop::deployment::DeployTargets::path_in(&root, &preferences)?;
                    let mut targets = crate::desktop::deployment::DeployTargets::load(&path)?;
                    for id in targets
                        .targets
                        .iter()
                        .map(|target| target.id.clone())
                        .collect::<Vec<_>>()
                    {
                        targets.remove_target(&root, &path, &id)?;
                    }
                    file.remove()?;
                    LandingCatalog::update(&catalog_path, &file.path, false).map(catalog_entries)
                })())
            },
            |this, result, window, cx| {
                this.landing.update(cx, |state, cx| {
                    state.saving = false;
                    match result {
                        Ok(files) => {
                            state.files = files;
                            state.set_app(None, window, cx);
                        }
                        Err(error) => state.status = format!("{error:#}"),
                    }
                    cx.notify();
                });
            },
            window,
            cx,
        );
    }
}

#[cfg(all(test, feature = "gui-tests"))]
mod tests {
    use super::LandingAction;
    use crate::desktop::landing::LandingSite;
    use gpui_kit::test::{TestAppContextExt, TestWindowExt};
    use gpui_kit::{AppContext, TestAppContext};

    #[gpui_kit::test]
    async fn landing_drafts_guard_close_and_keep_remote_site_order(cx: &mut TestAppContext) {
        let root = tempfile::tempdir().unwrap();
        let (handle, view) = super::super::tests::editor(cx, root.path());
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.landing.update(cx, |state, cx| {
                    state.catalog_path = Some(root.path().join("landing-apps.json"));
                    state.set_app(None, window, cx);
                    state.add_site(
                        LandingSite {
                            heading: "Remote".into(),
                            subheading: "**Markdown**".into(),
                            url: "https://someone-else.example/".into(),
                        },
                        window,
                        cx,
                    );
                    state.add_site(
                        LandingSite {
                            heading: "Second".into(),
                            subheading: String::new(),
                            url: "https://another.example/suites/shop".into(),
                        },
                        window,
                        cx,
                    );
                });
                let id = this.landing.read(cx).sites[1].id.clone();
                this.move_landing_site(&id, -1, cx);
                assert_eq!(this.landing.read(cx).app(cx).sites[0].heading, "Second");
                assert!(this.guard_landing_close(window, cx));
                assert!(matches!(
                    this.landing.read(cx).pending,
                    Some(LandingAction::Quit)
                ));
            });
            window.render_frame(cx);
            window.click("landing-keep-editing", cx);
            window.render_frame(cx);
            assert!(view.read(cx).landing.read(cx).dirty(cx));
            window.click("landing-close", cx);
            window.render_frame(cx);
            window.click("landing-discard", cx);
            window.render_frame(cx);
            assert!(!view.read(cx).landing.read(cx).dirty(cx));
            assert!(!root.path().join("landing-apps.json").exists());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    async fn landing_form_scrolls_to_the_last_site(cx: &mut TestAppContext) {
        let root = tempfile::tempdir().unwrap();
        let (handle, view) = super::super::tests::editor(cx, root.path());
        let last = cx
            .update_window(handle.into(), |_, window, cx| {
                view.update(cx, |this, cx| {
                    this.landing.update(cx, |state, cx| {
                        for index in 0..4 {
                            state.add_site(
                                LandingSite {
                                    heading: format!("Remote {index}"),
                                    subheading: "**Details**".into(),
                                    url: "https://external.example/".into(),
                                },
                                window,
                                cx,
                            );
                        }
                    });
                    this.show_landing_manager(window, cx);
                    format!(
                        "landing-site-url-{}",
                        this.landing.read(cx).sites.last().unwrap().id
                    )
                })
            })
            .unwrap();
        super::super::tests::settle_dialog(cx, handle).await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let viewport = window.find("landing-editor-body").bounds();
            assert!(
                window
                    .try_find(last.clone())
                    .is_none_or(|field| field.bounds().bottom() > viewport.bottom())
            );
            window.scroll(
                "landing-editor-body",
                gpui_kit::ScrollDelta::Pixels(gpui_kit::point(
                    gpui_kit::px(0.),
                    gpui_kit::px(-10_000.),
                )),
                cx,
            );
            window.render_frame(cx);
            let field = window.find(last).bounds();
            assert!(
                field.top() >= viewport.top() && field.bottom() <= viewport.bottom(),
                "{field:?} outside {viewport:?}"
            );
        })
        .unwrap();
    }

    #[gpui_kit::test]
    async fn landing_save_manages_its_path_without_a_file_picker(cx: &mut TestAppContext) {
        let root = tempfile::tempdir().unwrap();
        let (handle, view) = super::super::tests::editor(cx, root.path());
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.preferences_path = Some(root.path().join("settings.json"));
                this.open_landing_manager(window, cx);
            });
        })
        .unwrap();
        cx.wait_for(
            handle.into(),
            std::time::Duration::from_secs(3),
            |window, _| window.try_find("landing-save").is_some(),
        )
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            assert!(window.try_find("landing-save-as").is_none());
            assert!(window.try_find("landing-open").is_none());
            window.click("landing-save", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), std::time::Duration::from_secs(3), |_, cx| {
            view.read(cx).landing.read(cx).open.is_some()
        })
        .await;
        cx.update_window(handle.into(), |_, _, cx| {
            let state = view.read(cx).landing.read(cx);
            let file = state.open.as_ref().unwrap();
            assert!(
                file.path
                    .starts_with(root.path().join("landing-apps").canonicalize().unwrap())
            );
            assert!(file.path.is_file());
            assert!(!state.dirty(cx));
        })
        .unwrap();
    }
}
