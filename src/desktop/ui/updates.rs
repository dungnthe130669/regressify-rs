use super::*;
use crate::desktop::updates::{
    CURRENT_VERSION, DownloadControl, PreparedUpdate, Release, UpdateCheck, UpdateFeed,
};
use futures::FutureExt;

pub(super) struct UpdatePanel {
    feed: Option<UpdateFeed>,
    working: bool,
    available: Option<Release>,
    prepared: Option<PreparedUpdate>,
    message: String,
    download: Option<DownloadControl>,
    failed: bool,
    retry_download: bool,
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
            download: None,
            failed: false,
            retry_download: false,
            install_on_close: false,
        }
    }
}

impl UpdatePanel {
    fn begin_download(&mut self, release: &Release) -> DownloadControl {
        let control = DownloadControl::new(release);
        self.working = true;
        self.failed = false;
        self.retry_download = false;
        self.message = control.progress().message();
        self.download = Some(control.clone());
        control
    }

    fn refresh_download(&mut self) -> bool {
        let Some(download) = &self.download else {
            return false;
        };
        let message = if download.is_cancelled() {
            "Cancelling download...".into()
        } else {
            download.progress().message()
        };
        if self.message == message {
            return false;
        }
        self.message = message;
        true
    }

    fn complete_download(&mut self, result: Result<PreparedUpdate>) {
        self.working = false;
        self.failed = false;
        self.retry_download = true;
        if self
            .download
            .take()
            .is_some_and(|control| control.is_cancelled())
        {
            self.message = "Download cancelled. No update was installed.".into();
            return;
        }
        self.message = match result {
            Ok(prepared) => {
                self.retry_download = false;
                let message = format!(
                    "Version {} is verified and ready to install",
                    prepared.version()
                );
                self.prepared = Some(prepared);
                message
            }
            Err(error) => {
                self.failed = true;
                format!("Update download failed: {error:#}. No update was installed.")
            }
        };
    }
}

impl Drop for UpdatePanel {
    fn drop(&mut self) {
        if let Some(download) = &self.download {
            download.cancel();
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
        let title = if update.failed {
            format!("Update to Regressify {version} could not be completed")
        } else if prepared {
            format!("Regressify {version} is ready to install")
        } else {
            format!("New version available: Regressify {version}")
        };
        let detail = if update.working || update.failed || update.retry_download || prepared {
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
                    Icon::new(if update.failed {
                        gpui_kit::assets::IconName::CircleAlert
                    } else {
                        gpui_kit::assets::IconName::Download
                    })
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
                        .child(
                            div()
                                .id("update-banner-detail")
                                .test_support()
                                .text_xs()
                                .whitespace_normal()
                                .child(detail),
                        ),
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
                        } else if update.failed || update.retry_download {
                            "View details"
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
                        .w_full()
                        .whitespace_normal()
                        .child(state.message.clone()),
                )
                .when_some(state.download.as_ref(), |dialog, control| {
                    let progress = control.progress();
                    dialog.child(
                        div()
                            .id("update-download-progress")
                            .test_support()
                            .w_full()
                            .h(px(4.))
                            .bg(cx.theme().border)
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(
                                        (progress.received as f32 / progress.total.max(1) as f32)
                                            .clamp(0., 1.),
                                    ))
                                    .bg(cx.theme().primary),
                            ),
                    )
                })
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
                        .when(state.failed, |footer| {
                            footer.child(
                                Button::new("update-copy-details")
                                    .icon(gpui_kit::assets::IconName::Copy)
                                    .label("Copy details")
                                    .on_click(window.listener_for(&view, |this, _, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            this.updates.read(cx).message.clone(),
                                        ));
                                    })),
                            )
                        })
                        .when_some(state.download.as_ref(), |footer, control| {
                            footer.child(
                                Button::new("update-cancel-download")
                                    .icon(gpui_kit::assets::IconName::X)
                                    .label("Cancel download")
                                    .disabled(control.is_cancelled())
                                    .on_click(window.listener_for(&view, |this, _, _, cx| {
                                        this.updates.update(cx, |state, cx| {
                                            if let Some(download) = &state.download {
                                                download.cancel();
                                                state.refresh_download();
                                                cx.notify();
                                            }
                                        });
                                        cx.notify();
                                    })),
                            )
                        })
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
                                    .label(if state.retry_download {
                                        "Retry download"
                                    } else {
                                        "Download update"
                                    })
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
            state.failed = false;
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
                    state.message = match result {
                        Ok(UpdateCheck::Available(release)) => {
                            let message = format!("Version {} is available", release.version);
                            state.available = Some(release);
                            state.retry_download = false;
                            message
                        }
                        Ok(UpdateCheck::Current) => {
                            state.available = None;
                            "You have the latest version".into()
                        }
                        Ok(UpdateCheck::Unpublished) => {
                            state.available = None;
                            "No releases have been published yet".into()
                        }
                        Err(error) => {
                            state.failed = true;
                            format!("Update check failed: {error:#}")
                        }
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
        let control = self.updates.update(cx, |state, cx| {
            let control = state.begin_download(&release);
            cx.notify();
            control
        });
        let worker_control = control.clone();
        let task = cx.background_executor().spawn(async move {
            PreparedUpdate::download_with_control(
                release,
                &cache.ok_or_else(|| anyhow!("Cannot locate the update cache"))?,
                worker_control,
            )
        });
        self.observe_update_download(task, control, window, cx);
    }

    fn observe_update_download(
        &mut self,
        task: Task<Result<PreparedUpdate>>,
        control: DownloadControl,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |view, cx| {
            let task = task.fuse();
            futures::pin_mut!(task);
            loop {
                let timer = cx
                    .background_executor()
                    .timer(std::time::Duration::from_millis(250))
                    .fuse();
                futures::pin_mut!(timer);
                futures::select_biased! {
                    result = task => {
                        let _ = view.update_in(cx, |this, window, cx| {
                            this.updates.update(cx, |state, cx| {
                                state.complete_download(result);
                                cx.notify();
                            });
                            window.refresh();
                            cx.notify();
                        });
                        break;
                    }
                    _ = timer => {
                        if view.update_in(cx, |this, window, cx| {
                            let changed = this.updates.update(cx, |state, cx| {
                                let changed = state.refresh_download();
                                if changed {
                                    cx.notify();
                                }
                                changed
                            });
                            if changed {
                                window.refresh();
                                cx.notify();
                            }
                        }).is_err() {
                            control.cancel();
                            break;
                        }
                    }
                }
            }
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
                    "size": 100 * 1024 * 1024,
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
    async fn update_progress_refreshes_during_transfer_and_cancel_wins_completion(
        cx: &mut TestAppContext,
    ) {
        use crate::desktop::updates::DownloadPhase;

        let directory = tempfile::tempdir().unwrap();
        let (handle, view) = editor(cx, directory.path());
        let (finished, completion) = futures::channel::oneshot::channel();
        let control = cx
            .update_window(handle.into(), |_, window, cx| {
                view.update(cx, |this, cx| {
                    let release = available_release();
                    let control = this.updates.update(cx, |state, cx| {
                        state.available = Some(release.clone());
                        let control = state.begin_download(&release);
                        cx.notify();
                        control
                    });
                    let task = cx.background_executor().spawn(async move {
                        completion
                            .await
                            .unwrap_or_else(|_| Err(anyhow::anyhow!("worker stopped")))
                    });
                    this.observe_update_download(task, control.clone(), window, cx);
                    this.open_updates(window, cx);
                    control
                })
            })
            .unwrap();
        settle_dialog(cx, handle).await;
        control.set_progress_for_test(DownloadPhase::Downloading, 50 * 1024 * 1024);
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            view.read(cx).updates.read(cx).message.contains("50%")
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("update-download-progress").visible());
            assert!(
                view.read(cx)
                    .updates
                    .read(cx)
                    .message
                    .contains("50.0 / 100.0 MiB")
            );
            window.click("update-close", cx);
            assert!(view.read(cx).updates.read(cx).working);
            assert!(!control.is_cancelled());
        })
        .unwrap();
        for (phase, message) in [
            (DownloadPhase::Verifying, "Verifying installer SHA-256"),
            (DownloadPhase::Saving, "Saving verified installer to disk"),
        ] {
            control.set_progress_for_test(phase, 100 * 1024 * 1024);
            cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
                view.read(cx).updates.read(cx).message.contains(message)
            })
            .await;
        }
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("update-banner-detail").visible());
            window.click("update-banner-action", cx);
        })
        .unwrap();
        settle_dialog(cx, handle).await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("update-cancel-download", cx);
            assert!(control.is_cancelled());
        })
        .unwrap();
        assert!(
            finished
                .send(Ok(PreparedUpdate::missing_installer_for_test(
                    available_release()
                )
                .unwrap()))
                .is_ok()
        );
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            !view.read(cx).updates.read(cx).working
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            let state = view.read(cx).updates.read(cx);
            assert!(state.message.contains("Download cancelled"));
            assert!(!state.failed);
            assert!(state.prepared.is_none());
            assert!(!state.install_on_close);
            window.click("update-close", cx);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    async fn update_download_cancel_retry_and_errors_preserve_the_draft(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let (handle, view) = editor(cx, directory.path());
        let control = cx
            .update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                window.click("duplicate-item", cx);
                view.update(cx, |this, cx| {
                    this.preferences_path = None;
                    let release = available_release();
                    let control = this.updates.update(cx, |state, cx| {
                        state.available = Some(release.clone());
                        let control = state.begin_download(&release);
                        cx.notify();
                        control
                    });
                    this.open_updates(window, cx);
                    control
                })
            })
            .unwrap();
        settle_dialog(cx, handle).await;
        cx.update_window(handle.into(), |_, window, cx| {
            assert!(window.find("update-download-progress").visible());
            assert!(
                view.read(cx)
                    .updates
                    .read(cx)
                    .message
                    .contains("Connecting")
            );
            window.click("update-cancel-download", cx);
            assert!(control.is_cancelled());
            assert!(view.read(cx).updates.read(cx).working);
            assert!(
                view.read(cx)
                    .updates
                    .read(cx)
                    .message
                    .contains("Cancelling")
            );
            view.update(cx, |this, cx| {
                this.updates.update(cx, |state, cx| {
                    state.complete_download(Err(anyhow::anyhow!("cancelled")));
                    assert!(!state.failed);
                    assert!(state.retry_download);
                    assert!(state.download.is_none());
                    cx.notify();
                });
                cx.notify();
            });
            window.render_frame(cx);
            assert!(window.try_find("update-cancel-download").is_none());
            window.click("update-download", cx);
            assert!(view.read(cx).updates.read(cx).working);
            assert!(!view.read(cx).updates.read(cx).failed);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            !view.read(cx).updates.read(cx).working
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let state = view.read(cx).updates.read(cx);
            assert!(state.failed);
            assert!(state.retry_download);
            assert!(state.message.contains("Cannot locate the update cache"));
            assert!(state.message.contains("No update was installed"));
            assert!(state.prepared.is_none());
            assert!(window.find("update-copy-details").visible());
            window.click("update-close", cx);
            window.render_frame(cx);
            assert!(window.find("update-banner-detail").visible());
            assert!(view.read(cx).unsaved());
            assert!(!view.read(cx).updates.read(cx).install_on_close);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    async fn update_failure_details_fit_compact_and_large_windows(cx: &mut TestAppContext) {
        for (width, height) in [(1000., 640.), (1600., 1000.)] {
            for dark in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let (handle, view) =
                    editor_at_size(cx, directory.path(), size(px(width), px(height)));
                cx.update_window(handle.into(), |_, window, cx| {
                    design::apply_theme(dark, Some(window), cx);
                    view.update(cx, |this, cx| {
                        this.updates.update(cx, |state, cx| {
                            let release = available_release();
                            state.available = Some(release.clone());
                            state.begin_download(&release);
                            state.complete_download(Err(anyhow::anyhow!(
                                "Download stalled: no data received for 30 seconds (12345678 of 987654321 bytes). Check your connection, VPN or proxy, then retry"
                            )));
                            cx.notify();
                        });
                        this.open_updates(window, cx);
                        cx.notify();
                    });
                }).unwrap();
                settle_dialog(cx, handle).await;
                cx.update_window(handle.into(), |_, window, cx| {
                    let banner = window.find("update-banner").bounds();
                    let detail = window.find("update-banner-detail").bounds();
                    let action = window.find("update-banner-action").bounds();
                    assert!(detail.origin.x + detail.size.width <= action.origin.x);
                    assert!(
                        detail.origin.y + detail.size.height
                            <= banner.origin.y + banner.size.height
                    );
                    for id in [
                        "update-status",
                        "update-download",
                        "update-copy-details",
                        "update-close",
                    ] {
                        let bounds = window.find(id).bounds();
                        assert!(bounds.origin.x >= px(0.));
                        assert!(bounds.origin.x + bounds.size.width <= px(width));
                        assert!(bounds.origin.y + bounds.size.height <= px(height));
                    }
                    window.click("update-close", cx);
                })
                .unwrap();
            }
        }
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
