//! Adapt local schema suggestions to the editor's native completion menu.

use std::{rc::Rc, sync::Arc};

use gpui_kit::component::{
    input::{CompletionProvider, HoverProvider, Rope},
    text::{SelectionFormat, TextView},
};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Documentation, Position, TextEdit,
};

use super::*;
use crate::desktop::{
    completion::{CompletionCatalog, SourceIndex},
    document::{Document, Format, validate_semantics},
    validation,
};

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
                documentation: (!item.description.is_empty()).then(|| {
                    Documentation::String(
                        item.description
                            .lines()
                            .map(str::trim)
                            .filter(|line| !line.is_empty())
                            .collect::<Vec<_>>()
                            .join(" "),
                    )
                }),
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

impl HoverProvider for SourceCompletions {
    fn hover(
        &self,
        text: &Rope,
        offset: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Option<lsp_types::Hover>>> {
        let provider = self.clone();
        let text = text.clone();
        cx.background_executor().spawn(async move {
            let text = text.to_string();
            Ok(provider
                .catalog
                .hint(provider.kind, provider.format, &text, offset)
                .map(|hint| lsp_types::Hover {
                    range: Some(lsp_types::Range::new(
                        position(&text, hint.range.start),
                        position(&text, hint.range.end),
                    )),
                    contents: lsp_types::HoverContents::Markup(lsp_types::MarkupContent {
                        kind: lsp_types::MarkupKind::PlainText,
                        value: hint.text,
                    }),
                }))
        })
    }
}

pub(super) fn inspect_document(document: &Document) -> (Value, Vec<lsp_types::Diagnostic>) {
    let text = document.source();
    let value = document.value();
    let result = value
        .as_ref()
        .map_err(|error| anyhow!("{error:#}"))
        .and_then(|value| {
            let issues = validation::diagnostics(document.kind, value)?;
            if issues.is_empty() {
                validate_semantics(document.kind, value)?;
                return Ok(Vec::new());
            }
            let index = SourceIndex::new(document.format, text);
            let mut diagnostics = issues
                .into_iter()
                .map(|issue| {
                    let range = index.range(&issue.pointer);
                    source_diagnostic(
                        lsp_types::Range::new(
                            position(text, range.start),
                            position(text, range.end),
                        ),
                        issue.message,
                    )
                })
                .collect::<Vec<_>>();
            diagnostics.sort_by_key(|issue| (issue.range.start, issue.range.end));
            Ok(diagnostics)
        });
    let issues = match result {
        Ok(issues) => issues,
        Err(error) => {
            let syntax_error = value.as_ref().err();
            let line = syntax_error.and_then(error_line).unwrap_or(0);
            let length = text
                .lines()
                .nth(line as usize)
                .unwrap_or_default()
                .encode_utf16()
                .count()
                .max(1) as u32;
            let hint = if syntax_error.is_some() {
                match document.format {
                    Format::Yaml => {
                        "\nUse spaces for YAML indentation, ': ' after field names, and quotes around selectors beginning with #."
                    }
                    Format::Json => {
                        "\nUse double quotes for JSON field names and text, commas between fields, and no trailing commas."
                    }
                }
            } else {
                ""
            };
            vec![source_diagnostic(
                lsp_types::Range::new(Position::new(line, 0), Position::new(line, length)),
                format!("{error:#}{hint}"),
            )]
        }
    };
    (value.unwrap_or(Value::Null), issues)
}

fn source_diagnostic(range: lsp_types::Range, message: String) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic {
        range,
        severity: Some(lsp_types::DiagnosticSeverity::ERROR),
        message,
        source: Some("Regressify".into()),
        ..Default::default()
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
    pub(super) fn apply_editor_display(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.source_wrap = self.preferences.source_wrap;
        self.source_line_numbers = self.preferences.source_line_numbers;
        self.source_font_size = f32::from(self.preferences.source_font_size.clamp(10, 24));
        self.source.update(cx, |state, cx| {
            state.set_soft_wrap(self.source_wrap, window, cx);
            state.set_line_number(self.source_line_numbers, window, cx);
        });
        cx.notify();
    }

    fn change_source_size(&mut self, size: u8, window: &mut Window, cx: &mut Context<Self>) {
        self.preferences.source_font_size = size.clamp(10, 24);
        self.apply_editor_display(window, cx);
        self.save_preferences(window, cx);
    }

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
            state.clear_hover_state(cx);
            state.lsp_mut().completion_provider = Some(provider.clone());
            state.lsp_mut().hover_provider = Some(provider);
            state.lsp_mut().completion_menu.max_width = px(440.);
        });
    }

    pub(super) fn update_source_diagnostics(&mut self, cx: &mut Context<Self>) {
        let issues = self.source_issues.clone();
        self.source.update(cx, |state, cx| {
            state.clear_hover_state(cx);
            let text = state.text().clone();
            if let Some(diagnostics) = state.diagnostics_mut() {
                diagnostics.reset(&text);
                for issue in issues {
                    diagnostics.push(issue);
                }
            }
            cx.notify();
        });
    }

    fn jump_to_source_issue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cursor = self.source.read(cx).cursor_position();
        if let Some(issue) = self
            .source_issues
            .iter()
            .find(|issue| issue.range.start > cursor)
            .or_else(|| self.source_issues.first())
        {
            self.go_to_source_position(issue.range.start, window, cx);
        }
    }

    fn go_to_source_position(
        &mut self,
        position: Position,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || (self.tab != 1 && !self.select_editor_tab(1, window, cx)) {
            return;
        }
        self.source.update(cx, |state, cx| {
            state.unfold_at(position, cx);
            state.set_cursor_position(position, window, cx);
            window.focus(&state.focus_handle(cx), cx);
        });
    }

    pub(super) fn render_source_diagnostics(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let summary = self.diagnostic.as_ref()?;
        let rows = self
            .source_issues
            .iter()
            .enumerate()
            .map(|(index, issue)| {
                let position = issue.range.start;
                let mut description = String::with_capacity(issue.message.len());
                for character in issue.message.chars() {
                    if character == '\n' {
                        description.push_str("\\\n");
                    } else {
                        if character.is_ascii_punctuation() {
                            description.push('\\');
                        }
                        description.push(character);
                    }
                }
                let (severity, color) =
                    if issue.severity == Some(lsp_types::DiagnosticSeverity::WARNING) {
                        ("Warning", cx.theme().warning)
                    } else {
                        ("Error", cx.theme().danger)
                    };
                div()
                    .id(("source-diagnostic-row", index))
                    .role(Role::Row)
                    .aria_label(format!("Issue on line {}", position.line + 1))
                    .flex()
                    .items_start()
                    .gap_3()
                    .px_4()
                    .py_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .w_full()
                    .min_w_0()
                    .text_sm()
                    .line_height(px(20.))
                    .child(
                        div()
                            .id(("source-diagnostic-line", index))
                            .role(Role::Cell)
                            .w(px(64.))
                            .flex_shrink_0()
                            .child(
                                Button::new(("source-diagnostic", index))
                                    .ghost()
                                    .xsmall()
                                    .h(px(20.))
                                    .py_0()
                                    .text_sm()
                                    .line_height(px(20.))
                                    .label((position.line + 1).to_string())
                                    .accessibility_label(format!(
                                        "Go to issue on line {}",
                                        position.line + 1
                                    ))
                                    .tooltip("Go to this issue")
                                    .disabled(self.busy)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.go_to_source_position(position, window, cx)
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id(("source-diagnostic-type", index))
                            .role(Role::Cell)
                            .aria_label(severity)
                            .test_support()
                            .w(px(48.))
                            .h(px(20.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .child(div().size(px(8.)).rounded_full().bg(color)),
                    )
                    .child(
                        div()
                            .id(("source-diagnostic-message", index))
                            .role(Role::Cell)
                            .aria_label(issue.message.clone())
                            .test_support()
                            .flex_1()
                            .min_w_0()
                            .text_color(cx.theme().foreground)
                            .child(
                                TextView::markdown(("source-diagnostic-text", index), description)
                                    .selectable(true)
                                    .selection_format(SelectionFormat::Plain)
                                    .scrollable(false)
                                    .text_sm()
                                    .line_height(px(20.)),
                            ),
                    )
            })
            .collect::<Vec<_>>();
        Some(
            div()
                .id("diagnostic")
                .role(Role::Alert)
                .aria_label(summary.clone())
                .test_support()
                .flex()
                .flex_col()
                .h(px(if self.source_issues.len() == 1 {
                    112.
                } else {
                    160.
                }))
                .flex_shrink_0()
                .border_t_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .child(
                    div()
                        .id("source-issues-table")
                        .role(Role::Table)
                        .aria_label("Source issues")
                        .test_support()
                        .flex()
                        .flex_col()
                        .size_full()
                        .child(
                            div()
                                .id("source-issues-header")
                                .role(Role::Row)
                                .flex()
                                .flex_shrink_0()
                                .gap_3()
                                .px_4()
                                .py_1()
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .children([("Line", 64.), ("Type", 48.)].map(|(label, width)| {
                                    div()
                                        .id(label)
                                        .role(Role::ColumnHeader)
                                        .aria_label(label)
                                        .test_support()
                                        .w(px(width))
                                        .flex_shrink_0()
                                        .child(label)
                                }))
                                .child(
                                    div()
                                        .id("Description")
                                        .role(Role::ColumnHeader)
                                        .aria_label("Description")
                                        .test_support()
                                        .flex_1()
                                        .child("Description"),
                                ),
                        )
                        .child(
                            div()
                                .id("source-issues-scroll")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scrollbar()
                                .child(div().flex().flex_col().children(rows)),
                        ),
                )
                .into_any_element(),
        )
    }

    pub(super) fn render_source(&self, cx: &mut Context<Self>) -> AnyElement {
        use gpui_kit::component::input::{Enter, Escape, MoveDown, MoveUp};
        let cursor = self.source.read(cx).cursor_position();
        let location = format!("Ln {}, Col {}", cursor.line + 1, cursor.character + 1);
        let language = self.source.read(cx).language_name().to_uppercase();
        let menu = self.source.read(cx).completion_menu_state();
        let suggestions = menu.open.then(|| {
            format!(
                "{} {}",
                menu.items.len(),
                if menu.items.len() == 1 {
                    "suggestion"
                } else {
                    "suggestions"
                }
            )
        });
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
                                this.preferences.source_wrap = *checked;
                                this.apply_editor_display(window, cx);
                                this.save_preferences(window, cx);
                            })),
                    )
                    .child(
                        Checkbox::new("source-line-numbers")
                            .label("Line numbers")
                            .checked(self.source_line_numbers)
                            .on_change(cx.listener(|this, checked, window, cx| {
                                this.preferences.source_line_numbers = *checked;
                                this.apply_editor_display(window, cx);
                                this.save_preferences(window, cx);
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
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.change_source_size(
                                    (this.source_font_size as u8).saturating_sub(1),
                                    window,
                                    cx,
                                );
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
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.change_source_size(
                                    this.source_font_size as u8 + 1,
                                    window,
                                    cx,
                                );
                            })),
                    )
                    .child(
                        Button::new("source-reset-size")
                            .ghost()
                            .small()
                            .icon(gpui_kit::assets::IconName::RotateCcw)
                            .tooltip("Reset text size")
                            .accessibility_label("Reset text size")
                            .disabled(self.source_font_size == 14.)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.change_source_size(14, window, cx)
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
                    .when_some(suggestions, |bar, suggestions| {
                        bar.child(
                            div()
                                .id("source-completion-status")
                                .role(Role::Status)
                                .aria_label(suggestions.clone())
                                .test_support()
                                .child(suggestions),
                        )
                    })
                    .when(self.diagnostic.is_some(), |bar| {
                        bar.child(
                            Button::new("source-issue")
                                .ghost()
                                .xsmall()
                                .icon(gpui_kit::assets::IconName::CircleAlert)
                                .label(format!(
                                    "{} {}",
                                    self.source_issues.len(),
                                    if self.source_issues.len() == 1 {
                                        "issue"
                                    } else {
                                        "issues"
                                    }
                                ))
                                .text_color(cx.theme().danger)
                                .tooltip("Go to next issue")
                                .disabled(self.source_issues.is_empty() || self.busy)
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::SourceCompletions;
    use crate::desktop::{
        completion::CompletionCatalog,
        document::{DocumentKind, Format},
    };

    #[test]
    fn compact_completion_documentation_keeps_the_example_and_full_explanation() {
        let provider = SourceCompletions {
            catalog: Arc::new(CompletionCatalog::new().unwrap()),
            kind: DocumentKind::Suite,
            format: Format::Yaml,
        };
        let source = "scenarios: []\nbasicAuth:\n  ori";
        let items = provider.items(source, source.len());
        let origin = items.iter().find(|item| item.label == "origin").unwrap();
        let Some(lsp_types::Documentation::String(documentation)) = &origin.documentation else {
            panic!("Expected completion documentation");
        };
        assert_eq!(documentation.lines().count(), 1);
        assert!(documentation.contains("https://example.com"));
        assert!(documentation.contains("Required for every entry"));
        assert!(documentation.contains("Example:"));
    }
}
