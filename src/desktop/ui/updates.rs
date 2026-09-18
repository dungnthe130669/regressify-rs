use super::*;
use crate::desktop::updates::{CURRENT_VERSION, PreparedUpdate, Release, UpdateCheck, UpdateFeed};

pub(super) struct UpdatePanel {
    feed: Option<UpdateFeed>,
    working: bool,
    available: Option<Release>,
    prepared: Option<PreparedUpdate>,
    message: String,
    pub(super) install_on_close: bool,
}

impl Default for UpdatePanel {
    fn default() -> Self {
        let feed = UpdateFeed::configured();
        Self {
            message: if feed.is_some() {
                "Ready to check for updates".into()
            } else {
                "Updates are unavailable in this build".into()
            },
            feed,
            working: false,
            available: None,
            prepared: None,
            install_on_close: false,
        }
    }
}

impl Workbench {
    pub(super) fn render_update_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let update = self.updates.read(cx);
        let available = update.available.is_some() || update.prepared.is_some();
        let label = if available {
            "Update available"
        } else {
            "Software updates"
        };
        Button::new("software-updates")
            .ghost()
            .xsmall()
            .icon(if available {
                gpui_kit::assets::IconName::Download
            } else {
                gpui_kit::assets::IconName::RefreshCw
            })
            .accessibility_label(label)
            .tooltip(label)
            .disabled(self.busy)
            .on_click(cx.listener(|this, _, window, cx| this.open_updates(window, cx)))
    }

    pub(super) fn open_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let updates = self.updates.clone();
        let install_blocked = self.busy || self.runs.read(cx).running();
        window.open_dialog(cx, move |dialog, window, cx| {
            let state = updates.read(cx);
            let working = state.working;
            let available = state.available.is_some();
            let prepared = state.prepared.is_some();
            dialog
                .title("Software updates")
                .child(format!("Regressify Desktop {CURRENT_VERSION}"))
                .child(
                    div()
                        .id("update-status")
                        .role(Role::Status)
                        .aria_label(state.message.clone())
                        .test_support()
                        .text_sm()
                        .child(state.message.clone()),
                )
                .footer(
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("update-close")
                                .label("Close")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("update-check")
                                .icon(gpui_kit::assets::IconName::RefreshCw)
                                .label("Check for updates")
                                .disabled(state.feed.is_none() || working || prepared)
                                .on_click(window.listener_for(&view, |this, _, window, cx| {
                                    this.check_updates(window, cx);
                                })),
                        )
                        .when(available && !prepared, |footer| {
                            footer.child(
                                Button::new("update-download")
                                    .primary()
                                    .icon(gpui_kit::assets::IconName::Download)
                                    .label("Download update")
                                    .disabled(working)
                                    .on_click(window.listener_for(&view, |this, _, window, cx| {
                                        this.download_update(window, cx);
                                    })),
                            )
                        })
                        .when(prepared, |footer| {
                            footer.child(
                                Button::new("update-install")
                                    .primary()
                                    .icon(gpui_kit::assets::IconName::RotateCw)
                                    .label("Install and restart")
                                    .disabled(working || install_blocked)
                                    .on_click(window.listener_for(&view, |this, _, window, cx| {
                                        this.request_update_install(window, cx);
                                    })),
                            )
                        }),
                )
        });
    }

    pub(super) fn check_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.updates.read(cx);
        if state.working || state.prepared.is_some() {
            return;
        }
        let Some(feed) = state.feed.clone() else {
            return;
        };
        self.updates.update(cx, |state, cx| {
            state.working = true;
            state.message = "Checking for updates...".into();
            cx.notify();
        });
        let task = cx
            .background_executor()
            .spawn(async move { feed.check(CURRENT_VERSION.parse()?) });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |this, _, cx| {
                this.updates.update(cx, |state, cx| {
                    state.working = false;
                    state.available = None;
                    state.message = match result {
                        Ok(UpdateCheck::Available(release)) => {
                            let message = format!("Version {} is available", release.version);
                            state.available = Some(release);
                            message
                        }
                        Ok(UpdateCheck::Current) => "You have the latest version".into(),
                        Ok(UpdateCheck::Unpublished) => {
                            "No releases have been published yet".into()
                        }
                        Err(error) => format!("Update check failed: {error:#}"),
                    };
                    cx.notify();
                });
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn download_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.updates.read(cx);
        if state.working || state.prepared.is_some() {
            return;
        }
        let Some(release) = state.available.clone() else {
            return;
        };
        let cache = self
            .preferences_path
            .as_ref()
            .map(|path| path.with_file_name("updates"));
        self.updates.update(cx, |state, cx| {
            state.working = true;
            state.message = format!("Downloading and verifying version {}...", release.version);
            cx.notify();
        });
        let task = cx.background_executor().spawn(async move {
            PreparedUpdate::download(
                release,
                &cache.ok_or_else(|| anyhow!("Cannot locate the update cache"))?,
            )
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |this, _, cx| {
                this.updates.update(cx, |state, cx| {
                    state.working = false;
                    state.message = match result {
                        Ok(prepared) => {
                            let message = format!(
                                "Version {} is verified and ready to install",
                                prepared.version()
                            );
                            state.prepared = Some(prepared);
                            message
                        }
                        Err(error) => format!("Update download failed: {error:#}"),
                    };
                    cx.notify();
                });
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn request_update_install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.runs.read(cx).running() || self.updates.read(cx).prepared.is_none() {
            return;
        }
        if let Err(error) = self.flush_drafts(window, cx) {
            self.status = format!("Cannot install update: {error:#}");
            self.updates.update(cx, |state, cx| {
                state.message = format!("Cannot install update: {error:#}");
                cx.notify();
            });
            cx.notify();
            return;
        }
        window.close_dialog(cx);
        self.guard(files::Pending::Update, window, cx);
    }

    pub(super) fn finish_update(&mut self, cx: &mut Context<Self>) {
        if self.busy
            || self.unsaved()
            || self.runs.read(cx).running()
            || self.updates.read(cx).prepared.is_none()
        {
            return;
        }
        self.updates
            .update(cx, |state, _| state.install_on_close = true);
        self.finish_close(cx);
    }

    pub(super) fn quit_or_install(&mut self, cx: &mut Context<Self>) {
        if self.updates.read(cx).install_on_close {
            let workspace = self
                .session
                .workspace
                .as_ref()
                .map(|workspace| workspace.root.clone());
            let result = self.updates.update(cx, |state, cx| {
                state.install_on_close = false;
                let result = state
                    .prepared
                    .as_mut()
                    .ok_or_else(|| anyhow!("No verified update is ready"))?
                    .launch(workspace.as_deref());
                if let Err(error) = &result {
                    state.message = format!("Cannot start installer: {error:#}");
                }
                cx.notify();
                result
            });
            if let Err(error) = result {
                self.status = format!("Cannot install update: {error:#}");
                cx.notify();
                return;
            }
        }
        cx.quit();
    }
}
