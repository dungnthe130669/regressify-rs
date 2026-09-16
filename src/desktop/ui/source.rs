//! Adapt local schema suggestions to the editor's native completion menu.

use std::{rc::Rc, sync::Arc};

use gpui_kit::component::input::{CompletionProvider, Rope};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Documentation, Position, TextEdit,
};

use super::*;
use crate::desktop::{completion::CompletionCatalog, document::Format};

#[derive(Clone)]
struct SourceCompletions {
    catalog: Arc<CompletionCatalog>,
    kind: DocumentKind,
    format: Format,
}

impl SourceCompletions {
    fn items(&self, text: &str, cursor: usize) -> Vec<CompletionItem> {
        self.catalog
            .complete(self.kind, self.format, text, cursor)
            .into_iter()
            .map(|item| CompletionItem {
                label: item.label,
                detail: Some(item.detail),
                documentation: (!item.description.is_empty())
                    .then_some(Documentation::String(item.description)),
                kind: Some(if item.property {
                    CompletionItemKind::PROPERTY
                } else {
                    CompletionItemKind::VALUE
                }),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: lsp_types::Range {
                        start: position(text, item.replacement.start),
                        end: position(text, item.replacement.end),
                    },
                    new_text: item.text,
                })),
                ..Default::default()
            })
            .collect()
    }
}

impl CompletionProvider for SourceCompletions {
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        _: CompletionContext,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<Result<CompletionResponse>> {
        let provider = self.clone();
        let text = text.clone();
        cx.background_executor().spawn(async move {
            Ok(CompletionResponse::Array(
                provider.items(&text.to_string(), offset),
            ))
        })
    }

    fn is_completion_trigger(&self, _: usize, text: &str, _: &mut App) -> bool {
        !text.is_empty()
            && text.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '"' | ':' | ' ')
            })
    }
}

fn position(text: &str, offset: usize) -> Position {
    let before = &text[..offset];
    let line = before.bytes().filter(|byte| *byte == b'\n').count();
    let column = before
        .rsplit('\n')
        .next()
        .unwrap_or_default()
        .encode_utf16()
        .count();
    Position::new(line as u32, column as u32)
}

impl Workbench {
    fn route_completion<Command: Action + Clone>(
        &mut self,
        action: &Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.busy && self.source.read(cx).completion_menu_state().open {
            let handled = self.source.update(cx, |state, cx| {
                state.route_overlay_action(Box::new(action.clone()), window, cx)
            });
            if handled {
                cx.stop_propagation();
            }
        }
    }

    pub(super) fn request_completion(
        &mut self,
        _: &CompleteSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.tab != 1 {
            return;
        }
        let state = self.source.read(cx);
        let Some(provider) = state.lsp().completion_provider.clone() else {
            return;
        };
        let text = state.text().clone();
        let cursor = state.cursor();
        let document = self
            .session
            .open
            .as_ref()
            .map(|open| open.entry.path.clone());
        window.focus(&state.focus_handle(cx), cx);
        let task = provider.completions(
            &text,
            cursor,
            CompletionContext {
                trigger_kind: lsp_types::CompletionTriggerKind::INVOKED,
                trigger_character: None,
            },
            window,
            cx,
        );
        cx.spawn_in(window, async move |view, cx| {
            let Ok(response) = task.await else {
                return;
            };
            let items = match response {
                CompletionResponse::Array(items) => items,
                CompletionResponse::List(list) => list.items,
            };
            let _ = view.update_in(cx, |this, window, cx| {
                if this.session.open.as_ref().map(|open| &open.entry.path) != document.as_ref()
                    || this.tab != 1
                {
                    return;
                }
                this.source.update(cx, |state, cx| {
                    if state.cursor() == cursor
                        && state.text() == &text
                        && state.focus_handle(cx).is_focused(window)
                    {
                        state.present_completion_items(cursor, "", items, cx);
                    }
                });
            });
        })
        .detach();
    }

    pub(super) fn configure_source(&mut self, cx: &mut Context<Self>) {
        let Some(open) = &self.session.open else {
            return;
        };
        let catalog = match CompletionCatalog::new() {
            Ok(catalog) => Arc::new(catalog),
            Err(error) => {
                self.status = format!("Completion unavailable: {error:#}");
                return;
            }
        };
        let language = if open.document.format == Format::Json {
            "json"
        } else {
            "yaml"
        };
        let provider = Rc::new(SourceCompletions {
            catalog,
            kind: open.entry.kind,
            format: open.document.format,
        });
        self.source.update(cx, |state, cx| {
            if state.language_name().as_ref() != language {
                state.set_highlighter(language, cx);
            }
            state.lsp_mut().completion_provider = Some(provider);
            state.lsp_mut().completion_menu.max_width = px(440.);
        });
    }

    pub(super) fn update_source_diagnostics(&mut self, cx: &mut Context<Self>) {
        let message = self.diagnostic.clone();
        let line = self.source_issue_line;
        self.source.update(cx, |state, cx| {
            let text = state.text().clone();
            if let Some(diagnostics) = state.diagnostics_mut() {
                diagnostics.reset(&text);
                if let (Some(line), Some(message)) = (line, message) {
                    let length = text
                        .to_string()
                        .lines()
                        .nth(line as usize)
                        .unwrap_or_default()
                        .encode_utf16()
                        .count()
                        .max(1) as u32;
                    diagnostics.push(lsp_types::Diagnostic {
                        range: lsp_types::Range::new(
                            Position::new(line, 0),
                            Position::new(line, length),
                        ),
                        severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                        message,
                        source: Some("Regressify".into()),
                        ..Default::default()
                    });
                }
            }
            cx.notify();
        });
    }

    fn jump_to_source_issue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(line) = self.source_issue_line {
            self.source.update(cx, |state, cx| {
                let position = Position::new(line, 0);
                state.unfold_at(position, cx);
                state.set_cursor_position(position, window, cx);
                window.focus(&state.focus_handle(cx), cx);
            });
        }
    }

    pub(super) fn render_source(&self, cx: &mut Context<Self>) -> AnyElement {
        use gpui_kit::component::input::{Enter, Escape, MoveDown, MoveUp};
        let cursor = self.source.read(cx).cursor_position();
        let location = format!("Ln {}, Col {}", cursor.line + 1, cursor.character + 1);
        let language = self.source.read(cx).language_name().to_uppercase();
        div()
            .id("source-editor")
            .key_context("RegressifySource")
            .capture_action(cx.listener(|this, action: &Enter, window, cx| {
                this.route_completion(action, window, cx)
            }))
            .capture_action(cx.listener(|this, action: &MoveDown, window, cx| {
                this.route_completion(action, window, cx)
            }))
            .capture_action(cx.listener(|this, action: &MoveUp, window, cx| {
                this.route_completion(action, window, cx)
            }))
            .capture_action(cx.listener(|this, action: &Escape, window, cx| {
                this.route_completion(action, window, cx)
            }))
            .on_action(cx.listener(Self::request_completion))
            .flex()
            .flex_col()
            .size_full()
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("source-find")
                            .ghost()
                            .small()
                            .icon(IconName::Search)
                            .accessibility_label("Find in source")
                            .tooltip("Find in source")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.source
                                    .update(cx, |state, cx| state.open_search(false, cx))
                            })),
                    )
                    .child(
                        Button::new("source-replace")
                            .ghost()
                            .small()
                            .icon(gpui_kit::assets::IconName::Replace)
                            .accessibility_label("Replace in source")
                            .tooltip("Find and replace")
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.source
                                    .update(cx, |state, cx| state.open_search(true, cx))
                            })),
                    )
                    .child(
                        Button::new("source-suggest")
                            .ghost()
                            .small()
                            .icon(gpui_kit::assets::IconName::ListFilter)
                            .accessibility_label("Suggest")
                            .tooltip("Suggest (Ctrl+Space)")
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.request_completion(&CompleteSource, window, cx)
                            })),
                    )
                    .child(div().w(px(1.)).h_4().mx_2().bg(cx.theme().border))
                    .child(
                        Checkbox::new("source-wrap")
                            .label("Wrap")
                            .checked(self.source_wrap)
                            .on_change(cx.listener(|this, checked, window, cx| {
                                this.source_wrap = *checked;
                                this.source.update(cx, |state, cx| {
                                    state.set_soft_wrap(*checked, window, cx)
                                });
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("source-smaller")
                            .ghost()
                            .small()
                            .icon(gpui_kit::assets::IconName::ZoomOut)
                            .accessibility_label("Smaller source text")
                            .tooltip("Smaller source text")
                            .disabled(self.source_font_size <= 10.)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.source_font_size = (this.source_font_size - 1.).max(10.);
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("source-text-size")
                            .role(Role::Status)
                            .aria_label(format!("{} px", self.source_font_size))
                            .test_support()
                            .w(px(40.))
                            .text_center()
                            .text_xs()
                            .child(format!("{} px", self.source_font_size)),
                    )
                    .child(
                        Button::new("source-larger")
                            .ghost()
                            .small()
                            .icon(gpui_kit::assets::IconName::ZoomIn)
                            .accessibility_label("Larger source text")
                            .tooltip("Larger source text")
                            .disabled(self.source_font_size >= 24.)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.source_font_size = (this.source_font_size + 1.).min(24.);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div().flex_1().min_h_0().child(
                    CodeEditor::new(&self.source)
                        .aria_label("Configuration source")
                        .bordered(false)
                        .text_size(px(self.source_font_size))
                        .size_full()
                        .disabled(self.busy || self.session.open.is_none()),
                ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .h(px(30.))
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        div()
                            .id("source-language")
                            .role(Role::Status)
                            .aria_label(language.clone())
                            .test_support()
                            .child(language),
                    )
                    .child(
                        div()
                            .id("source-position")
                            .role(Role::Status)
                            .aria_label(location.clone())
                            .test_support()
                            .child(location),
                    )
                    .child(div().flex_1())
                    .when(self.diagnostic.is_some(), |bar| {
                        bar.child(
                            Button::new("source-issue")
                                .ghost()
                                .xsmall()
                                .icon(gpui_kit::assets::IconName::CircleAlert)
                                .label("1 issue")
                                .text_color(cx.theme().danger)
                                .tooltip("Go to syntax error")
                                .disabled(self.source_issue_line.is_none())
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.jump_to_source_issue(window, cx)
                                })),
                        )
                    })
                    .when(self.diagnostic.is_none(), |bar| {
                        bar.child(
                            Icon::new(gpui_kit::assets::IconName::CircleCheck)
                                .xsmall()
                                .text_color(cx.theme().success),
                        )
                    }),
            )
            .into_any_element()
    }
}

pub(super) fn error_line(error: &anyhow::Error) -> Option<u32> {
    error
        .downcast_ref::<serde_yaml::Error>()
        .and_then(|error| error.location())
        .map(|location| location.line())
        .or_else(|| {
            error
                .downcast_ref::<serde_json::Error>()
                .map(|error| error.line())
                .filter(|line| *line > 0)
        })
        .map(|line| line.saturating_sub(1) as u32)
}
