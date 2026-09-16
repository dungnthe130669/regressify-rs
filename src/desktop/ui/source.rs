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
    pub(super) fn accept_completion(
        &mut self,
        action: &gpui_kit::component::input::Enter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.source.read(cx).completion_menu_state().open {
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
}
