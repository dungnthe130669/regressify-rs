//! Report browsing and image loading are isolated from editable test documents.

use std::{cell::Cell, rc::Rc, sync::Arc};

use super::*;
use crate::desktop::results::{Catalog, Outcome, ReportEntry};

const PAGE_SIZE: usize = 50;

pub(super) enum ResultEvent {
    OpenSuite(Entry),
    ApproveRun(ReportApproval),
}

#[derive(Clone)]
pub(super) struct ReportApproval {
    pub entry: Entry,
    pub output: PathBuf,
    pub suite_id: String,
    pub run_id: String,
    pub scenario_ids: Vec<String>,
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

#[derive(Default)]
struct PointerGuides {
    active: Cell<Option<usize>>,
    bounds: [Cell<Option<Bounds<Pixels>>>; 3],
}

impl PointerGuides {
    fn position(&self, index: usize, mouse: Point<Pixels>) -> Option<Point<Pixels>> {
        let source = self.bounds.get(self.active.get()?)?.get()?;
        let target = self.bounds.get(index)?.get()?;
        if source.size.width <= px(0.) || source.size.height <= px(0.) || !source.contains(&mouse) {
            return None;
        }
        Some(point(
            target.origin.x + target.size.width * ((mouse.x - source.origin.x) / source.size.width),
            target.origin.y
                + target.size.height * ((mouse.y - source.origin.y) / source.size.height),
        ))
    }
}

pub(super) struct ResultsPanel {
    root: Option<PathBuf>,
    catalog: Catalog,
    suites: Vec<crate::desktop::workspace::SuiteSummary>,
    active_suite: Option<PathBuf>,
    report: Option<usize>,
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
    preview_scrolls: [ScrollHandle; 3],
    comparison_scroll: ScrollHandle,
    pointer_guides: Rc<PointerGuides>,
    status: String,
    loading: bool,
    image_loading: bool,
    generation: u64,
    preferred_run: Option<String>,
    preferred_reports: Option<Vec<(String, String)>>,
    disabled: bool,
}

impl EventEmitter<ResultEvent> for ResultsPanel {}

pub(super) struct CaptureList {
    results: Entity<ResultsPanel>,
    _subscription: Subscription,
}

impl CaptureList {
    pub(super) fn new(results: Entity<ResultsPanel>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&results, |_, _, cx| cx.notify());
        Self {
            results,
            _subscription: subscription,
        }
    }
}

impl Render for CaptureList {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.results
            .update(cx, |panel, cx| panel.render_captures(cx))
    }
}

impl ResultsPanel {
    pub(super) fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled != disabled {
            self.disabled = disabled;
            cx.notify();
        }
    }

    pub(super) fn show_run(
        &mut self,
        root: PathBuf,
        run_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_suite = None;
        self.preferred_reports = None;
        self.preferred_run = (!run_id.is_empty()).then_some(run_id);
        self.filter = 0;
        self.image_mode = 3;
        self.fit = true;
        self.search
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.refresh(root, window, cx);
    }

    pub(super) fn show_saved_reports(
        &mut self,
        root: PathBuf,
        targets: Vec<(String, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let matches = |suite: &&crate::desktop::workspace::SuiteSummary| {
            targets.iter().any(|(id, _)| id == &suite.output_id)
        };
        self.active_suite = self
            .suites
            .iter()
            .filter(matches)
            .find(|suite| self.active_suite.as_ref() == Some(&suite.entry.path))
            .or_else(|| self.suites.iter().find(matches))
            .map(|suite| suite.entry.path.clone());
        self.preferred_reports = Some(targets);
        self.preferred_run = None;
        self.filter = 0;
        self.image_mode = 3;
        self.fit = true;
        self.search
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.refresh(root, window, cx);
    }

    pub(super) fn set_suites(&mut self, suites: Vec<crate::desktop::workspace::SuiteSummary>) {
        if !suites
            .iter()
            .any(|suite| self.active_suite.as_ref() == Some(&suite.entry.path))
        {
            self.active_suite = None;
        }
        self.suites = suites;
    }

    pub(super) fn active_suite(&self) -> Option<&PathBuf> {
        self.active_suite.as_ref()
    }

    pub(super) fn select_suite(
        &mut self,
        entry: &Entry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_suite.as_ref() == Some(&entry.path) {
            return;
        }
        self.active_suite = Some(entry.path.clone());
        self.report = self.suite_report();
        self.capture = None;
        self.page = 0;
        self.previews.clear();
        self.image_loading = false;
        let outside_requested = self.preferred_reports.as_ref().is_some_and(|targets| {
            !self.suites.iter().any(|suite| {
                suite.entry.path == entry.path
                    && targets.iter().any(|(id, _)| id == &suite.output_id)
            })
        }) || (self.preferred_run.is_some() && self.report.is_none());
        if outside_requested && let Some(root) = self.root.clone() {
            self.preferred_run = None;
            self.preferred_reports = None;
            self.refresh(root, window, cx);
        } else if !self.loading {
            self.generation += 1;
            self.reconcile_capture(window, cx);
        }
        cx.notify();
    }

    fn suite_report(&self) -> Option<usize> {
        let path = self.active_suite.as_ref()?;
        self.catalog.reports.iter().position(|report| {
            report
                .source
                .as_ref()
                .is_some_and(|entry| &entry.path == path)
        })
    }

    pub(super) fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.generation += 1;
        self.root = None;
        self.preferred_run = None;
        self.preferred_reports = None;
        self.catalog = Catalog::default();
        self.suites.clear();
        self.active_suite = None;
        self.report = None;
        self.capture = None;
        self.filter = 0;
        self.page = 0;
        self.previews.clear();
        self.loading = false;
        self.image_loading = false;
        self.status = "No reports loaded".into();
        self.search
            .update(cx, |state, cx| state.set_value("", window, cx));
        cx.notify();
    }

    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Filter captures"));
        let subscription =
            cx.subscribe_in(&search, window, |this: &mut Self, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.reconcile_capture(window, cx);
                }
            });
        Self {
            root: None,
            catalog: Catalog::default(),
            suites: Vec::new(),
            active_suite: None,
            report: None,
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
            preview_scrolls: Default::default(),
            comparison_scroll: ScrollHandle::default(),
            pointer_guides: Rc::default(),
            status: "No reports loaded".into(),
            loading: false,
            image_loading: false,
            generation: 0,
            preferred_run: None,
            preferred_reports: None,
            disabled: false,
        }
    }

    pub(super) fn refresh(&mut self, root: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let preferred_run = self.preferred_run.clone();
        let requested_reports = self.preferred_reports.clone();
        let previous = self
            .selected_report()
            .map(|report| (report.path.clone(), self.capture));
        self.generation += 1;
        let generation = self.generation;
        self.root = Some(root.clone());
        self.loading = true;
        self.image_loading = false;
        self.previews.clear();
        self.status = "Loading reports...".into();
        let suites = self.suites.clone();
        let requested_run = preferred_run.clone();
        let task = cx.background_executor().spawn(async move {
            let mut catalog = Catalog::load(&root)?;
            catalog.resolve_names(&suites);
            if let Some(run) = requested_run {
                catalog.reports.retain(|report| report.run_id == run);
            }
            if let Some(targets) = requested_reports {
                catalog.reports.retain(|report| {
                    targets
                        .iter()
                        .any(|(suite, run)| suite == &report.report.id && run == &report.run_id)
                });
            }
            Ok::<_, anyhow::Error>(catalog)
        });
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
                        if this.active_suite.is_none() {
                            this.active_suite = this.catalog.reports.iter().find_map(|report| {
                                report.source.as_ref().map(|entry| entry.path.clone())
                            });
                        }
                        this.report = this.suite_report();
                        this.capture = None;
                        this.page = 0;
                        this.status = format!(
                            "{} {}, {} {}",
                            this.catalog.reports.len(),
                            if this.catalog.reports.len() == 1 {
                                "report"
                            } else {
                                "reports"
                            },
                            this.catalog.warnings.len(),
                            if this.catalog.warnings.len() == 1 {
                                "warning"
                            } else {
                                "warnings"
                            }
                        );
                        if let Some(report) = this.selected_report()
                            && !report.report.tests.is_empty()
                        {
                            let capture = previous
                                .as_ref()
                                .filter(|(path, _)| *path == report.path)
                                .and_then(|(_, index)| *index)
                                .filter(|index| *index < report.report.tests.len())
                                .unwrap_or(0);
                            let filtered = this.filtered(cx);
                            if filtered.contains(&capture) {
                                this.select_capture(capture, window, cx);
                            } else {
                                this.reconcile_capture(window, cx);
                            }
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
        self.catalog.reports.get(self.report?).filter(|report| {
            report
                .source
                .as_ref()
                .is_some_and(|entry| self.active_suite.as_ref() == Some(&entry.path))
        })
    }

    /// Keep selection and pagination inside the current filter without reloading unchanged images.
    fn reconcile_capture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let filtered = self.filtered(cx);
        let selection = self
            .capture
            .filter(|capture| filtered.contains(capture))
            .or_else(|| filtered.first().copied());
        self.page = selection
            .and_then(|capture| filtered.iter().position(|index| *index == capture))
            .unwrap_or(0)
            / PAGE_SIZE;
        if selection != self.capture {
            if let Some(capture) = selection {
                self.select_capture(capture, window, cx);
            } else {
                self.generation += 1;
                self.capture = None;
                self.previews.clear();
                self.image_loading = false;
            }
        }
        cx.notify();
    }

    fn select_capture(&mut self, capture: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading
            || (self.capture == Some(capture) && (self.image_loading || !self.previews.is_empty()))
        {
            return;
        }
        let Some(report) = self.selected_report() else {
            return;
        };
        let Some(test) = report.report.tests.get(capture) else {
            return;
        };
        let Some(root) = self.root.clone() else {
            return;
        };
        let artifacts = report.artifacts();
        let images = [
            ("Reference", test.pair.reference.clone()),
            ("Current", test.pair.test.clone()),
            ("Diff", test.pair.diff_image.clone()),
        ];
        self.comparison_scroll.set_offset(point(px(0.), px(0.)));
        self.pointer_guides = Rc::default();
        self.page = self
            .filtered(cx)
            .iter()
            .position(|index| *index == capture)
            .unwrap_or(0)
            / PAGE_SIZE;
        self.capture = Some(capture);
        self.previews.clear();
        self.image_loading = true;
        self.generation += 1;
        let generation = self.generation;
        let task = cx.background_executor().spawn(async move {
            images
                .into_iter()
                .map(|(label, path)| {
                    let result = (|| -> Result<LoadedImage> {
                        let path =
                            path.ok_or_else(|| anyhow!("No {} image", label.to_lowercase()))?;
                        let image = artifacts.image(&root, &path)?;
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
        let query = self.search.read(cx).value().trim().to_lowercase();
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
                        status && (query.is_empty() || test.pair.matches(&query))
                    })
                    .map(|(index, _)| index)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn adjacent_capture(&self, forward: bool, failed_only: bool, cx: &App) -> Option<usize> {
        if self.loading {
            return None;
        }
        let report = self.selected_report()?;
        let mut candidates = self.filtered(cx).into_iter().filter(|index| {
            !failed_only || report.report.tests[*index].outcome() != Outcome::Passed
        });
        if forward {
            candidates.find(|index| self.capture.is_none_or(|current| *index > current))
        } else {
            candidates.rfind(|index| self.capture.is_none_or(|current| *index < current))
        }
    }

    fn move_capture(
        &mut self,
        forward: bool,
        failed_only: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(capture) = self.adjacent_capture(forward, failed_only, cx) {
            self.select_capture(capture, window, cx);
        }
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
            .test_support()
            .flex()
            .flex_col()
            .gap_1()
            .p_3()
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .pb_2()
                    .child("Captures"),
            )
            .child(
                Input::new(&self.search)
                    .id("capture-search")
                    .prefix(IconName::Search)
                    .cleanable(true)
                    .small(),
            )
            .child(
                TabBar::new("result-filter")
                    .segmented()
                    .selected_index(self.filter)
                    .on_click(cx.listener(|this, index, window, cx| {
                        this.filter = *index;
                        this.reconcile_capture(window, cx);
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
            .overflow_y_scrollbar()
            .gap_1();
        if filtered.is_empty() {
            let message = if self.loading {
                "Loading captures..."
            } else if self.active_suite.is_none() {
                "Select a test suite"
            } else if self.selected_report().is_none() {
                "No captures for this suite"
            } else {
                "No matching captures"
            };
            rows = rows.child(
                div()
                    .id("capture-empty")
                    .role(Role::Status)
                    .aria_label(message)
                    .test_support()
                    .py_6()
                    .text_center()
                    .text_color(cx.theme().muted_foreground)
                    .child(message),
            );
        }
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
                        .h(px(56.))
                        .justify_start()
                        .selected(self.capture == Some(index))
                        .accessibility_label(format!(
                            "{} {}",
                            outcome.label(),
                            test.pair.display.title
                        ))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .w_full()
                                .min_w_0()
                                .child(
                                    Icon::new(match outcome {
                                        Outcome::Passed => gpui_kit::assets::IconName::CircleCheck,
                                        Outcome::Mismatch => gpui_kit::assets::IconName::Split,
                                        _ => gpui_kit::assets::IconName::CircleAlert,
                                    })
                                    .small()
                                    .text_color(color),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .flex_1()
                                        .min_w_0()
                                        .text_left()
                                        .child(
                                            div()
                                                .truncate()
                                                .text_color(cx.theme().foreground)
                                                .child(test.pair.display.title.clone()),
                                        )
                                        .child(div().text_xs().truncate().text_color(color).child(
                                            if test.pair.display.detail.is_empty() {
                                                outcome.label().to_string()
                                            } else {
                                                format!(
                                                    "{} / {}",
                                                    outcome.label(),
                                                    test.pair.display.detail
                                                )
                                            },
                                        )),
                                ),
                        )
                        .tooltip(
                            test.pair
                                .engine_error_msg
                                .clone()
                                .map(|error| format!("{error}\n{}", test.pair.capture_key()))
                                .unwrap_or_else(|| {
                                    format!(
                                        "{}\n{}\n{}",
                                        test.pair.display.title,
                                        test.pair.display.detail,
                                        test.pair.capture_key()
                                    )
                                }),
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

    fn preview_canvas_size(&self, index: usize) -> (usize, usize) {
        let images = if self.image_mode == 3 && index < 2 {
            &self.previews[..self.previews.len().min(2)]
        } else {
            self.previews
                .get(index)
                .map(std::slice::from_ref)
                .unwrap_or_default()
        };
        images
            .iter()
            .filter_map(|preview| preview.result.as_ref().ok())
            .fold((1, 1), |(width, height), image| {
                (width.max(image.width), height.max(image.height))
            })
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
                let (canvas_width, canvas_height) = self.preview_canvas_size(index);
                let scroll = if self.image_mode == 3 && index < 2 {
                    &self.comparison_scroll
                } else {
                    &self.preview_scrolls[index]
                };
                let display = div().id(("image-content", index)).test_support().relative();
                let display = if self.fit {
                    display
                        .w_full()
                        .aspect_ratio(canvas_width as f32 / canvas_height as f32)
                } else {
                    display
                        .w(px(canvas_width as f32 * self.zoom))
                        .h(px(canvas_height as f32 * self.zoom))
                }
                .child(
                    img(image.image.clone())
                        .absolute()
                        .top_0()
                        .left_0()
                        .w(relative(image.width as f32 / canvas_width as f32))
                        .h(relative(image.height as f32 / canvas_height as f32))
                        .object_fit(ObjectFit::Contain),
                )
                .cursor(CursorStyle::Crosshair)
                .on_mouse_move(cx.listener(move |this, _, _, cx| {
                    this.pointer_guides.active.set(Some(index));
                    cx.notify();
                }))
                .on_hover(cx.listener(move |this, hovered, _, cx| {
                    if *hovered {
                        this.pointer_guides.active.set(Some(index));
                    } else if this.pointer_guides.active.get() == Some(index) {
                        this.pointer_guides.active.set(None);
                    }
                    cx.notify();
                }))
                .child({
                    let geometry = self.pointer_guides.clone();
                    let guides = self.pointer_guides.clone();
                    canvas(
                        move |bounds, _, _| geometry.bounds[index].set(Some(bounds)),
                        move |bounds, _, window, _| {
                            let Some(pointer) = guides.position(index, window.mouse_position())
                            else {
                                return;
                            };
                            for (thickness, color) in
                                [(3., rgba(0x000000b3)), (1., rgba(0xffffffff))]
                            {
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(pointer.x - px(thickness / 2.), bounds.origin.y),
                                        size(px(thickness), bounds.size.height),
                                    ),
                                    color,
                                ));
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(bounds.origin.x, pointer.y - px(thickness / 2.)),
                                        size(bounds.size.width, px(thickness)),
                                    ),
                                    color,
                                ));
                            }
                        },
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                });
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
                            .flex_1()
                            .min_h_0()
                            .relative()
                            .child(
                                div()
                                    .id(("image-scroll", index))
                                    .role(Role::Group)
                                    .aria_label(format!("{} image preview", preview.label))
                                    .test_support()
                                    .size_full()
                                    .relative()
                                    .overflow_scroll()
                                    .map(|mut viewport| {
                                        viewport.style().allow_concurrent_scroll = Some(true);
                                        viewport
                                    })
                                    .track_scroll(scroll)
                                    .bg(cx.theme().muted)
                                    .child(display),
                            )
                            .scrollbar(scroll, scroll::ScrollbarAxis::Both),
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
            .map(|test| test.pair.display.title.clone())
            .unwrap_or_else(|| "Comparison".into());
        let filtered = self.filtered(cx);
        let position = self
            .capture
            .and_then(|capture| filtered.iter().position(|index| *index == capture))
            .map_or(0, |index| index + 1);
        let position = format!("{position} of {}", filtered.len());
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
        let source = self
            .selected_report()
            .and_then(|report| report.source.clone());
        div()
            .id("comparison-view")
            .test_support()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .child(
                div()
                    .px_4()
                    .py_2()
                    .flex()
                    .items_center()
                    .gap_3()
                    .font_weight(FontWeight::MEDIUM)
                    .child(
                        div()
                            .id("capture-heading")
                            .role(Role::Heading)
                            .aria_label(title.clone())
                            .test_support()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(title),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap_1()
                            .text_xs()
                            .flex_shrink_0()
                            .child(if mismatch.is_empty() {
                                String::new()
                            } else {
                                format!("{mismatch} mismatch")
                            })
                            .children(test.and_then(|test| test.pair.mis_match_threshold).map(
                                |threshold| {
                                    let label = format!("Allowed: {threshold}%");
                                    div()
                                        .id("capture-threshold")
                                        .role(Role::Status)
                                        .aria_label(label.clone())
                                        .test_support()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(label)
                                },
                            )),
                    ),
            )
            .when_some(test, |comparison, test| {
                comparison.child(
                    div()
                        .flex()
                        .items_center()
                        .flex_wrap()
                        .gap_2()
                        .px_4()
                        .pb_2()
                        .child(
                            div()
                                .id("capture-context")
                                .role(Role::Status)
                                .aria_label(test.pair.display.detail.clone())
                                .test_support()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(test.pair.display.detail.clone()),
                        )
                        .child(div().flex_1())
                        .child(
                            Button::new("copy-capture-details")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Copy)
                                .tooltip("Copy capture details")
                                .accessibility_label("Copy capture details")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(test) = this.selected_report().and_then(|report| {
                                        this.capture
                                            .and_then(|index| report.report.tests.get(index))
                                    }) {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            test.copy_summary(),
                                        ));
                                        this.status = "Capture details copied".into();
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(
                            Button::new("edit-capture-suite")
                                .ghost()
                                .xsmall()
                                .icon(gpui_kit::assets::IconName::Pencil)
                                .label("Edit suite")
                                .tooltip("Open the suite that produced this capture")
                                .disabled(source.is_none() || self.loading)
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    if let Some(entry) = &source {
                                        cx.emit(ResultEvent::OpenSuite(entry.clone()));
                                    }
                                })),
                        ),
                )
            })
            .child(
                TabBar::new("image-modes")
                    .underline()
                    .selected_index(self.image_mode)
                    .on_click(cx.listener(|this, index, _, cx| {
                        this.image_mode = *index;
                        this.pointer_guides.active.set(None);
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
                    .flex_wrap()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("previous-failed-capture")
                                    .ghost()
                                    .small()
                                    .icon(gpui_kit::assets::IconName::ChevronsLeft)
                                    .tooltip("Previous failed capture")
                                    .disabled(self.adjacent_capture(false, true, cx).is_none())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.move_capture(false, true, window, cx)
                                    })),
                            )
                            .child(
                                Button::new("previous-capture")
                                    .ghost()
                                    .small()
                                    .icon(IconName::ChevronLeft)
                                    .tooltip("Previous capture")
                                    .disabled(self.adjacent_capture(false, false, cx).is_none())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.move_capture(false, false, window, cx)
                                    })),
                            )
                            .child(
                                div()
                                    .id("capture-position")
                                    .role(Role::Status)
                                    .aria_label(position.clone())
                                    .test_support()
                                    .w(px(76.))
                                    .text_center()
                                    .text_xs()
                                    .child(position),
                            )
                            .child(
                                Button::new("next-capture")
                                    .ghost()
                                    .small()
                                    .icon(IconName::ChevronRight)
                                    .tooltip("Next capture")
                                    .disabled(self.adjacent_capture(true, false, cx).is_none())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.move_capture(true, false, window, cx)
                                    })),
                            )
                            .child(
                                Button::new("next-failed-capture")
                                    .ghost()
                                    .small()
                                    .icon(gpui_kit::assets::IconName::ChevronsRight)
                                    .tooltip("Next failed capture")
                                    .disabled(self.adjacent_capture(true, true, cx).is_none())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.move_capture(true, true, window, cx)
                                    })),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        Checkbox::new("fit-image")
                            .label("Fit width")
                            .checked(self.fit)
                            .disabled(self.capture.is_none())
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
                            .accessibility_label("Zoom out")
                            .disabled(self.capture.is_none())
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
                            .accessibility_label("Zoom in")
                            .disabled(self.capture.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.fit = false;
                                this.zoom = (this.zoom * 1.25).min(4.0);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("actual-size")
                            .ghost()
                            .icon(gpui_kit::assets::IconName::Scan)
                            .tooltip("Actual size (100%)")
                            .accessibility_label("Actual size (100%)")
                            .disabled(self.capture.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.fit = false;
                                this.zoom = 1.0;
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

#[cfg(test)]
mod pointer_tests {
    use super::PointerGuides;
    use gpui_kit::{Bounds, point, px, size};

    #[test]
    fn crosshairs_use_the_same_image_coordinate_after_scroll_and_scaling() {
        let guides = PointerGuides::default();
        guides.active.set(Some(0));
        guides.bounds[0].set(Some(Bounds::new(
            point(px(-100.), px(-200.)),
            size(px(1000.), px(2000.)),
        )));
        guides.bounds[1].set(Some(Bounds::new(
            point(px(600.), px(-200.)),
            size(px(1000.), px(2000.)),
        )));
        assert_eq!(
            guides.position(1, point(px(150.), px(300.))),
            Some(point(px(850.), px(300.)))
        );
        guides.bounds[1].set(Some(Bounds::new(
            point(px(600.), px(10.)),
            size(px(500.), px(1000.)),
        )));
        assert_eq!(
            guides.position(1, point(px(150.), px(300.))),
            Some(point(px(725.), px(260.)))
        );
        assert!(guides.position(1, point(px(-101.), px(300.))).is_none());
        guides.active.set(None);
        assert!(guides.position(1, point(px(150.), px(300.))).is_none());
    }

    #[cfg(feature = "gui-tests")]
    #[gpui_kit::test]
    async fn crosshairs_follow_either_pane_and_clear_on_exit(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::AppContext;
        use gpui_kit::test::{TestAppContextExt, TestWindowExt};
        use std::time::Duration;

        let directory = tempfile::tempdir().unwrap();
        let run = directory
            .path()
            .join(".regressify/ui-test/bitmaps_test/run-1");
        std::fs::create_dir_all(&run).unwrap();
        lodepng::encode32_file(
            run.join("capture.png"),
            &[30_u8, 150, 180, 255].repeat(1200 * 1800),
            1200,
            1800,
        )
        .unwrap();
        std::fs::write(
            run.join("report.json"),
            serde_json::to_vec(&serde_json::json!({
                "id": "ui-test", "tests": [{"status": "pass", "pair": {
                    "label": "Pointer fixture",
                    "reference": "../bitmaps_test/run-1/capture.png",
                    "test": "../bitmaps_test/run-1/capture.png"
                }}]
            }))
            .unwrap(),
        )
        .unwrap();
        let (handle, view) = crate::desktop::ui::tests::editor(cx, directory.path());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.within("workbench-tabs").click(2_usize, cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
            window.try_find(("image-details", 1_usize)).is_some()
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.within("image-modes").click(3_usize, cx);
            window.click("actual-size", cx);
            let results = view.read(cx).results.clone();
            for index in [0_usize, 1] {
                window.hover(("image-scroll", index), cx);
                window.render_frame(cx);
                let guides = results.read(cx).pointer_guides.clone();
                assert_eq!(guides.active.get(), Some(index));
                let mouse = window.mouse_position();
                let source = guides.bounds[index].get().unwrap();
                let target = guides.bounds[1 - index].get().unwrap();
                assert_eq!(guides.position(index, mouse), Some(mouse));
                assert_eq!(
                    guides.position(1 - index, mouse),
                    Some(mouse - source.origin + target.origin)
                );
                window.scroll(
                    ("image-scroll", index),
                    gpui_kit::ScrollDelta::Pixels(point(px(-90.), px(-140.))),
                    cx,
                );
                window.render_frame(cx);
                assert_eq!(window.mouse_position(), mouse);
                let source = guides.bounds[index].get().unwrap();
                let target = guides.bounds[1 - index].get().unwrap();
                assert_eq!(
                    guides.position(1 - index, mouse),
                    Some(mouse - source.origin + target.origin)
                );
            }
            let image = results.read(cx).previews[0]
                .result
                .as_ref()
                .unwrap()
                .image
                .clone();
            let offset = results.read(cx).comparison_scroll.offset();
            window.click(("capture", 0_usize), cx);
            assert!(std::sync::Arc::ptr_eq(
                &image,
                &results.read(cx).previews[0].result.as_ref().unwrap().image
            ));
            assert_eq!(results.read(cx).comparison_scroll.offset(), offset);
            assert!(!results.read(cx).image_loading);
            window.hover("actual-size", cx);
            window.render_frame(cx);
            let guides = results.read(cx).pointer_guides.clone();
            assert_eq!(guides.active.get(), None);
            assert!(guides.position(0, window.mouse_position()).is_none());
            assert!(guides.position(1, window.mouse_position()).is_none());
        })
        .unwrap();
    }
}

impl Render for ResultsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let report_name = self
            .selected_report()
            .map(ReportEntry::label)
            .unwrap_or_else(|| "No reports".into());
        let counts = self
            .selected_report()
            .map(|report| report.counts())
            .map(|(passed, mismatch, errors)| {
                format!(
                    "{passed} passed   {mismatch} mismatched   {errors} {}",
                    if errors == 1 { "error" } else { "errors" }
                )
            })
            .unwrap_or_default();
        let approval = self
            .selected_report()
            .filter(|report| {
                report.report.tests.iter().any(|test| {
                    test.pair.test.is_some()
                        && matches!(test.outcome(), Outcome::Passed | Outcome::Mismatch)
                })
            })
            .and_then(|report| {
                Some(ReportApproval {
                    entry: report.source.clone()?,
                    output: self.root.clone()?,
                    suite_id: report.report.id.clone(),
                    run_id: report.run_id.clone(),
                    scenario_ids: report
                        .report
                        .tests
                        .iter()
                        .map(|test| test.pair.scenario_id.clone())
                        .collect::<Option<BTreeSet<_>>>()
                        .unwrap_or_default()
                        .into_iter()
                        .collect(),
                })
            });
        div()
            .id("results-panel")
            .test_support()
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
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
                        div()
                            .id("report-title")
                            .role(Role::Status)
                            .aria_label(report_name.clone())
                            .test_support()
                            .flex_1()
                            .min_w(px(160.))
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(report_name),
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
                        Button::new("approve-report")
                            .small()
                            .icon(gpui_kit::assets::IconName::Check)
                            .label("Approve This Run")
                            .tooltip(
                                "Use successful captures from this report run as reference images",
                            )
                            .disabled(self.disabled || self.loading || approval.is_none())
                            .on_click(cx.listener(move |_, _, _, cx| {
                                if let Some(approval) = &approval {
                                    cx.emit(ResultEvent::ApproveRun(approval.clone()));
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
            .child(div().flex_1().min_h_0().child(self.render_comparison(cx)))
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
