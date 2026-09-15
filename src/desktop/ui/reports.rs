//! Report browsing and image loading are isolated from editable test documents.

use std::sync::Arc;

use super::*;
use crate::desktop::results::{Catalog, Outcome, ReportEntry};

const PAGE_SIZE: usize = 50;

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
struct SelectReport {
    index: usize,
}

struct Preview {
    label: &'static str,
    result: std::result::Result<LoadedImage, String>,
}

struct LoadedImage {
    image: Arc<Image>,
    width: usize,
    height: usize,
    path: PathBuf,
}

pub(super) struct ResultsPanel {
    root: Option<PathBuf>,
    catalog: Catalog,
    report: usize,
    capture: Option<usize>,
    filter: usize,
    page: usize,
    image_mode: usize,
    zoom: f32,
    fit: bool,
    search: Entity<InputState>,
    _subscription: Subscription,
    focus: FocusHandle,
    previews: Vec<Preview>,
    status: String,
    loading: bool,
    image_loading: bool,
    generation: u64,
}

impl ResultsPanel {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Filter captures"));
        let subscription = cx.subscribe(&search, |this: &mut Self, _, _: &InputEvent, cx| {
            this.page = 0;
            cx.notify();
        });
        Self {
            root: None,
            catalog: Catalog::default(),
            report: 0,
            capture: None,
            filter: 0,
            page: 0,
            image_mode: 1,
            zoom: 1.0,
            fit: true,
            search,
            _subscription: subscription,
            focus: cx.focus_handle(),
            previews: Vec::new(),
            status: "No reports loaded".into(),
            loading: false,
            image_loading: false,
            generation: 0,
        }
    }

    pub(super) fn refresh(&mut self, root: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        self.root = Some(root.clone());
        self.loading = true;
        self.image_loading = false;
        self.previews.clear();
        self.status = "Loading reports...".into();
        let task = cx
            .background_executor()
            .spawn(async move { Catalog::load(&root) });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |this, window, cx| {
                if this.generation != generation {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(catalog) => {
                        this.catalog = catalog;
                        this.report = 0;
                        this.capture = None;
                        this.page = 0;
                        this.status = format!(
                            "{} reports, {} warnings",
                            this.catalog.reports.len(),
                            this.catalog.warnings.len()
                        );
                        if this
                            .catalog
                            .reports
                            .first()
                            .is_some_and(|report| !report.report.tests.is_empty())
                        {
                            this.select_capture(0, window, cx);
                        }
                    }
                    Err(error) => {
                        this.catalog = Catalog::default();
                        this.status = format!("{error:#}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn selected_report(&self) -> Option<&ReportEntry> {
        self.catalog.reports.get(self.report)
    }

    fn select_capture(&mut self, capture: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(report) = self.selected_report().cloned() else {
            return;
        };
        let Some(test) = report.report.tests.get(capture).cloned() else {
            return;
        };
        let Some(root) = self.root.clone() else {
            return;
        };
        self.capture = Some(capture);
        self.previews.clear();
        self.image_loading = true;
        self.generation += 1;
        let generation = self.generation;
        let task = cx.background_executor().spawn(async move {
            [
                ("Reference", test.pair.reference),
                ("Current", test.pair.test),
                ("Diff", test.pair.diff_image),
            ]
            .into_iter()
            .map(|(label, path)| {
                let result = (|| -> Result<LoadedImage> {
                    let path = path.ok_or_else(|| anyhow!("No {} image", label.to_lowercase()))?;
                    let image = report.image(&root, &path)?;
                    let bytes = std::fs::read(&image.path)?;
                    Ok(LoadedImage {
                        image: Arc::new(Image::from_bytes(ImageFormat::Png, bytes)),
                        width: image.width,
                        height: image.height,
                        path: image.path,
                    })
                })()
                .map_err(|error| format!("{error:#}"));
                Preview { label, result }
            })
            .collect::<Vec<_>>()
        });
        cx.spawn_in(window, async move |view, cx| {
            let previews = task.await;
            let _ = view.update_in(cx, |this, _, cx| {
                if this.generation == generation {
                    this.previews = previews;
                    this.image_loading = false;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn filtered(&self, cx: &App) -> Vec<usize> {
        let query = self.search.read(cx).value().to_lowercase();
        self.selected_report()
            .map(|report| {
                report
                    .report
                    .tests
                    .iter()
                    .enumerate()
                    .filter(|(_, test)| {
                        let status = match self.filter {
                            1 => test.outcome() != Outcome::Passed,
                            2 => test.outcome() == Outcome::Passed,
                            _ => true,
                        };
                        status
                            && (query.is_empty()
                                || test.pair.label.to_lowercase().contains(&query)
                                || test
                                    .pair
                                    .engine_error_msg
                                    .as_deref()
                                    .unwrap_or_default()
                                    .to_lowercase()
                                    .contains(&query))
                    })
                    .map(|(index, _)| index)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn open_external(
        &mut self,
        target: Result<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = match target {
            Ok(path) => path,
            Err(error) => {
                self.status = format!("{error:#}");
                cx.notify();
                return;
            }
        };
        let task = cx
            .background_executor()
            .spawn(async move { open::that(target) });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |this, _, cx| {
                if let Err(error) = result {
                    this.status = format!("Cannot open artifact: {error}");
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_report(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = (|| -> Result<PathBuf> {
            let root = self
                .root
                .as_ref()
                .ok_or_else(|| anyhow!("No output directory selected"))?;
            let summary = root.join("index.html");
            if summary.exists() {
                let path = summary.canonicalize()?;
                anyhow::ensure!(
                    path.starts_with(root.canonicalize()?),
                    "Summary report escapes the output directory"
                );
                Ok(path)
            } else {
                self.selected_report()
                    .ok_or_else(|| anyhow!("No report selected"))?
                    .html_report(root)
            }
        })();
        self.open_external(target, window, cx);
    }

    fn render_captures(&self, cx: &mut Context<Self>) -> AnyElement {
        let filtered = self.filtered(cx);
        let pages = filtered.len().div_ceil(PAGE_SIZE).max(1);
        let mut list = div()
            .id("capture-list")
            .flex()
            .flex_col()
            .gap_1()
            .p_3()
            .size_full()
            .child(Input::new(&self.search).id("capture-search").small())
            .child(
                TabBar::new("result-filter")
                    .segmented()
                    .selected_index(self.filter)
                    .on_click(cx.listener(|this, index, _, cx| {
                        this.filter = *index;
                        this.page = 0;
                        cx.notify();
                    }))
                    .child(Tab::new().label("All"))
                    .child(Tab::new().label("Failed"))
                    .child(Tab::new().label("Passed")),
            );
        let mut rows = div()
            .id("capture-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .gap_1();
        if let Some(report) = self.selected_report() {
            for index in filtered
                .into_iter()
                .skip(self.page * PAGE_SIZE)
                .take(PAGE_SIZE)
            {
                let test = &report.report.tests[index];
                let outcome = test.outcome();
                let color = match outcome {
                    Outcome::Passed => cx.theme().success,
                    Outcome::Mismatch => cx.theme().warning,
                    _ => cx.theme().danger,
                };
                rows = rows.child(
                    Button::new(("capture", index))
                        .ghost()
                        .w_full()
                        .justify_start()
                        .selected(self.capture == Some(index))
                        .label(format!("{}  {}", outcome.label(), test.pair.label))
                        .tooltip(
                            test.pair
                                .engine_error_msg
                                .clone()
                                .unwrap_or_else(|| test.pair.label.clone()),
                        )
                        .text_color(color)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.select_capture(index, window, cx)
                        })),
                );
            }
        }
        list = list.child(rows).child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    Button::new("previous-captures")
                        .ghost()
                        .icon(IconName::ArrowLeft)
                        .tooltip("Previous captures")
                        .disabled(self.page == 0)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.page = this.page.saturating_sub(1);
                            cx.notify();
                        })),
                )
                .child(format!("{} / {pages}", self.page + 1))
                .child(
                    Button::new("next-captures")
                        .ghost()
                        .icon(IconName::ArrowRight)
                        .tooltip("Next captures")
                        .disabled(self.page + 1 >= pages)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this.page + 1 < pages {
                                this.page += 1;
                            }
                            cx.notify();
                        })),
                ),
        );
        list.into_any_element()
    }

    fn render_preview(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(preview) = self.previews.get(index) else {
            return div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .p_4()
                .child(if self.image_loading {
                    "Loading image..."
                } else {
                    "No capture selected"
                })
                .into_any_element();
        };
        let content = match &preview.result {
            Ok(image) => {
                let display = img(image.image.clone()).object_fit(ObjectFit::Contain);
                let display = if self.fit {
                    display
                        .w_full()
                        .aspect_ratio(image.width as f32 / image.height as f32)
                } else {
                    display
                        .w(px(image.width as f32 * self.zoom))
                        .h(px(image.height as f32 * self.zoom))
                };
                let path = image.path.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .size_full()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .px_3()
                            .py_2()
                            .child(
                                div()
                                    .id(("image-details", index))
                                    .role(Role::Status)
                                    .aria_label(format!(
                                        "{} {} x {}",
                                        preview.label, image.width, image.height
                                    ))
                                    .test_support()
                                    .child(format!(
                                        "{}  {} x {}",
                                        preview.label, image.width, image.height
                                    )),
                            )
                            .child(
                                Button::new(("open-image", index))
                                    .ghost()
                                    .xsmall()
                                    .icon(gpui_kit::assets::IconName::ExternalLink)
                                    .tooltip("Open image externally")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_external(Ok(path.clone()), window, cx)
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id(("image-scroll", index))
                            .flex_1()
                            .min_h_0()
                            .overflow_scroll()
                            .bg(cx.theme().muted)
                            .child(display),
                    )
                    .into_any_element()
            }
            Err(error) => div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .p_4()
                .size_full()
                .child(preview.label)
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(error.clone()),
                )
                .into_any_element(),
        };
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(content)
            .into_any_element()
    }

    fn render_comparison(&self, cx: &mut Context<Self>) -> AnyElement {
        let test = self.selected_report().and_then(|report| {
            self.capture
                .and_then(|index| report.report.tests.get(index))
        });
        let title = test
            .map(|test| test.pair.label.clone())
            .unwrap_or_else(|| "Comparison".into());
        let previews = if self.image_mode == 3 {
            div()
                .flex()
                .size_full()
                .gap_2()
                .child(self.render_preview(0, cx))
                .child(self.render_preview(1, cx))
                .into_any_element()
        } else {
            self.render_preview(self.image_mode, cx)
        };
        let mismatch = test
            .and_then(|test| test.pair.diff.as_ref())
            .map(|diff| {
                if diff.mis_match_percentage.is_empty() {
                    diff.raw_mis_match_percentage
                        .map(|percentage| format!("{percentage:.3}%"))
                        .unwrap_or_default()
                } else {
                    format!("{}%", diff.mis_match_percentage)
                }
            })
            .unwrap_or_default();
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .child(
                div()
                    .px_4()
                    .py_2()
                    .font_weight(FontWeight::MEDIUM)
                    .child(format!("{title}   {mismatch}")),
            )
            .child(
                TabBar::new("image-modes")
                    .underline()
                    .selected_index(self.image_mode)
                    .on_click(cx.listener(|this, index, _, cx| {
                        this.image_mode = *index;
                        cx.notify();
                    }))
                    .child(Tab::new().label("Reference"))
                    .child(Tab::new().label("Current"))
                    .child(Tab::new().label("Diff"))
                    .child(Tab::new().label("Side by side")),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .child(
                        Checkbox::new("fit-image")
                            .label("Fit width")
                            .checked(self.fit)
                            .on_change(cx.listener(|this, value, _, cx| {
                                this.fit = *value;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("zoom-out")
                            .ghost()
                            .icon(gpui_kit::assets::IconName::ZoomOut)
                            .tooltip("Zoom out")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.fit = false;
                                this.zoom = (this.zoom / 1.25).max(0.1);
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("zoom-value")
                            .role(Role::Status)
                            .aria_label(format!("{:.0}%", self.zoom * 100.))
                            .test_support()
                            .w(px(48.))
                            .text_center()
                            .child(format!("{:.0}%", self.zoom * 100.)),
                    )
                    .child(
                        Button::new("zoom-in")
                            .ghost()
                            .icon(gpui_kit::assets::IconName::ZoomIn)
                            .tooltip("Zoom in")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.fit = false;
                                this.zoom = (this.zoom * 1.25).min(4.0);
                                cx.notify();
                            })),
                    ),
            )
            .children(
                test.and_then(|test| test.pair.engine_error_msg.as_ref())
                    .map(|error| {
                        div()
                            .id("capture-error")
                            .role(Role::Alert)
                            .aria_label(error.clone())
                            .test_support()
                            .px_4()
                            .py_2()
                            .text_color(cx.theme().danger)
                            .child(error.clone())
                    }),
            )
            .child(div().flex_1().min_h_0().child(previews))
            .into_any_element()
    }
}

impl Render for ResultsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let report_name = self
            .selected_report()
            .map(ReportEntry::label)
            .unwrap_or_else(|| "No reports".into());
        let reports = self
            .catalog
            .reports
            .iter()
            .enumerate()
            .map(|(index, report)| (index, report.label()))
            .collect::<Vec<_>>();
        let counts = self
            .selected_report()
            .map(|report| report.counts())
            .map(|(passed, mismatch, errors)| {
                format!("{passed} passed   {mismatch} mismatched   {errors} errors")
            })
            .unwrap_or_default();
        let focus = self.focus.clone();
        let captures = self.render_captures(cx);
        let comparison = self.render_comparison(cx);
        div()
            .id("results-panel")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .on_action(cx.listener(|this, action: &SelectReport, window, cx| {
                if action.index < this.catalog.reports.len() {
                    this.report = action.index;
                    this.capture = None;
                    this.page = 0;
                    this.select_capture(0, window, cx);
                    cx.notify();
                }
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("report-picker")
                            .label(report_name)
                            .disabled(self.loading || reports.is_empty())
                            .dropdown_menu(move |menu, _, _| {
                                let mut menu = menu.action_context(focus.clone()).scrollable(true);
                                for (index, label) in &reports {
                                    menu = menu.menu(
                                        label.clone(),
                                        Box::new(SelectReport { index: *index }),
                                    );
                                }
                                menu
                            }),
                    )
                    .child(
                        Button::new("refresh-results")
                            .ghost()
                            .icon(gpui_kit::assets::IconName::RefreshCw)
                            .tooltip("Refresh reports")
                            .disabled(self.loading || self.root.is_none())
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(root) = this.root.clone() {
                                    this.refresh(root, window, cx);
                                }
                            })),
                    )
                    .child(
                        Button::new("open-report")
                            .ghost()
                            .icon(gpui_kit::assets::IconName::ExternalLink)
                            .tooltip("Open HTML report")
                            .disabled(self.root.is_none())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_report(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("open-artifacts")
                            .ghost()
                            .icon(IconName::FolderOpen)
                            .tooltip("Open artifact directory")
                            .disabled(self.root.is_none())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_external(
                                    this.root
                                        .clone()
                                        .ok_or_else(|| anyhow!("No output directory")),
                                    window,
                                    cx,
                                )
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(counts),
                    ),
            )
            .child(
                div().flex_1().min_h_0().child(
                    h_resizable("result-layout")
                        .child(
                            resizable_panel()
                                .size(px(270.))
                                .size_range(px(200.)..px(440.))
                                .child(captures),
                        )
                        .child(resizable_panel().child(comparison)),
                ),
            )
            .children(self.catalog.warnings.first().map(|warning| {
                div()
                    .px_4()
                    .py_2()
                    .text_xs()
                    .text_color(cx.theme().warning)
                    .child(warning.clone())
            }))
            .child(
                div()
                    .id("results-status")
                    .role(Role::Status)
                    .aria_label(self.status.clone())
                    .test_support()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(self.status.clone()),
            )
    }
}
