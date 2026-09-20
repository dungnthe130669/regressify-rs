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
            .when(available, |button| button.primary())
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

    pub(super) fn render_update_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let update = self.updates.read(cx);
        let version = update
            .prepared
            .as_ref()
            .map(PreparedUpdate::version)
            .or_else(|| update.available.as_ref().map(|release| release.version))?;
        let prepared = update.prepared.is_some();
        let title = if prepared {
            format!("Regressify {version} is ready to install")
        } else {
            format!("New version available: Regressify {version}")
        };
        let detail = if update.working {
            update.message.clone()
        } else {
            format!("Installed version: {CURRENT_VERSION}")
        };
        Some(
            div()
                .id("update-banner")
                .role(Role::Status)
                .aria_label(format!("{title}. {detail}"))
                .test_support()
                .flex()
                .items_center()
                .gap_3()
                .w_full()
                .min_h(px(64.))
                .flex_shrink_0()
                .px_4()
                .py_3()
                .bg(cx.theme().primary)
                .text_color(cx.theme().primary_foreground)
                .child(
                    Icon::new(gpui_kit::assets::IconName::Download)
                        .size_5()
                        .flex_shrink_0(),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(
                            div()
                                .id("update-banner-title")
                                .test_support()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        )
                        .child(div().text_xs().child(detail)),
                )
                .child(
                    Button::new("update-banner-action")
                        .small()
                        .flex_shrink_0()
                        .icon(if prepared {
                            gpui_kit::assets::IconName::RotateCw
                        } else {
                            gpui_kit::assets::IconName::Download
                        })
                        .label(if update.working {
                            "View progress"
                        } else if prepared {
                            "Install update"
                        } else {
                            "Update now"
                        })
                        .disabled(self.busy)
                        .on_click(cx.listener(|this, _, window, cx| this.open_updates(window, cx))),
                )
                .into_any_element(),
        )
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
        if self.session.open.is_some()
            && let Err(error) = self.flush_drafts(window, cx)
        {
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

#[cfg(all(test, feature = "gui-tests"))]
mod tests {
    use std::time::Duration;

    use gpui_kit::test::{TestAppContextExt, TestWindowExt};
    use gpui_kit::{AppContext, TestAppContext, px, size};

    use super::{PreparedUpdate, UpdateFeed};
    use crate::desktop::ui::{
        design,
        tests::{editor, editor_at_size, settle_dialog},
    };
    use crate::desktop::updates::Release;

    fn available_release() -> Release {
        UpdateFeed::new("regressify")
            .unwrap()
            .parse_manifest(
                &serde_json::json!({
                    "version": "9999.9999.9999.9999",
                    "installer": "https://drive.tuyen.blog/downloads/regressify/Regressify-Desktop-Setup-9999.9999.9999.9999-x64.exe",
                    "sha256": "a".repeat(64),
                    "size": 1,
                })
                .to_string(),
            )
            .unwrap()
    }

    #[gpui_kit::test]
    async fn update_install_does_not_require_an_open_document_or_workspace(
        cx: &mut TestAppContext,
    ) {
        for workspace_open in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let (handle, view) = editor(cx, directory.path());
            cx.update_window(handle.into(), |_, window, cx| {
                view.update(cx, |this, cx| {
                    if workspace_open {
                        this.session.open = None;
                        this.sync_editor(window, cx);
                    } else {
                        this.close_workspace(window, cx);
                    }
                    this.updates.update(cx, |state, cx| {
                        state.prepared = Some(
                            PreparedUpdate::missing_installer_for_test(available_release())
                                .unwrap(),
                        );
                        cx.notify();
                    });
                    this.open_updates(window, cx);
                });
            })
            .unwrap();
            cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
                !view.read(cx).preferences_saving
            })
            .await;
            settle_dialog(cx, handle).await;
            cx.update_window(handle.into(), |_, window, cx| {
                assert_eq!(view.read(cx).session.workspace.is_some(), workspace_open);
                assert!(view.read(cx).session.open.is_none());
                assert!(!view.read(cx).unsaved());
                assert!(window.find("update-install").visible());
                window.click("update-install", cx);
                let state = view.read(cx).updates.read(cx);
                assert!(
                    state.message.starts_with("Cannot start installer:"),
                    "{}",
                    state.message
                );
                assert!(state.prepared.is_some());
                assert!(!state.install_on_close);
            })
            .unwrap();
        }
    }

    #[gpui_kit::test]
    async fn update_install_flushes_pending_edits_and_preserves_the_cancel_guard(
        cx: &mut TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let (handle, view) = editor(cx, directory.path());
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.updates.update(cx, |state, cx| {
                    state.prepared = Some(
                        PreparedUpdate::missing_installer_for_test(available_release()).unwrap(),
                    );
                    cx.notify();
                });
                this.tab = 1;
                let draft = format!("{}\n# Pending update draft\n", this.source.read(cx).value());
                this.source
                    .update(cx, |state, cx| state.set_value(draft.clone(), window, cx));
                this.request_update_install(window, cx);
                assert_eq!(this.session.open.as_ref().unwrap().document.source(), draft);
            });
        })
        .unwrap();
        settle_dialog(cx, handle).await;
        cx.update_window(handle.into(), |_, window, cx| {
            assert!(window.find("guard-save").visible());
            assert!(window.find("guard-discard").visible());
            window.click("guard-cancel", cx);
            assert!(view.read(cx).unsaved());
            let state = view.read(cx).updates.read(cx);
            assert!(state.prepared.is_some());
            assert!(!state.install_on_close);
            assert!(!state.message.contains("Cannot start installer:"));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    async fn update_banner_opens_the_dialog_and_preserves_the_draft(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let (handle, view) = editor(cx, directory.path());
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.updates.update(cx, |state, cx| {
                    state.available = Some(available_release());
                    cx.notify();
                });
                cx.notify();
            });
            window.render_frame(cx);
            window.click("duplicate-item", cx);
            window.click("update-banner-action", cx);
        })
        .unwrap();
        settle_dialog(cx, handle).await;
        cx.update_window(handle.into(), |_, window, cx| {
            assert!(window.find("update-download").visible());
            assert!(view.read(cx).unsaved());
            assert!(!view.read(cx).updates.read(cx).working);
            assert!(!view.read(cx).updates.read(cx).install_on_close);
            window.click("update-close", cx);
            window.render_frame(cx);
            assert!(window.find("update-banner").visible());
            view.update(cx, |this, cx| {
                this.updates.update(cx, |state, cx| {
                    state.working = true;
                    state.message = "Downloading and verifying the update...".into();
                    cx.notify();
                });
                cx.notify();
            });
            window.render_frame(cx);
            assert!(window.find("update-banner").visible());
            window.click("update-banner-action", cx);
        })
        .unwrap();
        settle_dialog(cx, handle).await;
        cx.update_window(handle.into(), |_, window, cx| {
            assert!(window.find("update-status").visible());
            window.click("update-download", cx);
            assert!(view.read(cx).updates.read(cx).working);
            assert!(view.read(cx).unsaved());
            assert!(!view.read(cx).updates.read(cx).install_on_close);
            window.click("update-close", cx);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn update_banner_stays_visible_across_views_themes_and_sizes(cx: &mut TestAppContext) {
        for (width, height) in [(1000., 640.), (1600., 1000.)] {
            let directory = tempfile::tempdir().unwrap();
            let (handle, view) = editor_at_size(cx, directory.path(), size(px(width), px(height)));
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                assert!(window.try_find("update-banner").is_none());
                view.update(cx, |this, cx| {
                    this.updates.update(cx, |state, cx| {
                        state.available = Some(available_release());
                        cx.notify();
                    });
                    cx.notify();
                });
                for dark in [false, true] {
                    design::apply_theme(dark, Some(window), cx);
                    for main_tab in 0..4 {
                        view.update(cx, |this, cx| {
                            this.main_tab = main_tab;
                            cx.notify();
                        });
                        window.render_frame(cx);
                        assert!(window.find("update-banner").visible());
                        assert!(window.find("update-banner-action").visible());
                        let banner = window.find("update-banner").bounds();
                        let title = window.find("update-banner-title").bounds();
                        let action = window.find("update-banner-action").bounds();
                        assert_eq!(banner.size.width, px(width));
                        assert!(banner.origin.y >= px(38.));
                        assert!(banner.size.height >= px(64.));
                        assert!(banner.size.height <= px(100.));
                        assert!(title.origin.x + title.size.width <= action.origin.x);
                        assert!(action.origin.x + action.size.width <= px(width));
                        assert!(action.origin.y >= banner.origin.y);
                        assert!(
                            action.origin.y + action.size.height
                                <= banner.origin.y + banner.size.height
                        );
                    }
                }
                view.update(cx, |this, cx| {
                    this.updates.update(cx, |state, cx| {
                        state.available = None;
                        cx.notify();
                    });
                    cx.notify();
                });
                window.render_frame(cx);
                assert!(window.try_find("update-banner").is_none());
            })
            .unwrap();
        }
    }
}
