//! Source editing commands, schema assistance, and diagnostics.

use std::{ops::Range, rc::Rc, sync::Arc};

use gpui_kit::component::{
    input::{CompletionProvider, HoverProvider, Rope},
    status_bar::StatusBar,
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

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, schemars::JsonSchema)]
pub(super) enum LineOperation {
    ToggleComment,
    Indent,
    Outdent,
    DuplicateUp,
    DuplicateDown,
    MoveUp,
    MoveDown,
    Delete,
}

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
pub(super) struct SourceLineCommand {
    operation: LineOperation,
}

pub(super) fn bind_keys(modifier: &str, cx: &mut App) {
    cx.bind_keys([KeyBinding::new(
        &format!("{modifier}-shift-z"),
        gpui_kit::component::input::Redo,
        Some("RegressifySource > Input"),
    )]);
    cx.bind_keys(
        [
            (format!("{modifier}-/"), LineOperation::ToggleComment),
            (format!("{modifier}-]"), LineOperation::Indent),
            (format!("{modifier}-["), LineOperation::Outdent),
            ("alt-shift-up".into(), LineOperation::DuplicateUp),
            ("alt-shift-down".into(), LineOperation::DuplicateDown),
            ("alt-up".into(), LineOperation::MoveUp),
            ("alt-down".into(), LineOperation::MoveDown),
            (format!("{modifier}-shift-k"), LineOperation::Delete),
        ]
        .into_iter()
        .map(|(key, operation)| {
            KeyBinding::new(
                &key,
                SourceLineCommand { operation },
                Some("RegressifySource > Input"),
            )
        }),
    );
}

struct LineEdit {
    range: Range<usize>,
    text: String,
    selection: Range<usize>,
}

fn selected_lines(text: &str, selection: &Range<usize>) -> Range<usize> {
    let start = text[..selection.start]
        .rfind('\n')
        .map_or(0, |offset| offset + 1);
    // The line at a selection's exclusive column-zero end is not selected.
    let last = if !selection.is_empty() && text.as_bytes().get(selection.end - 1) == Some(&b'\n') {
        selection.end - 1
    } else {
        selection.end
    };
    let end = text[last..]
        .find('\n')
        .map_or(text.len(), |offset| last + offset + 1);
    start..end
}

fn toggle_line_comments(text: &str, selection: Range<usize>) -> Option<LineEdit> {
    let range = selected_lines(text, &selection);
    let block = &text[range.clone()];
    let lines = if block.is_empty() {
        vec![""]
    } else {
        block.split_inclusive('\n').collect::<Vec<_>>()
    };
    let uncomment = lines.iter().any(|line| !line.trim().is_empty())
        && lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .all(|line| line.trim_start_matches([' ', '\t']).starts_with('#'));
    let mut edits = Vec::new();
    let mut offset = 0;
    for line in lines {
        let content = line.trim_end_matches(['\r', '\n']);
        let trimmed = content.trim_start_matches([' ', '\t']);
        if !trimmed.is_empty() || selection.is_empty() {
            let prefix = offset + content.len() - trimmed.len();
            let removed = if uncomment {
                if trimmed.starts_with("# ") { 2 } else { 1 }
            } else {
                0
            };
            edits.push((prefix..prefix + removed, if uncomment { "" } else { "# " }));
        }
        offset += line.len();
    }
    replace_line_prefixes(text, range, selection, edits)
}

fn replace_line_prefixes(
    text: &str,
    range: Range<usize>,
    selection: Range<usize>,
    edits: Vec<(Range<usize>, &str)>,
) -> Option<LineEdit> {
    let mut replacement = text[range.clone()].to_string();
    let mut selection = selection;
    for (removed, inserted) in edits.into_iter().rev() {
        replacement.replace_range(removed.clone(), inserted);
        for endpoint in [&mut selection.start, &mut selection.end] {
            let start = range.start + removed.start;
            let end = range.start + removed.end;
            if *endpoint >= start {
                *endpoint = start + inserted.len() + endpoint.saturating_sub(end);
            }
        }
    }
    (replacement != text[range.clone()]).then_some(LineEdit {
        range,
        text: replacement,
        selection,
    })
}

// gpui-kit's block indent includes the exclusive end line; use our shared bounds.
fn indent_lines(text: &str, selection: Range<usize>, outdent: bool) -> Option<LineEdit> {
    let range = selected_lines(text, &selection);
    let block = &text[range.clone()];
    let lines = if block.is_empty() {
        vec![""]
    } else {
        block.split_inclusive('\n').collect::<Vec<_>>()
    };
    let width = gpui_kit::component::input::TabSize::default().tab_size;
    let indent = " ".repeat(width);
    let mut edits = Vec::new();
    let mut offset = 0;
    for line in lines {
        if outdent {
            let removed = if line.starts_with('\t') {
                1
            } else {
                line.bytes()
                    .take(width)
                    .take_while(|byte| *byte == b' ')
                    .count()
            };
            if removed > 0 {
                edits.push((offset..offset + removed, ""));
            }
        } else if !line.trim().is_empty() || selection.is_empty() {
            edits.push((offset..offset, indent.as_str()));
        }
        offset += line.len();
    }
    replace_line_prefixes(text, range, selection, edits)
}

fn split_line_ending(text: &str) -> (&str, &str) {
    if let Some(content) = text.strip_suffix("\r\n") {
        (content, "\r\n")
    } else if let Some(content) = text.strip_suffix('\n') {
        (content, "\n")
    } else {
        (text, "")
    }
}

fn line_edit(text: &str, selection: Range<usize>, operation: LineOperation) -> Option<LineEdit> {
    let range = selected_lines(text, &selection);
    let block = &text[range.clone()];
    let (content, ending) = split_line_ending(block);
    match operation {
        LineOperation::DuplicateUp | LineOperation::DuplicateDown => {
            let separator = if block.contains("\r\n") || text[..range.start].ends_with("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            let down = operation == LineOperation::DuplicateDown;
            let replacement = if !ending.is_empty() {
                block.to_string()
            } else if down {
                format!("{separator}{block}")
            } else {
                format!("{block}{separator}")
            };
            let offset = if down { range.end } else { range.start };
            let shift = if down { replacement.len() } else { 0 };
            Some(LineEdit {
                range: offset..offset,
                text: replacement,
                selection: selection.start + shift..selection.end + shift,
            })
        }
        LineOperation::MoveUp => {
            if range.start == 0 {
                return None;
            }
            let previous_start = text[..range.start - 1]
                .rfind('\n')
                .map_or(0, |offset| offset + 1);
            let previous = &text[previous_start..range.start];
            let (previous_content, previous_ending) = split_line_ending(previous);
            let replacement = if ending.is_empty() {
                format!("{block}{previous_ending}{previous_content}")
            } else {
                format!("{block}{previous}")
            };
            Some(LineEdit {
                range: previous_start..range.end,
                text: replacement,
                selection: selection.start - previous.len()..selection.end - previous.len(),
            })
        }
        LineOperation::MoveDown => {
            if range.end == text.len() {
                return None;
            }
            let next_end = text[range.end..]
                .find('\n')
                .map_or(text.len(), |offset| range.end + offset + 1);
            let next = &text[range.end..next_end];
            let (_, next_ending) = split_line_ending(next);
            let (replacement, shift) = if next_ending.is_empty() {
                (
                    format!("{next}{ending}{content}"),
                    next.len() + ending.len(),
                )
            } else {
                (format!("{next}{block}"), next.len())
            };
            Some(LineEdit {
                range: range.start..next_end,
                text: replacement,
                selection: (selection.start + shift).min(next_end)
                    ..(selection.end + shift).min(next_end),
            })
        }
        LineOperation::Delete => {
            let start = if range.end == text.len() && ending.is_empty() && range.start > 0 {
                range.start - split_line_ending(&text[..range.start]).1.len()
            } else {
                range.start
            };
            (start != range.end).then_some(LineEdit {
                range: start..range.end,
                text: String::new(),
                selection: start..start,
            })
        }
        LineOperation::ToggleComment => toggle_line_comments(text, selection),
        LineOperation::Indent => indent_lines(text, selection, false),
        LineOperation::Outdent => indent_lines(text, selection, true),
    }
}

#[derive(Clone)]
struct SourceCompletions {
    catalog: Arc<CompletionCatalog>,
    kind: DocumentKind,
    format: Format,
    workspace: Option<crate::desktop::workspace::Workspace>,
}

impl SourceCompletions {
    fn items(&self, text: &str, cursor: usize) -> Vec<CompletionItem> {
        self.catalog
            .complete_with_workspace(
                self.kind,
                self.format,
                text,
                cursor,
                self.workspace.as_ref(),
            )
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
            if document.format == Format::Text {
                document.validate()?;
                return Ok(Vec::new());
            }
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
                    Format::Text => "",
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

pub(super) fn append_action_viewport_diagnostics(
    document: &Document,
    issues: &[crate::desktop::validation::Diagnostic],
    diagnostics: &mut Vec<lsp_types::Diagnostic>,
) {
    if issues.is_empty() {
        return;
    }
    let text = document.source();
    let index = SourceIndex::new(document.format, text);
    let remaining = validation::MAX_DIAGNOSTICS.saturating_sub(diagnostics.len());
    for issue in issues.iter().take(remaining) {
        let range = index.range(&issue.pointer);
        diagnostics.push(source_diagnostic(
            lsp_types::Range::new(position(text, range.start), position(text, range.end)),
            issue.message.clone(),
        ));
    }
    diagnostics.sort_by_key(|issue| (issue.range.start, issue.range.end));
}

// Kit 0.7.3 tracks hover help and diagnostic popovers separately on EditorState;
// clear both so resolved or reconfigured diagnostics cannot leave a stale tooltip.
pub(super) fn clear_source_popovers(
    state: &mut gpui_kit::component::input::EditorState,
    cx: &mut Context<gpui_kit::component::input::EditorState>,
) {
    state.clear_hover_state(cx);
    state.clear_diagnostic_popover(cx);
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
    fn source_edit_menu(&self, cx: &App) -> impl IntoElement {
        let source = self.source.clone();
        let focus = source.read(cx).focus_handle(cx);
        let yaml = self
            .session
            .open
            .as_ref()
            .is_some_and(|open| open.document.format == Format::Yaml);
        Button::new("source-edit-menu")
            .ghost()
            .icon(gpui_kit::assets::IconName::SquarePen)
            .accessibility_label("Edit source lines")
            .tooltip("Edit source lines")
            .disabled(self.busy || self.session.open.is_none())
            .prepare_for_toolbar()
            .with_size(gpui_kit::component::Size::Medium)
            .dropdown_menu(move |menu, window, cx| {
                design::restore_dropdown_menu_focus(
                    menu.focus_handle(cx),
                    focus.clone(),
                    window,
                    cx,
                );
                let state = source.read(cx);
                let text = state.value();
                let lines = selected_lines(&text, &state.selected_range());
                menu.action_context(focus.clone())
                    .menu_with_disabled(
                        "Toggle Line Comment",
                        Box::new(SourceLineCommand {
                            operation: LineOperation::ToggleComment,
                        }),
                        !yaml,
                    )
                    .separator()
                    .menu(
                        "Indent Lines",
                        Box::new(SourceLineCommand {
                            operation: LineOperation::Indent,
                        }),
                    )
                    .menu(
                        "Outdent Lines",
                        Box::new(SourceLineCommand {
                            operation: LineOperation::Outdent,
                        }),
                    )
                    .separator()
                    .menu(
                        "Duplicate Lines Up",
                        Box::new(SourceLineCommand {
                            operation: LineOperation::DuplicateUp,
                        }),
                    )
                    .menu(
                        "Duplicate Lines Down",
                        Box::new(SourceLineCommand {
                            operation: LineOperation::DuplicateDown,
                        }),
                    )
                    .menu_with_disabled(
                        "Move Lines Up",
                        Box::new(SourceLineCommand {
                            operation: LineOperation::MoveUp,
                        }),
                        lines.start == 0,
                    )
                    .menu_with_disabled(
                        "Move Lines Down",
                        Box::new(SourceLineCommand {
                            operation: LineOperation::MoveDown,
                        }),
                        lines.end == text.len(),
                    )
                    .menu_with_disabled(
                        "Delete Lines",
                        Box::new(SourceLineCommand {
                            operation: LineOperation::Delete,
                        }),
                        text.is_empty(),
                    )
            })
    }

    fn source_line_command(
        &mut self,
        action: &SourceLineCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = self.source.read(cx);
        if !state.focus_handle(cx).is_focused(window) {
            cx.propagate();
            return;
        }
        if self.busy || !state.is_editable() || self.session.open.is_none() {
            return;
        }
        let text = state.value().to_string();
        let selection = state.selected_range();
        if action.operation == LineOperation::ToggleComment
            && self.session.open.as_ref().unwrap().document.format != Format::Yaml
        {
            return;
        }
        let Some(edit) = line_edit(&text, selection, action.operation) else {
            return;
        };
        self.source.update(cx, |state, cx| {
            state.apply_lsp_edits(
                &vec![TextEdit {
                    range: lsp_types::Range::new(
                        position(&text, edit.range.start),
                        position(&text, edit.range.end),
                    ),
                    new_text: edit.text,
                }],
                window,
                cx,
            );
            // Kit 0.7.3 collapses reversed ranges; keep the edited selection forward.
            state.set_selected_range(edit.selection, cx);
        });
    }

    pub(super) fn source_window_activation_changed(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !window.is_window_active() {
            self.source_focus_restore = self
                .session
                .open
                .as_ref()
                .filter(|_| self.source.read(cx).focus_handle(cx).is_focused(window))
                .map(|open| open.entry.path.clone());
            return;
        }
        let Some(path) = self.source_focus_restore.take() else {
            return;
        };
        // Restore only the same visible editor, without taking focus from another control.
        if self.busy
            || self.main_tab != 0
            || self.overview
            || self.tab != 1
            || self
                .session
                .open
                .as_ref()
                .is_none_or(|open| open.entry.path != path)
            || window.focused(cx).is_some_and(|focus| focus != self.focus)
        {
            return;
        }
        window.focus(&self.source.read(cx).focus_handle(cx), cx);
    }

    pub(super) fn apply_editor_display(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.source_wrap = self.preferences.source_wrap;
        self.source_line_numbers = self.preferences.source_line_numbers;
        self.source_font_size = f32::from(self.preferences.source_font_size.clamp(10, 24));
        self.source.update(cx, |state, cx| {
            state.set_soft_wrap(self.source_wrap, window, cx);
            state.set_line_number(self.source_line_numbers, window, cx);
        });
        if !self
            .source_font_size_input
            .read(cx)
            .focus_handle(cx)
            .is_focused(window)
        {
            self.restore_source_size(window, cx);
        }
        cx.notify();
    }

    pub(super) fn change_source_size(
        &mut self,
        size: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !(10..=24).contains(&size) || size == self.preferences.source_font_size {
            return;
        }
        self.preferences.source_font_size = size;
        self.apply_editor_display(window, cx);
        self.save_preferences(window, cx);
    }

    pub(super) fn restore_source_size(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.preferences.source_font_size.to_string();
        self.source_font_size_input.update(cx, |state, cx| {
            if state.value().as_ref() != value {
                state.set_value(value, window, cx);
            }
        });
    }

    pub(super) fn step_source_size(
        &mut self,
        action: StepAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Ok(size) = self.source_font_size_input.read(cx).value().parse::<u8>() else {
            return;
        };
        if !(10..=24).contains(&size) {
            return;
        }
        let next = match action {
            StepAction::Increment => size.saturating_add(1),
            StepAction::Decrement => size.saturating_sub(1),
        };
        self.change_source_size(next, window, cx);
        self.restore_source_size(window, cx);
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
        if self.busy || self.tab != 1 || self.session.open.is_none() {
            return;
        }
        self.source
            .update(cx, |state, cx| state.show_completions(window, cx));
    }

    pub(super) fn configure_source(&mut self, cx: &mut Context<Self>) {
        let Some(open) = &self.session.open else {
            return;
        };
        if open.document.format == Format::Text {
            self.source.update(cx, |state, cx| {
                state.set_highlighter("text", cx);
                clear_source_popovers(state, cx);
                state.lsp_mut().completion_provider = None;
                state.lsp_mut().hover_provider = None;
            });
            return;
        }
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
            workspace: self.session.workspace.clone(),
        });
        self.source.update(cx, |state, cx| {
            if state.language_name().as_ref() != language {
                state.set_highlighter(language, cx);
            }
            clear_source_popovers(state, cx);
            state.lsp_mut().completion_provider = Some(provider.clone());
            state.lsp_mut().hover_provider = Some(provider);
            state.lsp_mut().completion_menu.max_width = px(440.);
        });
    }

    pub(super) fn update_source_diagnostics(&mut self, cx: &mut Context<Self>) {
        let issues = self.source_issues.clone();
        self.source.update(cx, |state, cx| {
            clear_source_popovers(state, cx);
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
                            .flex()
                            .justify_center()
                            .child(
                                Button::new(("source-diagnostic", index))
                                    .ghost()
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
                            .justify_center()
                            .child(
                                div()
                                    .id(("source-diagnostic-dot", index))
                                    .test_support()
                                    .size(px(8.))
                                    .child(
                                        gpui_kit::component::badge::Badge::new()
                                            .dot()
                                            .color(color)
                                            .child(div().size(px(8.))),
                                    ),
                            ),
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
                                        .text_center()
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
        use gpui_kit::component::input::{
            Enter, Escape, IndentInline, MoveDown, MoveUp, OutdentInline,
        };
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
            .capture_action(cx.listener(|this, _: &IndentInline, window, cx| {
                let state = this.source.read(cx);
                if !state.focus_handle(cx).is_focused(window) {
                    return;
                }
                if state.completion_menu_state().open {
                    this.route_completion(
                        &Enter {
                            secondary: false,
                            shift: false,
                        },
                        window,
                        cx,
                    );
                } else if !state.selected_range().is_empty() {
                    this.source_line_command(
                        &SourceLineCommand {
                            operation: LineOperation::Indent,
                        },
                        window,
                        cx,
                    );
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &OutdentInline, window, cx| {
                if this.source.read(cx).focus_handle(cx).is_focused(window) {
                    this.source_line_command(
                        &SourceLineCommand {
                            operation: LineOperation::Outdent,
                        },
                        window,
                        cx,
                    );
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, action: &MoveDown, window, cx| {
                this.route_completion(action, window, cx)
            }))
            .capture_action(cx.listener(|this, action: &MoveUp, window, cx| {
                this.route_completion(action, window, cx)
            }))
            .capture_action(cx.listener(|this, action: &Escape, window, cx| {
                if this.source.read(cx).completion_menu_state().open {
                    this.route_completion(action, window, cx);
                } else {
                    this.source.update(cx, clear_source_popovers);
                }
            }))
            .on_action(cx.listener(Self::request_completion))
            .on_action(cx.listener(Self::source_line_command))
            .flex()
            .flex_col()
            .size_full()
            .child(
                div()
                    .id("source-toolbar")
                    .role(Role::Group)
                    .aria_label("Source commands")
                    .test_support()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(design::ToolbarFocus::new(
                        "source-toolbar-focus",
                        gpui_kit::component::toolbar::Toolbar::new("source-command-toolbar")
                            .with_size(gpui_kit::component::Size::Medium)
                            .h_auto()
                            .flex_wrap()
                            .content(self.source_edit_menu(cx))
                            .child(
                                Button::new("source-find")
                                    .ghost()
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
                                    .icon(gpui_kit::assets::IconName::ListFilter)
                                    .accessibility_label("Suggest")
                                    .tooltip("Suggest (Ctrl+Space)")
                                    .disabled(self.busy)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.request_completion(&CompleteSource, window, cx)
                                    })),
                            )
                            .content(
                                gpui_kit::component::separator::Separator::vertical()
                                    .h_4()
                                    .mx_2(),
                            )
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
                            .content(div().flex_1())
                            .child(
                                Button::new("source-reset-size")
                                    .ghost()
                                    .icon(gpui_kit::assets::IconName::RotateCcw)
                                    .tooltip("Reset text size")
                                    .accessibility_label("Reset text size")
                                    .disabled(self.source_font_size == 14.)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.change_source_size(14, window, cx);
                                        this.restore_source_size(window, cx);
                                    })),
                            )
                            .content(
                                div()
                                    .id("source-text-size")
                                    .role(Role::Group)
                                    .aria_label("Source font size")
                                    .test_support()
                                    .w(px(132.))
                                    .flex_shrink_0()
                                    .child(
                                        NumberInput::new(&self.source_font_size_input)
                                            .with_size(gpui_kit::component::Size::Medium),
                                    ),
                            ),
                    )),
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
                StatusBar::new()
                    .flex_shrink_0()
                    .left(
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
                        bar.right(
                            div()
                                .id("source-completion-status")
                                .role(Role::Status)
                                .aria_label(suggestions.clone())
                                .test_support()
                                .child(suggestions),
                        )
                    })
                    .when(self.diagnostic.is_some(), |bar| {
                        bar.right(
                            Button::new("source-issue")
                                .ghost()
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
                        bar.right(
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

    use super::{LineOperation, SourceCompletions, line_edit};
    use crate::desktop::{
        completion::CompletionCatalog,
        document::{DocumentKind, Format},
    };

    #[test]
    fn action_viewport_diagnostics_preserve_the_combined_issue_bound() {
        let document = crate::desktop::document::Document::new(
            DocumentKind::Suite,
            Format::Json,
            serde_json::json!({"scenarios": [{"url": ""}]}).to_string(),
        )
        .unwrap();
        let (_, mut diagnostics) = super::inspect_document(&document);
        assert_eq!(diagnostics.len(), 1);
        let original = diagnostics[0].message.clone();
        let limit = crate::desktop::validation::MAX_DIAGNOSTICS;
        let contextual = (0..limit)
            .map(|_| crate::desktop::validation::Diagnostic {
                pointer: "/scenarios/0/url".into(),
                message: "Invalid viewportNames".into(),
            })
            .collect::<Vec<_>>();
        super::append_action_viewport_diagnostics(&document, &contextual, &mut diagnostics);
        assert_eq!(diagnostics.len(), limit);
        assert!(diagnostics.iter().any(|issue| issue.message == original));
        super::append_action_viewport_diagnostics(&document, &contextual, &mut diagnostics);
        assert_eq!(diagnostics.len(), limit);
    }

    fn edited_lines(
        text: &str,
        selection: std::ops::Range<usize>,
        operation: LineOperation,
    ) -> (String, std::ops::Range<usize>) {
        let edit = line_edit(text, selection, operation).expect("Expected a line edit");
        let mut result = text.to_string();
        result.replace_range(edit.range, &edit.text);
        assert!(result.is_char_boundary(edit.selection.start));
        assert!(result.is_char_boundary(edit.selection.end));
        (result, edit.selection)
    }

    #[test]
    fn source_line_commands_comments_preserve_indentation_unicode_and_crlf() {
        let original = "  label: caf\u{e9}\r\n\r\n  debug: true\r\nscenarios: []\r\n";
        let selection = 2..original.find("scenarios").unwrap();
        let (commented, selected) =
            edited_lines(original, selection.clone(), LineOperation::ToggleComment);
        assert_eq!(
            commented,
            "  # label: caf\u{e9}\r\n\r\n  # debug: true\r\nscenarios: []\r\n"
        );
        assert_eq!(
            edited_lines(&commented, selected, LineOperation::ToggleComment),
            (original.to_string(), selection)
        );
        assert_eq!(
            edited_lines(
                "#debug: true\n  # headless: true",
                0..30,
                LineOperation::ToggleComment
            )
            .0,
            "debug: true\n  headless: true"
        );
    }

    #[test]
    fn source_line_commands_comments_handle_mixed_and_empty_lines() {
        let original = "# existing\nnew\n";
        let (commented, selected) =
            edited_lines(original, 0..original.len(), LineOperation::ToggleComment);
        assert_eq!(commented, "# # existing\n# new\n");
        assert_eq!(
            edited_lines(&commented, selected, LineOperation::ToggleComment).0,
            original
        );
        assert_eq!(
            edited_lines("", 0..0, LineOperation::ToggleComment),
            ("# ".into(), 2..2)
        );
        assert_eq!(
            edited_lines("\n", 0..0, LineOperation::ToggleComment),
            ("# \n".into(), 2..2)
        );
        assert!(line_edit(" \n\n", 0..3, LineOperation::ToggleComment).is_none());
    }

    #[test]
    fn source_line_commands_indent_only_selected_lines() {
        let original = "  a\r\n\r\n b\r\nc\r\n";
        let selection = 0..original.find('c').unwrap();
        let (indented, selected) = edited_lines(original, selection, LineOperation::Indent);
        assert_eq!(indented, "    a\r\n\r\n   b\r\nc\r\n");
        assert_eq!(
            edited_lines(&indented, selected, LineOperation::Outdent).0,
            original
        );
        assert_eq!(
            edited_lines("\ta\n b", 0..5, LineOperation::Outdent),
            ("a\nb".into(), 0..3)
        );
        assert_eq!(
            edited_lines("", 0..0, LineOperation::Indent),
            ("  ".into(), 2..2)
        );
        assert!(line_edit("text", 0..4, LineOperation::Outdent).is_none());
    }

    #[test]
    fn source_line_commands_duplicate_selected_blocks_and_unterminated_lines() {
        for (original, selection, operation, expected, selected) in [
            (
                "a\nb\nc",
                0..4,
                LineOperation::DuplicateDown,
                "a\nb\na\nb\nc",
                4..8,
            ),
            (
                "a\nb\nc",
                2..4,
                LineOperation::DuplicateUp,
                "a\nb\nb\nc",
                2..4,
            ),
            ("a\nb", 2..3, LineOperation::DuplicateDown, "a\nb\nb", 4..5),
            ("a\nb", 2..3, LineOperation::DuplicateUp, "a\nb\nb", 2..3),
            (
                "a\r\nb",
                3..4,
                LineOperation::DuplicateDown,
                "a\r\nb\r\nb",
                6..7,
            ),
            ("", 0..0, LineOperation::DuplicateDown, "\n", 1..1),
            ("a\n", 2..2, LineOperation::DuplicateUp, "a\n\n", 2..2),
        ] {
            assert_eq!(
                edited_lines(original, selection, operation),
                (expected.into(), selected),
                "{operation:?}: {original:?}"
            );
        }
    }

    #[test]
    fn source_line_commands_move_blocks_without_changing_line_endings() {
        for (original, selection, operation, expected, selected) in [
            (
                "a\nb\nc\nd\n",
                2..6,
                LineOperation::MoveUp,
                "b\nc\na\nd\n",
                0..4,
            ),
            (
                "a\nb\nc\nd\n",
                2..6,
                LineOperation::MoveDown,
                "a\nd\nb\nc\n",
                4..8,
            ),
            ("a\nb", 2..3, LineOperation::MoveUp, "b\na", 0..1),
            ("a\nb", 0..2, LineOperation::MoveDown, "b\na", 2..3),
            ("a\r\nb", 3..4, LineOperation::MoveUp, "b\r\na", 0..1),
            ("a\r\nb", 0..3, LineOperation::MoveDown, "b\r\na", 3..4),
            ("a\n", 2..2, LineOperation::MoveUp, "\na", 0..0),
            (
                "\u{e9}\n\u{1f680}",
                3..7,
                LineOperation::MoveUp,
                "\u{1f680}\n\u{e9}",
                0..4,
            ),
        ] {
            assert_eq!(
                edited_lines(original, selection, operation),
                (expected.into(), selected),
                "{operation:?}: {original:?}"
            );
        }
        assert!(line_edit("a\nb", 0..0, LineOperation::MoveUp).is_none());
        assert!(line_edit("a\nb\n", 2..2, LineOperation::MoveDown).is_none());
        assert!(line_edit("", 0..0, LineOperation::MoveDown).is_none());
    }

    #[test]
    fn source_line_commands_delete_whole_lines_including_the_last_line() {
        for (original, selection, expected, selected) in [
            ("a\nb\nc\n", 2..4, "a\nc\n", 2..2),
            ("a\nb", 2..2, "a", 1..1),
            ("a\r\nb", 3..4, "a", 1..1),
            ("a\nb\n", 4..4, "a\nb", 3..3),
            ("a\nb\n", 0..4, "", 0..0),
        ] {
            assert_eq!(
                edited_lines(original, selection, LineOperation::Delete),
                (expected.into(), selected),
                "{original:?}"
            );
        }
        assert!(line_edit("", 0..0, LineOperation::Delete).is_none());
    }

    #[test]
    fn compact_completion_documentation_keeps_the_example_and_full_explanation() {
        let provider = SourceCompletions {
            workspace: None,
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
