//! Report browsing and image loading are isolated from editable test documents.

use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    sync::Arc,
};

use super::*;
use crate::desktop::results::{
    Catalog, Outcome, Pair, ReportArtifacts, ReportEntry, ReportSelection,
};
use gpui_kit::component::hover_card::HoverCard;
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};

mod log;

const ZOOM_LEVELS: [f32; 17] = [
    0.25,
    1. / 3.,
    0.5,
    2. / 3.,
    0.75,
    0.8,
    0.9,
    1.,
    1.1,
    1.25,
    1.5,
    1.75,
    2.,
    2.5,
    3.,
    4.,
    5.,
];

fn next_zoom(current: f32, increase: bool) -> f32 {
    if increase {
        ZOOM_LEVELS
            .iter()
            .copied()
            .find(|zoom| *zoom > current + 0.001)
            .unwrap_or(ZOOM_LEVELS[ZOOM_LEVELS.len() - 1])
    } else {
        ZOOM_LEVELS
            .iter()
            .copied()
            .rfind(|zoom| *zoom < current - 0.001)
            .unwrap_or(current.min(ZOOM_LEVELS[0]))
    }
}

fn image_page_url(pair: &Pair, index: usize) -> Option<reqwest::Url> {
    let value = match index {
        0 => pair.reference_url.as_deref(),
        1 => pair.url.as_deref(),
        _ => None,
    }?;
    reqwest::Url::parse(value)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https" | "file"))
}

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
    pub scenario_id: String,
    pub capture_key: String,
}

struct Preview {
    label: &'static str,
    result: std::result::Result<LoadedImage, String>,
}

struct LoadedImage {
    image: Arc<Image>,
    width: usize,
    height: usize,
    path: Option<PathBuf>,
}

impl LoadedImage {
    fn load(
        artifacts: &ReportArtifacts,
        root: &Path,
        label: &'static str,
        path: Option<&str>,
    ) -> Result<Self> {
        match path.map(|path| artifacts.read_image(root, path)) {
            Some(Ok((image, bytes))) => {
                if matches!(label, "Reference" | "Test")
                    && crate::test::capture::read_error_marker(&image.path).is_some()
                {
                    return Self::error_placeholder();
                }
                Ok(Self {
                    image: Arc::new(Image::from_bytes(ImageFormat::Png, bytes)),
                    width: image.width,
                    height: image.height,
                    path: Some(image.path),
                })
            }
            Some(Err(error))
                if label == "Reference"
                    && error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
            {
                Self::error_placeholder()
            }
            None if label == "Reference" => Self::error_placeholder(),
            Some(Err(error)) => Err(error),
            None => Err(anyhow!("No {} image", label.to_lowercase())),
        }
    }

    fn error_placeholder() -> Result<Self> {
        let bytes = crate::test::capture::error_icon_png();
        let (width, height) = crate::helpers::png::read_bounded_png_dimensions_from_reader(
            bytes,
            Path::new("error-icon.png"),
        )?;
        Ok(Self {
            image: Arc::new(Image::from_bytes(ImageFormat::Png, bytes.to_vec())),
            width,
            height,
            path: None,
        })
    }
}

struct ImageMenu {
    menu: Entity<PopupMenu>,
    position: Point<Pixels>,
    _subscription: Subscription,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ZoomMode {
    FitArea,
    FitWidth,
    Manual,
}

#[derive(Clone, Copy)]
struct PreviewView {
    mode: ZoomMode,
    zoom: f32,
    padding: Option<gpui_kit::Edges<Pixels>>,
}

impl Default for PreviewView {
    fn default() -> Self {
        Self {
            mode: ZoomMode::FitArea,
            zoom: 1.0,
            padding: None,
        }
    }
}

#[derive(Default, Clone)]
struct PreviewTexture(Rc<RefCell<Option<Arc<gpui_kit::RenderImage>>>>);

impl PreviewTexture {
    fn source(&self, image: Arc<Image>) -> gpui_kit::ImageSource {
        let texture = self.clone();
        gpui_kit::ImageSource::from(move |window: &mut Window, cx: &mut App| {
            let rendered = image.clone().use_render_image(window, cx)?;
            *texture.0.borrow_mut() = Some(rendered.clone());
            Some(Ok(rendered))
        })
    }
}

#[derive(Clone)]
struct PreviewPan {
    position: Point<Pixels>,
    offset: Point<Pixels>,
    scroll: ScrollHandle,
    horizontal: bool,
    vertical: bool,
    dragged: bool,
}

#[derive(Default)]
struct PointerGuides {
    active: Cell<Option<usize>>,
    bounds: [Cell<Option<Bounds<Pixels>>>; 3],
    pan: RefCell<Option<PreviewPan>>,
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

struct PointerLayer {
    index: usize,
    guides: Rc<PointerGuides>,
}

impl Render for PointerLayer {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let index = self.index;
        let geometry = self.guides.clone();
        let guides = self.guides.clone();
        canvas(
            move |bounds, _, _| geometry.bounds[index].set(Some(bounds)),
            move |bounds, _, window, _| {
                let panning = guides.clone();
                window.on_mouse_event(
                    move |event: &gpui_kit::MouseMoveEvent, phase, window, cx| {
                        if phase != gpui_kit::DispatchPhase::Bubble {
                            return;
                        }
                        let Some(pan) = panning.pan.borrow().clone() else {
                            return;
                        };
                        if event.pressed_button != Some(MouseButton::Left) {
                            panning.pan.borrow_mut().take();
                            window.refresh();
                            return;
                        }
                        let movement = event.position - pan.position;
                        if !pan.dragged && movement.x.abs().max(movement.y.abs()) < px(4.) {
                            return;
                        }
                        if let Some(pan) = panning.pan.borrow_mut().as_mut() {
                            pan.dragged = true;
                        }
                        let maximum = pan.scroll.max_offset();
                        pan.scroll.set_offset(point(
                            if pan.horizontal {
                                (pan.offset.x + movement.x).clamp(-maximum.x, px(0.))
                            } else {
                                pan.offset.x
                            },
                            if pan.vertical {
                                (pan.offset.y + movement.y).clamp(-maximum.y, px(0.))
                            } else {
                                pan.offset.y
                            },
                        ));
                        window.refresh();
                        cx.stop_propagation();
                    },
                );
                let panning = guides.clone();
                window.on_mouse_event(move |event: &gpui_kit::MouseUpEvent, phase, window, cx| {
                    if phase == gpui_kit::DispatchPhase::Capture
                        && event.button == MouseButton::Left
                        && let Some(pan) = panning.pan.borrow_mut().take()
                    {
                        window.refresh();
                        if pan.dragged {
                            cx.stop_propagation();
                        }
                    }
                });
                let Some(pointer) = guides.position(index, window.mouse_position()) else {
                    return;
                };
                for (thickness, color) in [(3., rgba(0x000000b3)), (1., rgba(0xffffffff))] {
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
        .size_full()
    }
}

pub(super) struct ResultsPanel {
    capture_log: log::CaptureLogView,
    root: Option<PathBuf>,
    catalog: Catalog,
    suites: Vec<crate::desktop::workspace::SuiteSummary>,
    active_suite: Option<PathBuf>,
    report: Option<usize>,
    capture: Option<usize>,
    visible_captures: Vec<usize>,
    filter: usize,
    capture_scroll: gpui_kit::UniformListScrollHandle,
    image_mode: usize,
    preview_views: [PreviewView; 4],
    preview_scenario: Option<(PathBuf, String)>,
    visible: bool,
    search: Entity<InputState>,
    _subscription: Subscription,
    focus: FocusHandle,
    capture_focus: FocusHandle,
    previews: Vec<Preview>,
    image_menu: Option<ImageMenu>,
    preview_textures: [PreviewTexture; 3],
    image_task: Option<gpui_kit::Task<()>>,
    image_load_gate: Arc<futures::lock::Mutex<()>>,
    preview_scrolls: [ScrollHandle; 3],
    comparison_scroll: ScrollHandle,
    pointer_guides: Rc<PointerGuides>,
    pointer_layers: [Entity<PointerLayer>; 3],
    status: String,
    loading: bool,
    image_loading: bool,
    generation: u64,
    preferred_run: Option<String>,
    preferred_reports: Option<Vec<(String, String)>>,
    approval_confirmation: bool,
    approval_error: Option<String>,
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
    pub(super) fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.visible && !visible {
            self.reset_view();
            cx.notify();
        }
        self.visible = visible;
    }

    fn reset_view(&mut self) {
        self.approval_confirmation = false;
        self.approval_error = None;
        self.image_menu = None;
        self.preview_views = Default::default();
        self.comparison_scroll.set_offset(point(px(0.), px(0.)));
        for scroll in &self.preview_scrolls {
            scroll.set_offset(point(px(0.), px(0.)));
        }
        self.pointer_guides.pan.borrow_mut().take();
        self.pointer_guides.active.set(None);
    }

    fn select_image_mode(&mut self, index: usize, cx: &mut Context<Self>) {
        self.image_menu = None;
        self.image_mode = index;
        self.pointer_guides.pan.borrow_mut().take();
        self.pointer_guides.active.set(None);
        cx.notify();
    }

    fn set_zoom_mode(&mut self, mode: ZoomMode, cx: &mut Context<Self>) {
        let view = &mut self.preview_views[self.image_mode];
        if view.mode == mode {
            return;
        }
        view.mode = mode;
        view.padding = None;
        let scroll = if self.image_mode == 3 {
            &self.comparison_scroll
        } else {
            &self.preview_scrolls[self.image_mode]
        };
        scroll.set_offset(point(px(0.), px(0.)));
        self.pointer_guides.pan.borrow_mut().take();
        cx.notify();
    }

    pub(super) fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled != disabled {
            self.disabled = disabled;
            if disabled {
                self.approval_confirmation = false;
            }
            cx.notify();
        }
    }

    pub(super) fn approval_failed(&mut self, error: String, cx: &mut Context<Self>) {
        self.approval_confirmation = false;
        self.approval_error = Some(error);
        cx.notify();
    }

    pub(super) fn approval_finished(
        &mut self,
        outcome: Option<&crate::desktop::runner::RunOutcome>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::desktop::runner::RunOutcome;

        if let Some(root) = self.root.clone() {
            self.refresh(root, window, cx);
        }
        self.approval_error = match outcome {
            Some(RunOutcome::Completed) => None,
            Some(RunOutcome::Failed(error)) => Some(format!("Approval failed: {error}")),
            Some(RunOutcome::Cancelled) => Some("Approval cancelled".into()),
            None => Some("Approval did not complete".into()),
        };
        cx.notify();
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
        self.reset_view();
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
        self.reset_view();
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

    pub(super) fn suite_counts(&self, entry: &Entry) -> Option<crate::progress::Counts> {
        self.catalog.reports.iter().find_map(|report| {
            report
                .source
                .as_ref()
                .filter(|source| source.path == entry.path)
                .map(|_| report.report.scenario_counts())
        })
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
        self.visible_captures.clear();
        self.capture_scroll = Default::default();
        self.preview_scenario = None;
        self.reset_view();
        self.reset_previews(Some(window), cx);
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
        self.visible_captures.clear();
        self.filter = 0;
        self.capture_scroll = Default::default();
        self.preview_scenario = None;
        self.reset_view();
        self.reset_previews(Some(window), cx);
        self.loading = false;
        self.status = "No reports loaded".into();
        self.search
            .update(cx, |state, cx| state.set_value("", window, cx));
        cx.notify();
    }

    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.on_release(|this, cx| this.reset_previews(None, cx))
            .detach();
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Filter captures"));
        let subscription =
            cx.subscribe_in(&search, window, |this: &mut Self, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.reconcile_capture(window, cx);
                }
            });
        let pointer_guides = Rc::<PointerGuides>::default();
        let pointer_layers = std::array::from_fn(|index| {
            cx.new(|_| PointerLayer {
                index,
                guides: pointer_guides.clone(),
            })
        });
        Self {
            capture_log: log::CaptureLogView::new(window, cx),
            root: None,
            catalog: Catalog::default(),
            suites: Vec::new(),
            active_suite: None,
            report: None,
            capture: None,
            visible_captures: Vec::new(),
            filter: 0,
            capture_scroll: Default::default(),
            image_mode: 3,
            preview_views: Default::default(),
            preview_scenario: None,
            visible: false,
            search,
            _subscription: subscription,
            focus: cx.focus_handle(),
            capture_focus: cx.focus_handle().tab_index(0),
            previews: Vec::new(),
            image_menu: None,
            preview_textures: Default::default(),
            image_task: None,
            image_load_gate: Arc::default(),
            preview_scrolls: Default::default(),
            comparison_scroll: ScrollHandle::default(),
            pointer_guides,
            pointer_layers,
            status: "No reports loaded".into(),
            loading: false,
            image_loading: false,
            generation: 0,
            preferred_run: None,
            preferred_reports: None,
            approval_confirmation: false,
            approval_error: None,
            disabled: false,
        }
    }

    pub(super) fn refresh(&mut self, root: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.capture_log.generation = self.capture_log.generation.wrapping_add(1);
        self.capture_log.page = None;
        self.capture_log.editor.update(cx, |state, cx| {
            state.set_value("Loading capture log...", window, cx)
        });
        let requested_run = self.preferred_run.clone();
        let requested_reports = self.preferred_reports.clone();
        let previous = self
            .selected_report()
            .map(|report| (report.path.clone(), self.capture));
        self.generation += 1;
        let generation = self.generation;
        self.root = Some(root.clone());
        self.loading = true;
        self.visible_captures.clear();
        self.reset_previews(Some(window), cx);
        self.status = "Loading reports...".into();
        let suites = self.suites.clone();
        let task = cx.background_executor().spawn(async move {
            let selection = if let Some(targets) = &requested_reports {
                ReportSelection::Exact(targets)
            } else if let Some(run) = &requested_run {
                ReportSelection::Run(run)
            } else {
                ReportSelection::Latest
            };
            let mut catalog = Catalog::load_selected(&root, selection)?;
            catalog.resolve_names(&suites);
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
                        this.capture_scroll = Default::default();
                        this.rebuild_capture_filter(cx);
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
                            if this.visible_captures.contains(&capture) {
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

    fn selected_approval(&self) -> Option<ReportApproval> {
        let report = self.selected_report()?;
        let test = report.report.tests.get(self.capture?)?;
        let scenario_id = test.pair.scenario_id.as_ref().filter(|id| !id.is_empty())?;
        self.previews.get(1)?.result.as_ref().ok()?.path.as_ref()?;
        Some(ReportApproval {
            entry: report.source.clone()?,
            output: self.root.clone()?,
            suite_id: report.report.id.clone(),
            run_id: report.run_id.clone(),
            scenario_id: scenario_id.clone(),
            capture_key: test.pair.capture_key().to_string(),
        })
    }

    /// Keep selection inside the current filter without reloading unchanged images.
    fn reconcile_capture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.rebuild_capture_filter(cx);
        let filtered = &self.visible_captures;
        let selection = self
            .capture
            .filter(|capture| filtered.contains(capture))
            .or_else(|| filtered.first().copied());
        if let Some(position) =
            selection.and_then(|capture| filtered.iter().position(|index| *index == capture))
        {
            self.capture_scroll
                .scroll_to_item(position, gpui_kit::ScrollStrategy::Nearest);
        }
        if selection != self.capture {
            if let Some(capture) = selection {
                self.select_capture(capture, window, cx);
            } else {
                self.generation += 1;
                self.capture = None;
                self.reset_previews(Some(window), cx);
            }
        }
        cx.notify();
    }

    fn reset_previews(&mut self, mut window: Option<&mut Window>, cx: &mut App) {
        self.approval_confirmation = false;
        self.image_menu = None;
        self.image_task = None;
        self.image_loading = false;
        for preview in &self.previews {
            if let Ok(image) = &preview.result {
                image.image.clone().remove_asset(cx);
            }
        }
        for texture in &self.preview_textures {
            if let Some(image) = texture.0.borrow_mut().take() {
                cx.drop_image(image, window.as_deref_mut());
            }
        }
        self.previews.clear();
        for view in &mut self.preview_views {
            view.padding = None;
        }
        self.pointer_guides.pan.borrow_mut().take();
        self.pointer_guides.active.set(None);
        for bounds in &self.pointer_guides.bounds {
            bounds.set(None);
        }
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
        let scenario = (
            report.path.clone(),
            test.pair
                .scenario_id
                .as_ref()
                .filter(|id| !id.is_empty())
                .map(|id| format!("scenario:{id}"))
                .unwrap_or_else(|| format!("capture:{capture}")),
        );
        let artifacts = Arc::new(report.artifacts());
        let images = [
            ("Reference", test.pair.reference.clone()),
            ("Test", test.pair.test.clone()),
            ("Diff", test.pair.diff_image.clone()),
        ];
        if self.preview_scenario.as_ref() != Some(&scenario) {
            self.image_mode = 3;
            self.reset_view();
            self.preview_scenario = Some(scenario);
        }
        self.reset_previews(Some(window), cx);
        self.comparison_scroll.set_offset(point(px(0.), px(0.)));
        for scroll in &self.preview_scrolls {
            scroll.set_offset(point(px(0.), px(0.)));
        }
        if let Some(position) = self
            .visible_captures
            .iter()
            .position(|index| *index == capture)
        {
            self.capture_scroll
                .scroll_to_item(position, gpui_kit::ScrollStrategy::Nearest);
        }
        self.capture = Some(capture);
        if self.capture_log.active {
            self.capture_log.query.offset = 0;
            self.load_capture_log(window, cx);
        }
        self.image_loading = true;
        self.generation += 1;
        let generation = self.generation;
        let load_gate = self.image_load_gate.clone();
        self.image_task = Some(cx.spawn_in(window, async move |view, cx| {
            let mut previews = Vec::with_capacity(images.len());
            for (label, path) in images {
                let root = root.clone();
                let artifacts = artifacts.clone();
                let gate = load_gate.clone();
                let task = cx.background_executor().spawn(async move {
                    let _permit = gate.lock().await;
                    let result = LoadedImage::load(&artifacts, &root, label, path.as_deref())
                        .map_err(|error| format!("{error:#}"));
                    Preview { label, result }
                });
                previews.push(task.await);
            }
            let _ = view.update_in(cx, |this, _, cx| {
                if this.generation == generation {
                    this.previews = previews;
                    this.image_loading = false;
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    fn rebuild_capture_filter(&mut self, cx: &App) {
        let query = self.search.read(cx).value().trim().to_lowercase();
        self.visible_captures = self
            .selected_report()
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
            .unwrap_or_default();
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

    fn show_image_menu(
        &mut self,
        index: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(image) = self
            .previews
            .get(index)
            .and_then(|preview| preview.result.as_ref().ok())
        else {
            return;
        };
        let Some(path) = &image.path else {
            return;
        };
        let viewer_path = path.clone();
        let reveal_path = path.clone();
        let browser_url = reqwest::Url::from_file_path(path).ok();
        let page_url = self.selected_report().and_then(|report| {
            self.capture
                .and_then(|capture| report.report.tests.get(capture))
                .and_then(|test| image_page_url(&test.pair, index))
        });
        let browser_disabled = browser_url.is_none();
        let page_disabled = page_url.is_none();
        let copy_url = page_url.clone();
        let url_kind = if index == 0 { "Reference" } else { "Test" };
        let browser = cx.listener(move |this, _, window, cx| {
            if let Some(url) = &browser_url {
                this.open_browser(url.clone(), window, cx);
            }
        });
        let viewer = cx.listener(move |this, _, window, cx| {
            this.open_external(Ok(viewer_path.clone()), window, cx);
        });
        let page = cx.listener(move |this, _, window, cx| {
            if let Some(url) = &page_url {
                this.open_browser(url.clone(), window, cx);
            }
        });
        let focus = window.focused(cx).unwrap_or_else(|| self.focus.clone());
        let menu = PopupMenu::build(window, cx, move |menu, _, _| {
            menu.action_context(focus)
                .item(
                    PopupMenuItem::new("Open in default browser")
                        .disabled(browser_disabled)
                        .on_click(browser),
                )
                .item(PopupMenuItem::new("Open in default image viewer").on_click(viewer))
                .item(
                    PopupMenuItem::new("Show in file manager")
                        .on_click(move |_, _, cx| cx.reveal_path(&reveal_path)),
                )
                .when(index < 2, |menu| {
                    menu.separator()
                        .item(
                            PopupMenuItem::new(format!("Open {url_kind} URL"))
                                .disabled(page_disabled)
                                .on_click(page),
                        )
                        .item(
                            PopupMenuItem::new(format!("Copy {url_kind} URL"))
                                .disabled(page_disabled)
                                .on_click(move |_, _, cx| {
                                    if let Some(url) = &copy_url {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            url.to_string(),
                                        ));
                                    }
                                }),
                        )
                })
        });
        let subscription = cx.subscribe(&menu, |this, _, _: &gpui_kit::DismissEvent, cx| {
            this.image_menu = None;
            cx.notify();
        });
        menu.focus_handle(cx).focus(window, cx);
        self.image_menu = Some(ImageMenu {
            menu,
            position,
            _subscription: subscription,
        });
        self.pointer_guides.pan.borrow_mut().take();
        cx.notify();
    }

    fn open_browser(&mut self, url: reqwest::Url, window: &mut Window, cx: &mut Context<Self>) {
        let task = cx
            .background_executor()
            .spawn(async move { webbrowser::open(url.as_str()) });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |this, _, cx| {
                if let Err(error) = result {
                    this.status = format!("Cannot open browser: {error}");
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
        let filtered = &self.visible_captures;
        let list = div()
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
            .test_support()
            .key_context("RegressifyList")
            .track_focus(&self.capture_focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    window.focus(&this.capture_focus, cx);
                }),
            )
            .on_action(
                cx.listener(|this, action: &lists::MoveListSelection, window, cx| {
                    if this.loading {
                        return;
                    }
                    let current = this.capture.and_then(|capture| {
                        this.visible_captures
                            .iter()
                            .position(|index| *index == capture)
                    });
                    if let Some(position) = action.index(current, this.visible_captures.len()) {
                        this.select_capture(this.visible_captures[position], window, cx);
                    }
                }),
            )
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .relative();
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
            let empty = if self.loading {
                div()
                    .py_6()
                    .text_center()
                    .text_color(cx.theme().muted_foreground)
                    .child(message)
                    .into_any_element()
            } else {
                gpui_kit::component::empty::Empty::new()
                    .text_color(cx.theme().muted_foreground)
                    .child(message)
                    .into_any_element()
            };
            rows = rows.child(
                div()
                    .id("capture-empty")
                    .role(Role::Status)
                    .aria_label(message)
                    .test_support()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(empty),
            );
        }
        if !filtered.is_empty() {
            let scroll = self.capture_scroll.0.borrow().base_handle.clone();
            rows = rows
                .child(
                    uniform_list(
                        "capture-rows",
                        filtered.len(),
                        cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                            range
                                .map(|position| {
                                    this.render_capture_row(this.visible_captures[position], cx)
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .track_scroll(&self.capture_scroll)
                    .size_full(),
                )
                .scrollbar(&scroll, scroll::ScrollbarAxis::Vertical);
        }
        list.child(rows).into_any_element()
    }

    fn render_capture_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(test) = self
            .selected_report()
            .and_then(|report| report.report.tests.get(index))
        else {
            return div().into_any_element();
        };
        let outcome = test.outcome();
        let color = match outcome {
            Outcome::Passed => cx.theme().success,
            Outcome::Mismatch => cx.theme().warning,
            _ => cx.theme().danger,
        };
        div()
            .h(px(60.))
            .pb_1()
            .child(
                Button::new(("capture", index))
                    .ghost()
                    .w_full()
                    .h(px(56.))
                    .justify_start()
                    .selected(self.capture == Some(index))
                    .accessibility_label(format!("{} {}", outcome.label(), test.pair.display.title))
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
                                    .flex_1()
                                    .min_w_0()
                                    .text_left()
                                    .child(
                                        gpui_kit::component::label::Label::new(
                                            test.pair.display.title.clone(),
                                        )
                                        .secondary(if test.pair.display.detail.is_empty() {
                                            outcome.label().to_string()
                                        } else {
                                            format!(
                                                "{} / {}",
                                                outcome.label(),
                                                test.pair.display.detail
                                            )
                                        })
                                        .truncate(),
                                    ),
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
                        window.focus(&this.capture_focus, cx);
                        this.select_capture(index, window, cx);
                    })),
            )
            .into_any_element()
    }

    fn comparison_preview_count(&self) -> usize {
        if self
            .previews
            .get(2)
            .is_some_and(|preview| preview.result.is_ok())
        {
            3
        } else {
            2
        }
    }

    fn preview_canvas_size(&self, index: usize) -> (usize, usize) {
        let images = if self.image_mode == 3 {
            &self.previews[..self.previews.len().min(self.comparison_preview_count())]
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

    fn zoom_preview_index(&self) -> usize {
        if self.image_mode == 3 {
            self.previews
                .iter()
                .take(self.comparison_preview_count())
                .position(|preview| preview.result.is_ok())
                .unwrap_or(0)
        } else {
            self.image_mode
        }
    }

    fn render_zoom_value(&self) -> AnyElement {
        let view = self.preview_views[self.image_mode];
        let index = self.zoom_preview_index();
        let canvas_width = self.preview_canvas_size(index).0 as f32;
        let guides = self.pointer_guides.clone();
        div()
            .w(px(56.))
            .h_full()
            .flex_shrink_0()
            // Fitted bounds are only current after the image panes have been laid out.
            .child(gpui_kit::deferred(
                canvas(
                    move |bounds, window, cx| {
                        let zoom = if view.mode == ZoomMode::Manual {
                            view.zoom
                        } else {
                            guides.bounds[index]
                                .get()
                                .map(|bounds| bounds.size.width / px(canvas_width))
                                .unwrap_or(view.zoom)
                        };
                        let label = format!("{:.1}%", zoom * 100.);
                        let mut display = div()
                            .id("zoom-value")
                            .role(Role::Status)
                            .aria_label(label.clone())
                            .test_support()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(label)
                            .into_any_element();
                        display.prepaint_as_root(
                            bounds.origin,
                            bounds.size.map(gpui_kit::AvailableSpace::Definite),
                            window,
                            cx,
                        );
                        display
                    },
                    |_, mut display, window, cx| display.paint(window, cx),
                )
                .size_full(),
            ))
            .into_any_element()
    }

    fn step_zoom(&mut self, increase: bool, cx: &mut Context<Self>) {
        let index = self.zoom_preview_index();
        let view = self.preview_views[self.image_mode];
        let current = if view.mode != ZoomMode::Manual {
            self.pointer_guides.bounds[index]
                .get()
                .map(|bounds| bounds.size.width / px(self.preview_canvas_size(index).0 as f32))
                .unwrap_or(view.zoom)
        } else {
            view.zoom
        };
        let zoom = next_zoom(current, increase);
        if zoom == current {
            return;
        }
        self.preview_views[self.image_mode] = PreviewView {
            mode: ZoomMode::Manual,
            zoom,
            padding: None,
        };
        self.pointer_guides.pan.borrow_mut().take();
        cx.notify();
    }

    fn zoom_at_pointer(
        &mut self,
        index: usize,
        event: &gpui_kit::ScrollWheelEvent,
        cx: &mut Context<Self>,
    ) {
        if !event.modifiers.control {
            return;
        }
        cx.stop_propagation();
        let steps = event.delta.pixel_delta(px(40.)).y / px(40.);
        if steps == 0. || !steps.is_finite() {
            return;
        }
        let Some(bounds) = self.pointer_guides.bounds[index].get() else {
            return;
        };
        if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) {
            return;
        }
        let (width, height) = self.preview_canvas_size(index);
        let view = self.preview_views[self.image_mode];
        let scale = if view.mode != ZoomMode::Manual {
            bounds.size.width / px(width as f32)
        } else {
            view.zoom
        };
        let zoom = next_zoom(scale, steps > 0.);
        if zoom == scale {
            return;
        }
        let next_size = size(px(width as f32 * zoom), px(height as f32 * zoom));
        let scroll = if self.image_mode == 3 {
            &self.comparison_scroll
        } else {
            &self.preview_scrolls[index]
        };
        let viewport = scroll.bounds().size;
        let padding = view
            .padding
            .filter(|_| view.mode == ZoomMode::Manual)
            .map(|padding| point(padding.left, padding.top))
            .unwrap_or_else(|| {
                point(
                    (viewport.width - bounds.size.width).max(px(0.)) / 2.,
                    px(0.),
                )
            });
        let origin = bounds.origin - scroll.offset() - padding;
        let anchored = event.position
            - origin
            - point(
                (event.position.x - bounds.origin.x) * (next_size.width / bounds.size.width),
                (event.position.y - bounds.origin.y) * (next_size.height / bounds.size.height),
            );
        self.preview_views[self.image_mode] = PreviewView {
            mode: ZoomMode::Manual,
            zoom,
            padding: Some(gpui_kit::Edges {
                left: anchored.x.max(px(0.)),
                top: anchored.y.max(px(0.)),
                right: (viewport.width - anchored.x - next_size.width).max(px(0.)),
                bottom: (viewport.height - anchored.y - next_size.height).max(px(0.)),
            }),
        };
        scroll.set_offset(point(anchored.x.min(px(0.)), anchored.y.min(px(0.))));
        self.pointer_guides.pan.borrow_mut().take();
        cx.notify();
    }

    fn render_preview(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(preview) = self.previews.get(index) else {
            if self.image_loading {
                return div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .p_4()
                    .child("Loading image...")
                    .into_any_element();
            }
            return div()
                .id(("capture-preview-empty", index))
                .role(Role::Status)
                .aria_label("No capture selected")
                .test_support()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .child(
                    gpui_kit::component::empty::Empty::new()
                        .size_full()
                        .child("No capture selected"),
                )
                .into_any_element();
        };
        let content = match &preview.result {
            Ok(image) => {
                let view = self.preview_views[self.image_mode];
                let (canvas_width, canvas_height) = self.preview_canvas_size(index);
                let scroll = if self.image_mode == 3 {
                    &self.comparison_scroll
                } else {
                    &self.preview_scrolls[index]
                };
                let display = div()
                    .id(("image-content", index))
                    .role(if self.image_mode == 3 {
                        Role::Button
                    } else {
                        Role::Group
                    })
                    .aria_label(format!("{} image canvas", preview.label))
                    .test_support()
                    .relative()
                    .child(
                        img(self.preview_textures[index].source(image.image.clone()))
                            .absolute()
                            .top_0()
                            .left_0()
                            .w(relative(image.width as f32 / canvas_width as f32))
                            .h(relative(image.height as f32 / canvas_height as f32))
                            .object_fit(ObjectFit::Contain),
                    )
                    .cursor(
                        if self
                            .pointer_guides
                            .pan
                            .borrow()
                            .as_ref()
                            .is_some_and(|pan| pan.horizontal || pan.vertical)
                        {
                            CursorStyle::ClosedHand
                        } else if view.mode != ZoomMode::FitArea
                            && self.pointer_guides.bounds[index]
                                .get()
                                .is_some_and(|bounds| {
                                    bounds.size.width > scroll.bounds().size.width
                                        || bounds.size.height > scroll.bounds().size.height
                                })
                        {
                            CursorStyle::OpenHand
                        } else if self.image_mode == 3 {
                            CursorStyle::PointingHand
                        } else {
                            CursorStyle::Crosshair
                        },
                    )
                    .on_mouse_down(MouseButton::Left, {
                        let guides = self.pointer_guides.clone();
                        let scroll = scroll.clone();
                        move |event, window, cx| {
                            let Some(bounds) = guides.bounds[index].get() else {
                                return;
                            };
                            let viewport = scroll.bounds().size;
                            let horizontal = bounds.size.width > viewport.width;
                            let vertical = bounds.size.height > viewport.height;
                            *guides.pan.borrow_mut() = Some(PreviewPan {
                                position: event.position,
                                offset: scroll.offset(),
                                scroll: scroll.clone(),
                                horizontal,
                                vertical,
                                dragged: false,
                            });
                            window.refresh();
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.image_mode == 3 {
                            this.select_image_mode(index, cx);
                        }
                    }))
                    .on_mouse_move({
                        let guides = self.pointer_guides.clone();
                        let layers = self.pointer_layers.clone();
                        move |_, _, cx| {
                            guides.active.set(Some(index));
                            for layer in &layers {
                                layer.update(cx, |_, cx| cx.notify());
                            }
                        }
                    })
                    .on_hover({
                        let guides = self.pointer_guides.clone();
                        let layers = self.pointer_layers.clone();
                        move |hovered, _, cx| {
                            let active = if *hovered {
                                Some(index)
                            } else {
                                guides.active.get().filter(|active| *active != index)
                            };
                            if guides.active.replace(active) != active {
                                for layer in &layers {
                                    layer.update(cx, |_, cx| cx.notify());
                                }
                            }
                        }
                    })
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .child(self.pointer_layers[index].clone()),
                    );
                let display = if view.mode == ZoomMode::FitArea {
                    div().size_full().child(
                        canvas(
                            move |bounds, window, cx| {
                                let scale = (bounds.size.width / px(canvas_width as f32))
                                    .min(bounds.size.height / px(canvas_height as f32))
                                    .min(1.0);
                                let mut display = display
                                    .w(px(canvas_width as f32 * scale))
                                    .h(px(canvas_height as f32 * scale))
                                    .into_any_element();
                                display.prepaint_as_root(
                                    bounds.origin
                                        + point(
                                            (bounds.size.width - px(canvas_width as f32 * scale))
                                                / 2.,
                                            px(0.),
                                        ),
                                    bounds.size.map(gpui_kit::AvailableSpace::Definite),
                                    window,
                                    cx,
                                );
                                display
                            },
                            |_, mut display, window, cx| display.paint(window, cx),
                        )
                        .size_full(),
                    )
                } else if view.mode == ZoomMode::FitWidth {
                    div().w_full().flex().items_start().justify_center().child(
                        display
                            .w_full()
                            .max_w(px(canvas_width as f32))
                            .aspect_ratio(canvas_width as f32 / canvas_height as f32)
                            .flex_shrink_0(),
                    )
                } else {
                    let width = px(canvas_width as f32 * view.zoom);
                    let height = px(canvas_height as f32 * view.zoom);
                    let display = display.w(width).h(height).flex_shrink_0();
                    if let Some(padding) = view.padding {
                        div()
                            .relative()
                            .w(width + padding.left + padding.right)
                            .h(height + padding.top + padding.bottom)
                            .child(display.absolute().left(padding.left).top(padding.top))
                    } else {
                        div()
                            .w(width)
                            .min_w_full()
                            .h(height)
                            .flex()
                            .justify_center()
                            .child(display)
                    }
                }
                .id(("image-surface", index))
                .min_h_full()
                .on_scroll_wheel(cx.listener(move |this, event, _, cx| {
                    this.zoom_at_pointer(index, event, cx);
                }));
                div()
                    .id(("image-details", index))
                    .role(Role::Group)
                    .aria_label(format!(
                        "{} {} x {}",
                        preview.label, image.width, image.height
                    ))
                    .test_support()
                    .size_full()
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
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(
                                    move |this, event: &gpui_kit::MouseDownEvent, window, cx| {
                                        this.show_image_menu(index, event.position, window, cx);
                                        window.prevent_default();
                                        cx.stop_propagation();
                                    },
                                ),
                            )
                            .child(display),
                    )
                    .scrollbar(scroll, scroll::ScrollbarAxis::Both)
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

    fn render_capture_information(&self, pair: &Pair, cx: &mut Context<Self>) -> AnyElement {
        let pair = pair.clone();
        let panel = cx.entity().downgrade();
        HoverCard::new("capture-information-card")
            .anchor(gpui_kit::Anchor::TopLeft)
            .open_delay(std::time::Duration::from_millis(200))
            .close_delay(std::time::Duration::from_millis(300))
            .trigger(
                Button::new("capture-information")
                    .ghost()
                    .xsmall()
                    .icon(gpui_kit::assets::IconName::Info)
                    .accessibility_label("Scenario information"),
            )
            .content(move |_, window, cx| {
                let difference = pair.diff.as_ref();
                let dimensions = difference.and_then(|diff| diff.dimension_difference.as_ref());
                let percentage = difference
                    .and_then(|diff| {
                        if diff.mis_match_percentage.is_empty() {
                            diff.raw_mis_match_percentage
                                .map(|value| format!("{value:.3}%"))
                        } else {
                            Some(format!("{}%", diff.mis_match_percentage))
                        }
                    })
                    .unwrap_or_else(|| "Unavailable".into());
                let content = div()
                    .id("capture-information-content")
                    .role(Role::Group)
                    .aria_label("Scenario information details")
                    .test_support()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .w(px(460.))
                    .max_w(window.bounds().size.width - px(48.))
                    .text_sm();
                let selector = if pair.selector.is_empty() {
                    "Unavailable".to_string()
                } else {
                    pair.selector.clone()
                };
                let mut details = gpui_kit::component::description_list::DescriptionList::new()
                    .columns(1)
                    .bordered(false)
                    .label_width(px(112.));
                for (id, visible_label, aria_label, value) in [
                    (
                        "capture-info-label",
                        "Label:",
                        format!("Label: {}", pair.display.title),
                        pair.display.title.clone(),
                    ),
                    (
                        "capture-info-selector",
                        "Selector:",
                        format!("Selector: {selector}"),
                        selector,
                    ),
                ] {
                    details = details.item(
                        div().text_sm().child(visible_label).into_any_element(),
                        div()
                            .id(id)
                            .role(Role::Status)
                            .aria_label(aria_label)
                            .test_support()
                            .min_w_0()
                            .flex_shrink_1()
                            .truncate()
                            .child(value)
                            .into_any_element(),
                        1,
                    );
                }
                for (index, kind) in [(0, "Reference"), (1, "Test")] {
                    let url = image_page_url(&pair, index);
                    let same = index == 1 && url.is_some() && url == image_page_url(&pair, 0);
                    let value = if same {
                        "Same URL".into()
                    } else {
                        url.as_ref()
                            .map(|url| url.as_str().to_string())
                            .unwrap_or_else(|| "Unavailable".into())
                    };
                    let link_url = url.clone();
                    let copy_url = url.clone();
                    let panel = panel.clone();
                    let label = div()
                        .id(("capture-info-url-label", index))
                        .role(Role::Status)
                        .aria_label(format!("{kind} URL:"))
                        .test_support()
                        .flex_shrink_0()
                        .child(format!("{kind} URL:"))
                        .into_any_element();
                    let value = div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .min_w_0()
                        .child(
                            Button::new(("capture-info-url", index))
                                .ghost()
                                .xsmall()
                                .flex_shrink_1()
                                .min_w_0()
                                .px_0()
                                .justify_start()
                                .text_left()
                                .cursor(if url.is_some() {
                                    CursorStyle::PointingHand
                                } else {
                                    CursorStyle::Arrow
                                })
                                .accessibility_label(format!("Open {kind} URL"))
                                .tooltip(
                                    url.as_ref()
                                        .map(|url| url.as_str())
                                        .unwrap_or("URL unavailable")
                                        .to_string(),
                                )
                                .disabled(url.is_none())
                                .child(
                                    div()
                                        .id(("capture-info-url-value", index))
                                        .role(Role::Status)
                                        .aria_label(value.clone())
                                        .test_support()
                                        .min_w_0()
                                        .truncate()
                                        .text_color(cx.theme().primary)
                                        .underline()
                                        .child(value),
                                )
                                .on_click(move |_, window, cx| {
                                    if let Some(url) = &link_url {
                                        let _ = panel.update(cx, |this, cx| {
                                            this.open_browser(url.clone(), window, cx)
                                        });
                                    }
                                }),
                        )
                        .child(
                            Button::new(("capture-info-copy-url", index))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Copy)
                                .tooltip(format!("Copy {kind} URL"))
                                .accessibility_label(format!("Copy {kind} URL"))
                                .disabled(url.is_none())
                                .on_click(move |_, _, cx| {
                                    if let Some(url) = &copy_url {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            url.to_string(),
                                        ));
                                    }
                                }),
                        )
                        .into_any_element();
                    details = details.item(label, value, 1);
                }
                let mut diff_stats = div().flex().items_center().gap_3().min_w_0();
                for (id, label) in [
                    ("capture-info-diff", format!("Diff %: {percentage}")),
                    (
                        "capture-info-diff-x",
                        format!(
                            "Diff X: {}",
                            dimensions
                                .map(|value| value.width.to_string())
                                .unwrap_or_else(|| "Unavailable".into())
                        ),
                    ),
                    (
                        "capture-info-diff-y",
                        format!(
                            "Diff Y: {}",
                            dimensions
                                .map(|value| value.height.to_string())
                                .unwrap_or_else(|| "Unavailable".into())
                        ),
                    ),
                ] {
                    diff_stats = diff_stats.child(
                        div()
                            .id(id)
                            .role(Role::Status)
                            .aria_label(label.clone())
                            .test_support()
                            .min_w_0()
                            .flex_shrink_1()
                            .truncate()
                            .child(label),
                    );
                }
                details = details.item(
                    div().text_sm().child("Difference:").into_any_element(),
                    diff_stats.into_any_element(),
                    1,
                );
                content.child(details)
            })
            .into_any_element()
    }

    fn render_comparison(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.capture_log.active && self.capture.is_some() && self.selected_report().is_some() {
            return self.render_capture_log(cx);
        }
        let view = self.preview_views[self.image_mode];
        let test = self.selected_report().and_then(|report| {
            self.capture
                .and_then(|index| report.report.tests.get(index))
        });
        let title = test
            .map(|test| test.pair.display.title.clone())
            .unwrap_or_else(|| "Comparison".into());
        let previews = if self.image_mode == 3 {
            div()
                .flex()
                .size_full()
                .gap_2()
                .children(
                    (0..self.comparison_preview_count())
                        .map(|index| self.render_preview(index, cx)),
                )
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
        let approval = self.selected_approval();
        div()
            .id("comparison-view")
            .test_support()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .child(
                div()
                    .px_3()
                    .h(px(32.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .id("capture-heading")
                                    .role(Role::Heading)
                                    .aria_label(title.clone())
                                    .test_support()
                                    .min_w_0()
                                    .truncate()
                                    .child(title),
                            )
                            .when_some(test, |heading, test| {
                                heading.child(self.render_capture_information(&test.pair, cx))
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .flex_shrink_0()
                            .child(if mismatch.is_empty() {
                                String::new()
                            } else {
                                format!("{mismatch} mismatch")
                            }),
                    )
                    .when_some(test, |header, test| {
                        header
                            .child(Button::new("results-show-capture-log").ghost().xsmall().icon(gpui_kit::assets::IconName::List)
                                .tooltip("Capture Log").accessibility_label("Capture Log")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.capture_log.active = true;
                                    this.capture_log.query.offset = 0;
                                    this.load_capture_log(window, cx);
                                    cx.notify();
                                })))
                            .child(
                                Button::new("copy-capture-details")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Copy)
                                    .tooltip(format!(
                                        "Copy capture details\n\n{}",
                                        test.copy_summary()
                                    ))
                                    .accessibility_label("Copy capture details")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(test) =
                                            this.selected_report().and_then(|report| {
                                                this.capture.and_then(|index| {
                                                    report.report.tests.get(index)
                                                })
                                            })
                                        {
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
                                    .accessibility_label("Edit suite")
                                    .tooltip("Open the suite that produced this capture")
                                    .disabled(source.is_none() || self.loading)
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        if let Some(entry) = &source {
                                            cx.emit(ResultEvent::OpenSuite(entry.clone()));
                                        }
                                    })),
                            )
                    })
                    .child(
                        Button::new("approve-scenario")
                            .xsmall()
                            .flex_shrink_0()
                            .icon(gpui_kit::assets::IconName::Check)
                            .label("Approve This Scenario")
                            .tooltip("Promote this scenario's successful captures for this viewport only and recalculate the report")
                            .disabled(self.disabled || self.loading || self.image_loading || approval.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                if !this.disabled && !this.loading && !this.image_loading && this.selected_approval().is_some() {
                                    this.approval_confirmation = true;
                                    this.approval_error = None;
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .when_some(approval.filter(|_| self.approval_confirmation), |comparison, approval| {
                let viewport = approval.capture_key.strip_prefix(&approval.scenario_id)
                    .and_then(|suffix| suffix.split_once("__"))
                    .map(|(viewport, _)| viewport.trim_start_matches('_'))
                    .filter(|viewport| !viewport.is_empty())
                    .unwrap_or("Default");
                comparison.child(
                    div()
                        .id("scenario-approval-confirmation")
                        .role(Role::Group)
                        .aria_label("Scenario approval confirmation")
                        .test_support()
                        .flex()
                        .items_center()
                        .flex_wrap()
                        .flex_shrink_0()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .bg(cx.theme().warning.opacity(0.12))
                        .child(div().flex().flex_col().flex_1().min_w_0().gap_1()
                            .child(div().text_sm().child("Replace this scenario's baseline for this viewport only?"))
                            .child(div().text_xs().min_w_0().truncate().child(
                                test.map(|test| test.pair.display.title.clone()).unwrap_or_default()
                            ))
                            .child(div().id("scenario-approval-viewport").role(Role::Status)
                                .aria_label(format!("Viewport: {viewport}")).test_support()
                                .text_xs().child(format!("Viewport: {viewport}")))
                            .child(div().id("scenario-approval-run").role(Role::Status)
                                .aria_label(format!("Run: {}", approval.run_id)).test_support()
                                .text_xs().min_w_0().truncate().child(format!("Run: {}", approval.run_id))))
                        .child(Button::new("scenario-approval-cancel").small().label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.approval_confirmation = false;
                                cx.notify();
                            })))
                        .child(Button::new("scenario-approval-confirm").small().danger()
                            .icon(gpui_kit::assets::IconName::Check)
                            .label("Approve This Scenario")
                            .disabled(self.disabled || self.loading || self.image_loading)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.approval_confirmation && !this.disabled && !this.loading && !this.image_loading
                                    && let Some(approval) = this.selected_approval()
                                {
                                    this.approval_confirmation = false;
                                    cx.emit(ResultEvent::ApproveRun(approval));
                                    cx.notify();
                                }
                            })))
                )
            })
            .children(self.approval_error.as_ref().map(|error| {
                div()
                    .id("scenario-approval-error")
                    .role(Role::Alert)
                    .aria_label(error.clone())
                    .test_support()
                    .flex_shrink_0()
                    .child(
                        gpui_kit::component::marker::Marker::new()
                            .w_full()
                            .px_4()
                            .py_2()
                            .bg(cx.theme().danger.opacity(0.08))
                            .text_color(cx.theme().danger)
                            .child(Icon::new(IconName::CircleX))
                            .child(div().flex_1().min_w_0().whitespace_normal().child(error.clone())),
                    )
            }))
            .child(
                div()
                    .id("image-toolbar")
                    .test_support()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .min_w_0()
                    .h(px(36.))
                    .gap_1()
                    .px_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        TabBar::new("image-modes")
                            .underline()
                            .small()
                            .flex_shrink_0()
                            .selected_index(self.image_mode)
                            .on_click(cx.listener(|this, index, _, cx| {
                                this.select_image_mode(*index, cx);
                            }))
                            .child(Tab::new().label("Reference"))
                            .child(Tab::new().label("Test"))
                            .child(Tab::new().label("Diff"))
                            .child(Tab::new().label("Side by side")),
                    )
                    .child(div().flex_1())
                    .child(
                        gpui_kit::component::button::ButtonGroup::new("image-fit-modes")
                            .small()
                            .disabled(self.capture.is_none())
                            .child(
                                Button::new("fit-image")
                                    .label("Fit area")
                                    .selected(view.mode == ZoomMode::FitArea)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_zoom_mode(ZoomMode::FitArea, cx);
                                    })),
                            )
                            .child(
                                Button::new("fit-width")
                                    .label("Fit width")
                                    .selected(view.mode == ZoomMode::FitWidth)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_zoom_mode(ZoomMode::FitWidth, cx);
                                    })),
                            ),
                    )
                    .child(
                        Button::new("zoom-out")
                            .ghost()
                            .small()
                            .icon(gpui_kit::assets::IconName::ZoomOut)
                            .tooltip("Zoom out")
                            .accessibility_label("Zoom out")
                            .disabled(self.capture.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.step_zoom(false, cx);
                            })),
                    )
                    .child(self.render_zoom_value())
                    .child(
                        Button::new("zoom-in")
                            .ghost()
                            .small()
                            .icon(gpui_kit::assets::IconName::ZoomIn)
                            .tooltip("Zoom in")
                            .accessibility_label("Zoom in")
                            .disabled(self.capture.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.step_zoom(true, cx);
                            })),
                    )
                    .child(
                        Button::new("actual-size")
                            .ghost()
                            .small()
                            .icon(gpui_kit::assets::IconName::Scan)
                            .tooltip("Actual size (100%)")
                            .accessibility_label("Actual size (100%)")
                            .disabled(self.capture.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.preview_views[this.image_mode] = PreviewView {
                                    mode: ZoomMode::Manual,
                                    ..Default::default()
                                };
                                this.pointer_guides.pan.borrow_mut().take();
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
                            .child(
                                gpui_kit::component::marker::Marker::new()
                                    .w_full()
                                    .px_4()
                                    .py_2()
                                    .bg(cx.theme().danger.opacity(0.08))
                                    .text_color(cx.theme().danger)
                                    .child(Icon::new(IconName::CircleX))
                                    .child(div().flex_1().min_w_0().whitespace_normal().child(error.clone())),
                            )
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
        let log_warning = self.selected_report().is_some_and(|report| {
            report.report.tests.iter().any(|test| {
                [&test.pair.reference_log, &test.pair.test_log]
                    .into_iter()
                    .flatten()
                    .any(|log| !log.state.can_export())
            })
        }) || self
            .capture_log
            .page
            .as_ref()
            .is_some_and(|page| !page.state.can_export());
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
                    .px_3()
                    .py_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .id("report-title")
                            .role(Role::Status)
                            .aria_label(report_name.clone())
                            .test_support()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(report_name),
                    )
                    .child(
                        div()
                            .id("results-status")
                            .role(Role::Status)
                            .aria_label(self.status.clone())
                            .test_support()
                            .child(
                                Button::new("results-information")
                                    .ghost()
                                    .xsmall()
                                    .icon(gpui_kit::assets::IconName::Info)
                                    .accessibility_label("Report status")
                                    .tooltip(self.status.clone()),
                            ),
                    )
                    .child(
                        Button::new("refresh-results")
                            .ghost()
                            .xsmall()
                            .icon(gpui_kit::assets::IconName::RefreshCw)
                            .tooltip(format!("Refresh reports\n{counts}"))
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
                            .xsmall()
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
                            .xsmall()
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
                    ),
            )
            .when(log_warning, |panel| panel.child(div().id("results-log-health-warning").role(Role::Status)
                .aria_label("Some capture logs are incomplete or unavailable; image results are unchanged").test_support()
                .px_3().py_1().text_xs().text_color(cx.theme().warning)
                .child("Some capture logs are incomplete or unavailable; image results are unchanged")))
            .child(div().flex_1().min_h_0().child(self.render_comparison(cx)))
            .children(self.catalog.warnings.first().map(|warning| {
                div()
                    .px_4()
                    .py_2()
                    .text_xs()
                    .text_color(cx.theme().warning)
                    .child(warning.clone())
            }))
            .children(self.image_menu.as_ref().map(|menu| {
                gpui_kit::deferred(
                    gpui_kit::anchored()
                        .position(menu.position)
                        .anchor(gpui_kit::Anchor::TopLeft)
                        .snap_to_window_with_margin(px(8.))
                        .child(menu.menu.clone()),
                )
                .with_priority(gpui_kit::base::POPUP_PRIORITY)
            }))
    }
}

#[cfg(test)]
mod pointer_tests {
    use super::{PointerGuides, ZOOM_LEVELS, next_zoom};
    use gpui_kit::{Bounds, point, px, size};

    #[test]
    fn marked_reference_and_test_failures_use_the_shared_rounded_artwork() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join(".regressify");
        let suite = root.join("shop");
        let html = suite.join("html_report");
        let run = suite.join("bitmaps_test/run-1");
        std::fs::create_dir_all(&html).unwrap();
        std::fs::create_dir_all(&run).unwrap();
        std::fs::write(run.join("report.json"), r#"{"id":"shop","tests":[]}"#).unwrap();
        let legacy = lodepng::encode32(&[214_u8, 40, 40, 255], 1, 1).unwrap();
        let legacy_path = html.join("legacy-error.png");
        std::fs::write(&legacy_path, &legacy).unwrap();
        std::fs::write(
            crate::test::capture::error_marker_path(&legacy_path),
            r#"{"error":true,"message":"Capture failed"}"#,
        )
        .unwrap();
        std::fs::write(
            html.join("diff-error.png"),
            crate::test::capture::error_icon_png(),
        )
        .unwrap();
        let catalog = super::Catalog::load(&root).unwrap();
        let artifacts = catalog.reports[0].artifacts();
        let diff =
            super::LoadedImage::load(&artifacts, &root, "Diff", Some("diff-error.png")).unwrap();
        let shared = super::LoadedImage::error_placeholder().unwrap();
        for label in ["Reference", "Test"] {
            let preview =
                super::LoadedImage::load(&artifacts, &root, label, Some("legacy-error.png"))
                    .unwrap();
            assert_eq!((preview.width, preview.height), (diff.width, diff.height));
            assert_eq!(
                (preview.width, preview.height),
                (shared.width, shared.height)
            );
            assert!(preview.path.is_none());
        }
        assert!(std::fs::read(&legacy_path).unwrap() == legacy);
        std::fs::write(
            crate::test::capture::error_marker_path(&legacy_path),
            r#"{"error":false,"message":""}"#,
        )
        .unwrap();
        let valid =
            super::LoadedImage::load(&artifacts, &root, "Test", Some("legacy-error.png")).unwrap();
        assert_eq!((valid.width, valid.height), (1, 1));
        assert!(valid.path.is_some());
    }

    #[test]
    fn reference_placeholder_only_replaces_missing_reference_images() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join(".regressify");
        let suite = root.join("shop");
        let html = suite.join("html_report");
        let run = suite.join("bitmaps_test/run-1");
        std::fs::create_dir_all(&html).unwrap();
        std::fs::create_dir_all(&run).unwrap();
        std::fs::write(run.join("report.json"), r#"{"id":"shop","tests":[]}"#).unwrap();
        let png = lodepng::encode32(&[30_u8, 150, 180, 255], 1, 1).unwrap();
        std::fs::write(html.join("valid.png"), &png).unwrap();
        std::fs::write(html.join("invalid.png"), "not a PNG").unwrap();
        std::fs::write(directory.path().join("outside.png"), &png).unwrap();
        let catalog = super::Catalog::load(&root).unwrap();
        let artifacts = catalog.reports[0].artifacts();
        let placeholder = lodepng::decode32(crate::test::capture::error_icon_png()).unwrap();

        for path in [None, Some("../bitmaps_reference/missing.png")] {
            let image = super::LoadedImage::load(&artifacts, &root, "Reference", path).unwrap();
            assert_eq!(
                (image.width, image.height),
                (placeholder.width, placeholder.height)
            );
            assert!(image.path.is_none());
            for label in ["Test", "Diff"] {
                assert!(super::LoadedImage::load(&artifacts, &root, label, path).is_err());
            }
        }
        for label in ["Reference", "Test", "Diff"] {
            let image =
                super::LoadedImage::load(&artifacts, &root, label, Some("valid.png")).unwrap();
            assert_eq!((image.width, image.height), (1, 1));
            assert_eq!(
                image.path,
                Some(html.join("valid.png").canonicalize().unwrap())
            );
        }
        for (path, message) in [
            ("invalid.png", "PNG header"),
            ("/absolute.png", "relative file path"),
            ("../../../outside.png", "escapes the output directory"),
        ] {
            let error = match super::LoadedImage::load(&artifacts, &root, "Reference", Some(path)) {
                Ok(_) => panic!("Invalid reference must not become a placeholder: {path}"),
                Err(error) => error,
            };
            assert!(format!("{error:#}").contains(message));
        }
        assert!(!suite.join("bitmaps_reference").exists());
        assert_eq!(std::fs::read(html.join("valid.png")).unwrap(), png);
    }

    #[test]
    fn image_urls_use_each_recorded_initial_url_without_cross_fallback() {
        let mut pair: crate::desktop::results::Pair = serde_json::from_value(serde_json::json!({
            "referenceUrl": "https://production.example.test/start?source=reference",
            "url": "https://staging.example.test/start?source=test"
        }))
        .unwrap();
        pair.display.page_url = Some("https://current-workspace.example.test/changed".into());
        assert_eq!(
            super::image_page_url(&pair, 0).unwrap().as_str(),
            pair.reference_url.as_ref().unwrap()
        );
        assert_eq!(
            super::image_page_url(&pair, 1).unwrap().as_str(),
            pair.url.as_ref().unwrap()
        );
        assert!(super::image_page_url(&pair, 2).is_none());
        pair.reference_url = None;
        assert!(super::image_page_url(&pair, 0).is_none());
        assert!(super::image_page_url(&pair, 1).is_some());
        for invalid in [
            "",
            "relative/path",
            "javascript:alert(1)",
            "data:text/html,test",
            "vscode://file/private",
        ] {
            pair.url = Some(invalid.into());
            assert!(super::image_page_url(&pair, 1).is_none());
        }
        pair.url = Some("file:///tmp/scenario.html".into());
        assert_eq!(super::image_page_url(&pair, 1).unwrap().scheme(), "file");
        pair.url = None;
        assert!(super::image_page_url(&pair, 1).is_none());
    }

    #[test]
    fn zoom_steps_follow_chrome_presets_and_snap_in_the_requested_direction() {
        for levels in ZOOM_LEVELS.windows(2) {
            assert_eq!(next_zoom(levels[0], true), levels[1]);
            assert_eq!(next_zoom(levels[1], false), levels[0]);
        }
        for (current, smaller, larger) in [
            (0.42, 1. / 3., 0.5),
            (0.68, 2. / 3., 0.75),
            (0.92, 0.9, 1.),
            (1.17, 1.1, 1.25),
            (1.5625, 1.5, 1.75),
        ] {
            assert_eq!(next_zoom(current, false), smaller);
            assert_eq!(next_zoom(current, true), larger);
        }
        assert_eq!(next_zoom(0.25, false), 0.25);
        assert_eq!(next_zoom(5., true), 5.);
        assert_eq!(next_zoom(0.1, true), 0.25);
        assert_eq!(next_zoom(0.1, false), 0.1);
        assert_eq!(next_zoom(1.0005, true), 1.1);
        assert_eq!(next_zoom(0.9995, false), 0.9);
    }

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
    async fn comparison_completion_replaces_previously_opened_run(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        check_comparison_completion(cx, false, crate::desktop::runner::RunOutcome::Completed).await;
    }

    #[cfg(feature = "gui-tests")]
    #[gpui_kit::test]
    async fn comparison_completion_replaces_saved_selection_in_requested_output(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        check_comparison_completion(cx, true, crate::desktop::runner::RunOutcome::Completed).await;
    }

    #[cfg(feature = "gui-tests")]
    #[gpui_kit::test]
    async fn comparison_completion_failed_command_keeps_reviewed_run(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        check_comparison_completion(
            cx,
            false,
            crate::desktop::runner::RunOutcome::Failed("Report could not be written".into()),
        )
        .await;
    }

    #[cfg(feature = "gui-tests")]
    #[gpui_kit::test]
    async fn comparison_completion_cancelled_command_keeps_reviewed_run(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        check_comparison_completion(cx, false, crate::desktop::runner::RunOutcome::Cancelled).await;
    }

    #[cfg(feature = "gui-tests")]
    async fn check_comparison_completion(
        cx: &mut gpui_kit::TestAppContext,
        saved_selection: bool,
        outcome: crate::desktop::runner::RunOutcome,
    ) {
        use crate::desktop::runner::{RunCommand, RunOptions, RunOutcome, RunRequest};
        use crate::progress::{Event, LiveProgress, Record};
        use gpui_kit::AppContext;
        use gpui_kit::test::{TestAppContextExt, TestWindowExt};
        use std::time::Duration;

        let completed = matches!(outcome, RunOutcome::Completed);
        let directory = tempfile::tempdir().unwrap();
        let (handle, view) = crate::desktop::ui::tests::editor(cx, directory.path());
        let (results, runs, suites) = cx.update(|cx| {
            let editor = view.read(cx);
            (
                editor.results.clone(),
                editor.runs.clone(),
                editor.session.suites.clone(),
            )
        });
        let scenario_id = suites[0].scenarios[0].id.clone();
        let root = directory.path().canonicalize().unwrap();
        let output = root.join(".regressify");
        let compared_output = if saved_selection {
            root.join("comparison-output")
        } else {
            output.clone()
        };
        for run_id in ["previous", "compared"] {
            let selected_output = if run_id == "compared" {
                &compared_output
            } else {
                &output
            };
            let run = selected_output.join("ui-test/bitmaps_test").join(run_id);
            std::fs::create_dir_all(&run).unwrap();
            let tests = [
                "$load-page",
                "$input-username",
                "$input-email",
                "$input-password",
                "FINAL",
            ]
            .iter()
            .enumerate()
            .map(|(index, capture)| {
                serde_json::json!({
                    "status": if run_id == "compared" && index == 1 { "fail" } else { "pass" },
                    "pair": {
                        "scenarioId": scenario_id,
                        "scenarioLabel": "Checkout",
                        "fileName": format!("{scenario_id}_desktop__{capture}.png"),
                    },
                })
            })
            .collect::<Vec<_>>();
            std::fs::write(
                run.join("report.json"),
                serde_json::to_vec(&serde_json::json!({
                    "id": "ui-test", "testSuite": "ui-test", "tests": tests,
                }))
                .unwrap(),
            )
            .unwrap();
        }
        let unrelated = compared_output.join("unrelated/bitmaps_test/compared");
        std::fs::create_dir_all(&unrelated).unwrap();
        std::fs::write(
            unrelated.join("report.json"),
            serde_json::to_vec(&serde_json::json!({
                "id": "unrelated", "tests": [{
                    "status": "pass", "pair": { "fileName": "other_desktop__FINAL.png" },
                }],
            }))
            .unwrap(),
        )
        .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            results.update(cx, |panel, cx| {
                panel.set_suites(suites);
                if saved_selection {
                    panel.show_saved_reports(
                        output.clone(),
                        vec![("ui-test".into(), "previous".into())],
                        window,
                        cx,
                    );
                } else {
                    panel.show_run(output.clone(), "previous".into(), window, cx);
                }
            });
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
            let panel = results.read(cx);
            !panel.loading && panel.selected_report().is_some()
        })
        .await;
        let generation = cx.update(|cx| {
            let panel = results.read(cx);
            assert_eq!(panel.selected_report().unwrap().counts(), (5, 0, 0));
            panel.generation
        });
        cx.update(|cx| {
            view.update(cx, |editor, _| editor.main_tab = 1);
            runs.update(cx, |panel, cx| {
                panel.manager.request = Some(RunRequest {
                    root,
                    command: RunCommand::Compare,
                    options: RunOptions {
                        suites: vec!["ui-test".into()],
                        run_id: "compared".into(),
                        regressify_dir: if saved_selection {
                            "comparison-output".into()
                        } else {
                            ".regressify".into()
                        },
                        ..Default::default()
                    },
                });
                panel.manager.outcome = Some(outcome);
                panel.manager.progress = LiveProgress::default();
                panel.manager.progress.apply(Record {
                    version: 1,
                    elapsed_ms: 1,
                    event: Event::Compared {
                        suite_id: "ui-test".into(),
                        capture: format!("{scenario_id}_desktop__$input-username"),
                        label: "Checkout".into(),
                        scenario_id: Some(scenario_id),
                        passed: false,
                        error: Some("Visual mismatch".into()),
                    },
                });
                panel.manager.progress.end(false, None);
                cx.emit(crate::desktop::ui::runs::RunEvent::Finished);
            });
        });
        cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
            let panel = results.read(cx);
            panel.generation > generation && !panel.loading
        })
        .await;
        cx.update(|cx| {
            let panel = results.read(cx);
            assert_eq!(panel.catalog.reports.len(), 1);
            let report = panel.selected_report().unwrap();
            assert_eq!(
                report.run_id,
                if completed { "compared" } else { "previous" }
            );
            assert_eq!(
                report.counts(),
                if completed { (4, 1, 0) } else { (5, 0, 0) }
            );
            assert_eq!(
                report.report.scenario_counts().failed,
                usize::from(completed)
            );
            assert_eq!(
                panel.root.as_ref(),
                Some(if completed { &compared_output } else { &output })
            );
            assert_eq!(view.read(cx).main_tab, 1);
            view.update(cx, |editor, cx| {
                editor.main_tab = 2;
                cx.notify();
            });
        });
        cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
            window
                .try_find(("capture", 1_usize))
                .is_some_and(|capture| {
                    capture.label()
                        == Some(if completed {
                            "Mismatch Checkout"
                        } else {
                            "Passed Checkout"
                        })
                })
        })
        .await;
    }

    #[cfg(feature = "gui-tests")]
    #[gpui_kit::test]
    async fn approval_completion_preserves_comparison_and_updates_suite_counts(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use crate::desktop::runner::{RunCommand, RunOptions, RunOutcome, RunRequest};
        use crate::progress::{Event, LiveProgress, Record};
        use gpui_kit::AppContext;
        use gpui_kit::test::{TestAppContextExt, TestWindowExt};
        use std::time::Duration;

        let directory = tempfile::tempdir().unwrap();
        let (handle, view) = crate::desktop::ui::tests::editor(cx, directory.path());
        let (results, runs, scenario_id, suite_index) = cx.update(|cx| {
            let editor = view.read(cx);
            (
                editor.results.clone(),
                editor.runs.clone(),
                editor.session.suites[0].scenarios[0].id.clone(),
                editor
                    .session
                    .entries
                    .iter()
                    .position(|entry| entry.suite_name().as_deref() == Some("ui-test"))
                    .unwrap(),
            )
        });
        let output = directory.path().join("review-output");
        let suite = output.join("ui-test");
        let run = suite.join("bitmaps_test/reviewed");
        let reference = suite.join("bitmaps_reference");
        std::fs::create_dir_all(&run).unwrap();
        std::fs::create_dir_all(&reference).unwrap();
        let mut metadata = serde_json::Map::new();
        for viewport in ["desktop", "phone"] {
            let key = format!("{scenario_id}_{viewport}__FINAL");
            lodepng::encode32_file(run.join(format!("{key}.png")), &[200_u8, 20, 30, 255], 1, 1)
                .unwrap();
            lodepng::encode32_file(
                reference.join(format!("{key}.png")),
                &[10_u8, 20, 30, 255],
                1,
                1,
            )
            .unwrap();
            metadata.insert(key, serde_json::json!({
                "scenarioId": scenario_id, "scenarioLabel": "Checkout",
                "referenceUrl": "https://production.example.test", "url": "https://staging.example.test",
            }));
        }
        std::fs::write(
            run.join(".report-urls.json"),
            serde_json::to_vec(&serde_json::json!({
                "version": 1, "captures": metadata,
            }))
            .unwrap(),
        )
        .unwrap();
        crate::compare::report_gen::generate_comparison_report(
            &suite, "ui-test", "reviewed", 0.1, 1,
        )
        .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |editor, _| editor.main_tab = 2);
            runs.update(cx, |panel, cx| {
                panel.manager.request = Some(RunRequest {
                    root: directory.path().canonicalize().unwrap(),
                    command: RunCommand::Test,
                    options: RunOptions {
                        suites: vec!["ui-test".into()],
                        ..Default::default()
                    },
                });
                panel.manager.progress = LiveProgress::default();
                for viewport in ["desktop", "phone"] {
                    panel.manager.progress.apply(Record {
                        version: 1,
                        elapsed_ms: 1,
                        event: Event::Compared {
                            suite_id: "ui-test".into(),
                            capture: format!("{scenario_id}_{viewport}__FINAL"),
                            label: "Checkout".into(),
                            scenario_id: Some(scenario_id.clone()),
                            passed: false,
                            error: Some("Visual mismatch".into()),
                        },
                    });
                }
                panel.manager.progress.end(false, None);
                panel.manager.outcome = Some(RunOutcome::Completed);
                cx.notify();
            });
            let suites = view.read(cx).session.suites.clone();
            results.update(cx, |panel, cx| {
                panel.set_suites(suites);
                panel.show_run(output.clone(), "reviewed".into(), window, cx);
            });
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
            let panel = results.read(cx);
            panel.capture == Some(0) && !panel.image_loading && !panel.previews.is_empty()
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            assert_eq!(
                window.find(("suite-activity", suite_index)).label(),
                Some("2 failed / 2 finished")
            );
            window.click(("capture", 1_usize), cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
            let panel = results.read(cx);
            panel.capture == Some(1) && !panel.image_loading
        })
        .await;
        let request = cx
            .update_window(handle.into(), |_, window, cx| {
                window.within("image-modes").click(1_usize, cx);
                window.click("actual-size", cx);
                window.click("zoom-in", cx);
                window.click("approve-scenario", cx);
                assert_eq!(view.read(cx).main_tab, 2);
                assert_eq!(
                    window.find("scenario-approval-viewport").label(),
                    Some("Viewport: phone")
                );
                let approval = results.read(cx).selected_approval().unwrap();
                let intent = runs.read(cx).report_approval_intent(&approval).unwrap();
                assert!(!intent.review_after_run);
                window.click("scenario-approval-cancel", cx);
                intent.resolve(&view.read(cx).session.suites).unwrap()
            })
            .unwrap();
        crate::compare::report_gen::approve_scenario_viewport(
            &suite,
            "reviewed",
            &scenario_id,
            &format!("{scenario_id}_phone__FINAL"),
        )
        .unwrap();
        cx.update(|cx| {
            runs.update(cx, |panel, cx| {
                panel.manager.request = Some(request);
                panel.manager.progress = LiveProgress::default();
                panel.manager.progress.end(false, None);
                panel.manager.outcome = Some(RunOutcome::Completed);
                cx.emit(crate::desktop::ui::runs::RunEvent::Finished);
            })
        });
        cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
            window
                .try_find(("suite-activity", suite_index))
                .is_some_and(|status| status.label() == Some("1 failed / 2 finished"))
                && window.try_find(("image-details", 1_usize)).is_some()
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            assert_eq!(view.read(cx).main_tab, 2);
            assert_eq!(results.read(cx).capture, Some(1));
            assert_eq!(results.read(cx).root.as_ref(), Some(&output));
            assert_eq!(
                window.within("image-modes").find(1_usize).selected(),
                Some(true)
            );
            assert_eq!(window.find("zoom-value").label(), Some("110.0%"));
            assert_eq!(
                window.find(("capture", 1_usize)).label(),
                Some("Passed Checkout")
            );
            runs.update(cx, |panel, cx| {
                panel.manager.outcome =
                    Some(RunOutcome::Failed("Report could not be written".into()));
                cx.emit(crate::desktop::ui::runs::RunEvent::Finished);
            });
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
            window
                .try_find("scenario-approval-error")
                .is_some_and(|error| {
                    error.label() == Some("Approval failed: Report could not be written")
                })
                && window.try_find(("image-details", 1_usize)).is_some()
        })
        .await;
        cx.update(|cx| assert_eq!(view.read(cx).main_tab, 2));
    }

    #[cfg(feature = "gui-tests")]
    #[gpui_kit::test]
    async fn scenario_approval_tracks_viewport_and_rejects_unavailable_captures(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use gpui_kit::AppContext;
        use gpui_kit::test::{TestAppContextExt, TestWindowExt};
        use std::time::Duration;

        let directory = tempfile::tempdir().unwrap();
        let (handle, view) = crate::desktop::ui::tests::editor(cx, directory.path());
        let (results, scenario_id) = cx.update(|cx| {
            (
                view.read(cx).results.clone(),
                view.read(cx).session.suites[0].scenarios[0].id.clone(),
            )
        });
        let run = directory
            .path()
            .join(".regressify/ui-test/bitmaps_test/reviewed");
        std::fs::create_dir_all(&run).unwrap();
        let mut captures = Vec::new();
        for (index, viewport) in ["desktop", "mobile", "failed", "legacy", "missing"]
            .iter()
            .enumerate()
        {
            let filename = format!("{scenario_id}_{viewport}__FINAL.png");
            if index != 4 {
                lodepng::encode32_file(run.join(&filename), &[30_u8, 150, 180, 255], 1, 1).unwrap();
            }
            if index == 2 {
                std::fs::write(
                    crate::test::capture::error_marker_path(&run.join(&filename)),
                    r#"{"error":true,"message":"Capture failed"}"#,
                )
                .unwrap();
            }
            captures.push(serde_json::json!({
                "status": "fail", "pair": {
                    "label": viewport, "fileName": filename,
                    "scenarioId": (index != 3).then_some(&scenario_id),
                    "test": format!("../bitmaps_test/reviewed/{filename}")
                }
            }));
        }
        std::fs::write(
            run.join("report.json"),
            serde_json::to_vec(&serde_json::json!({
                "id": "ui-test", "tests": captures,
            }))
            .unwrap(),
        )
        .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.within("workbench-tabs").click(2_usize, cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
            let panel = results.read(cx);
            panel.capture == Some(0) && !panel.image_loading && !panel.previews.is_empty()
        })
        .await;
        for (index, viewport) in ["desktop", "mobile", "failed", "legacy", "missing"]
            .iter()
            .enumerate()
        {
            cx.update_window(handle.into(), |_, window, cx| {
                results.update(cx, |panel, cx| panel.select_capture(index, window, cx));
                if index != 0 {
                    assert!(results.read(cx).selected_approval().is_none());
                    assert!(!results.read(cx).approval_confirmation);
                }
            })
            .unwrap();
            cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
                let panel = results.read(cx);
                panel.capture == Some(index) && !panel.image_loading
            })
            .await;
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                assert!(window.try_find("approve-report").is_none());
                let approval = results.read(cx).selected_approval();
                assert_eq!(approval.is_some(), index < 2);
                if let Some(approval) = approval {
                    assert_eq!(approval.scenario_id, scenario_id);
                    assert_eq!(
                        approval.capture_key,
                        format!("{scenario_id}_{viewport}__FINAL")
                    );
                    assert_eq!(approval.run_id, "reviewed");
                    window.click("approve-scenario", cx);
                    assert_eq!(view.read(cx).main_tab, 2);
                    assert!(window.try_find("scenario-approval-confirm").is_some());
                    assert_eq!(
                        window.find("scenario-approval-viewport").label(),
                        Some(format!("Viewport: {viewport}").as_str())
                    );
                    if index == 0 {
                        results.update(cx, |panel, cx| panel.set_disabled(true, cx));
                        window.render_frame(cx);
                        assert!(window.try_find("scenario-approval-confirm").is_none());
                        results.update(cx, |panel, cx| panel.set_disabled(false, cx));
                        window.click("approve-scenario", cx);
                    }
                } else {
                    window.click("approve-scenario", cx);
                    assert_eq!(view.read(cx).main_tab, 2);
                    assert!(window.try_find("scenario-approval-confirm").is_none());
                }
            })
            .unwrap();
        }
    }

    #[cfg(feature = "gui-tests")]
    #[gpui_kit::test]
    async fn preview_loading_keeps_only_the_latest_queued_capture(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use gpui_kit::AppContext;
        use gpui_kit::test::{TestAppContextExt, TestWindowExt};
        use std::{sync::Arc, time::Duration};

        let directory = tempfile::tempdir().unwrap();
        let run = directory
            .path()
            .join(".regressify/ui-test/bitmaps_test/run-1");
        std::fs::create_dir_all(&run).unwrap();
        lodepng::encode32_file(run.join("capture.png"), &[30_u8, 150, 180, 255], 1, 1).unwrap();
        let captures = (0..40).map(|index| serde_json::json!({
            "status": "pass", "pair": {"label": format!("Capture {index}"), "test": "../bitmaps_test/run-1/capture.png"}
        })).collect::<Vec<_>>();
        std::fs::write(
            run.join("report.json"),
            serde_json::to_vec(&serde_json::json!({
                "id": "ui-test", "tests": captures
            }))
            .unwrap(),
        )
        .unwrap();
        let (handle, view) = crate::desktop::ui::tests::editor(cx, directory.path());
        let results = cx.update(|cx| view.read(cx).results.clone());
        let gate = cx.update(|cx| results.read(cx).image_load_gate.clone());
        let permit = gate.lock().await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.within("workbench-tabs").click(2_usize, cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
            window
                .try_find("capture-heading")
                .is_some_and(|heading| heading.label() == Some("Capture 0"))
        })
        .await;
        for index in 1..40 {
            cx.update_window(handle.into(), |_, window, cx| {
                results.update(cx, |panel, cx| panel.select_capture(index, window, cx));
            })
            .unwrap();
            cx.run_until_parked();
            assert!(
                Arc::strong_count(&gate) <= 4,
                "Superseded image tasks must be dropped"
            );
        }
        drop(permit);
        cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
            window.try_find(("image-details", 1_usize)).is_some()
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            assert_eq!(window.find("capture-heading").label(), Some("Capture 39"));
            results.update(cx, |panel, cx| panel.clear(window, cx));
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(Arc::strong_count(&gate), 2);
        cx.update(|cx| {
            assert!(results.read(cx).image_task.is_none());
            assert!(!results.read(cx).image_loading);
            assert!(results.read(cx).previews.is_empty());
        });
    }

    #[cfg(feature = "gui-tests")]
    #[gpui_kit::test]
    async fn crosshairs_follow_either_pane_and_clear_on_exit(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::test::{TestAppContextExt, TestWindowExt};
        use gpui_kit::{AppContext, InputEvent as _};
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
            let filtered = results.read(cx).visible_captures.as_ptr();
            for _ in 0..20 {
                window.render_frame(cx);
                assert_eq!(results.read(cx).visible_captures.as_ptr(), filtered);
                assert_eq!(results.read(cx).visible_captures.as_slice(), &[0]);
            }
            for id in [
                "previous-capture",
                "next-capture",
                "previous-failed-capture",
                "next-failed-capture",
                "previous-captures",
                "next-captures",
                "capture-position",
            ] {
                assert!(window.try_find(id).is_none());
            }
            let assert_position =
                |actual: Option<gpui_kit::Point<gpui_kit::Pixels>>,
                 expected: gpui_kit::Point<gpui_kit::Pixels>| {
                    let actual = actual.expect("Pointer guide must be visible");
                    assert!((actual.x - expected.x).abs() < px(0.01));
                    assert!((actual.y - expected.y).abs() < px(0.01));
                };
            for index in [0_usize, 1] {
                window.hover(("image-scroll", index), cx);
                window.render_frame(cx);
                let guides = results.read(cx).pointer_guides.clone();
                assert_eq!(guides.active.get(), Some(index));
                let mouse = window.mouse_position();
                let source = guides.bounds[index].get().unwrap();
                let target = guides.bounds[1 - index].get().unwrap();
                assert_position(guides.position(index, mouse), mouse);
                assert_position(
                    guides.position(1 - index, mouse),
                    mouse - source.origin + target.origin,
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
                assert_position(
                    guides.position(1 - index, mouse),
                    mouse - source.origin + target.origin,
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
        cx.run_until_parked();
        let results = cx.update(|cx| view.read(cx).results.clone());
        let notifications = std::rc::Rc::new(std::cell::Cell::new(0));
        let observed = notifications.clone();
        let _subscription = cx.update(|cx| {
            cx.observe(&results, move |_, _| {
                observed.set(observed.get() + 1);
            })
        });
        for index in [0_usize, 1] {
            cx.update_window(handle.into(), |_, window, cx| {
                window.hover(("image-scroll", index), cx);
            })
            .unwrap();
            cx.run_until_parked();
            notifications.set(0);
            for step in 1..=20 {
                cx.update_window(handle.into(), |_, window, cx| {
                    let origin = window.find(("image-scroll", index)).bounds().center();
                    window.dispatch_event(
                        gpui_kit::MouseMoveEvent {
                            position: origin + point(px(step as f32), px(step as f32)),
                            pressed_button: None,
                            modifiers: Default::default(),
                        }
                        .to_platform_input(),
                        cx,
                    );
                    window.render_frame(cx);
                })
                .unwrap();
                cx.run_until_parked();
            }
            assert_eq!(
                notifications.get(),
                0,
                "Pointer movement within pane {index} must not invalidate Results or its navigation observers"
            );
        }
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("fit-image", cx);
            for index in [0_usize, 1] {
                window.hover(("image-content", index), cx);
                window.render_frame(cx);
                let guides = results.read(cx).pointer_guides.clone();
                assert_eq!(guides.active.get(), Some(index));
                let mouse = window.mouse_position();
                let source = window.find(("image-content", index)).bounds();
                let target = window.find(("image-content", 1 - index)).bounds();
                assert_eq!(guides.bounds[index].get(), Some(source));
                assert_eq!(guides.position(index, mouse), Some(mouse));
                assert_eq!(
                    guides.position(1 - index, mouse),
                    Some(mouse - source.origin + target.origin)
                );
            }
            window.hover("actual-size", cx);
            window.render_frame(cx);
            assert_eq!(results.read(cx).pointer_guides.active.get(), None);
        })
        .unwrap();
        let released = cx
            .update_window(handle.into(), |_, window, cx| {
                let source = gpui_kit::ImageSource::from(
                    results.read(cx).previews[1]
                        .result
                        .as_ref()
                        .unwrap()
                        .image
                        .clone(),
                );
                let decoded = results.read(cx).preview_textures[1]
                    .0
                    .borrow()
                    .as_ref()
                    .map(std::sync::Arc::downgrade)
                    .expect("Displayed image should be decoded");
                assert!(
                    source.is_asset_cached(cx),
                    "Displayed images should have reached GPUI's decoder cache"
                );
                results.update(cx, |panel, cx| panel.clear(window, cx));
                assert!(
                    !source.is_asset_cached(cx),
                    "Clearing previews must evict their decoded image assets"
                );
                assert!(
                    results
                        .read(cx)
                        .preview_textures
                        .iter()
                        .all(|texture| texture.0.borrow().is_none())
                );
                window.render_frame(cx);
                decoded
            })
            .unwrap();
        cx.run_until_parked();
        assert!(
            released.upgrade().is_none(),
            "Decoded pixels must be freed after obsolete image frames are dropped"
        );
    }

    #[cfg(feature = "gui-tests")]
    #[gpui_kit::test]
    fn released_results_evict_preview_assets(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::AppContext;
        use std::sync::Arc;

        let directory = tempfile::tempdir().unwrap();
        let (handle, _) = crate::desktop::ui::tests::editor(cx, directory.path());
        let encoded = Arc::new(gpui_kit::Image::from_bytes(
            gpui_kit::ImageFormat::Png,
            lodepng::encode32(&[30_u8, 150, 180, 255], 1, 1).unwrap(),
        ));
        let panel = cx
            .update_window(handle.into(), |_, window, cx| {
                let panel = cx.new(|cx| super::ResultsPanel::new(window, cx));
                panel.update(cx, |panel, _| {
                    panel.previews.push(super::Preview {
                        label: "Current",
                        result: Ok(super::LoadedImage {
                            image: encoded.clone(),
                            width: 1,
                            height: 1,
                            path: Some(directory.path().join("fixture.png")),
                        }),
                    })
                });
                encoded.clone().get_render_image(window, cx);
                assert!(encoded.is_asset_cached(cx));
                panel
            })
            .unwrap();
        drop(panel);
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(
                !encoded.is_asset_cached(cx),
                "Releasing Results must remove its preview assets"
            )
        });
    }
}
