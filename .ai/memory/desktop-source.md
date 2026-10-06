# Desktop Source Editing

Verified: 2026-10-06

- [Action viewport choices](../../src/desktop/ui/choices.rs) reuse the native
  list picker and effective consumer labels. Workspace diagnostics call the
  shared CLI validator, point to own/inherited action fields, and retain invalid
  drafts. Background results are keyed to workspace, file, and exact source.
- Source typing must schedule contextual validation as well as schema inspection;
  the native `action_viewport_picker_filters_choices_and_preserves_invalid_source`
  regression caught the missing callback. Syntax errors remain owned by Source:
  duplicating them in contextual validation broke the Windows smoke's `1 issue`
  assertion. The repaired `-SchemaFlows` smoke passed with preferences restored.
- [Viewport completion](../../src/desktop/completion.rs) uses parsed full value
  spans for spaces/hyphens and forces YAML block-list probes into value mode.
  The `action_viewport` desktop tests cover source preservation and these cases.
- [Source commands](../../src/desktop/ui/source.rs) bind inside
  `RegressifySource > Input` and additionally check the source focus handle and
  busy/editable state. The Edit menu uses the same actions with `action_context`.
- Line edits are a single `apply_lsp_edits` replacement, followed by selection
  restoration. Do not use `set_value` for interactive commands: it resets native
  history. JSON deliberately disables comment toggling.
- gpui-kit 0.6.1 block indentation includes the line at an exclusive column-zero
  selection end. Source uses shared line bounds for indentation, comments, moves,
  duplication, and deletion; plain Tab and completion acceptance remain native.
- On Windows/Linux, the toolkit's redo binding is Ctrl+Y. Source also binds
  Ctrl+Shift+Z to native redo so it cannot fall through to document history.
- Native input-change notifications are deferred. [GUI tests](../../src/desktop/ui/tests.rs)
  settle the executor before asserting that Source edits reached the session.
- [YAML field removal](../../src/desktop/document.rs) removes an orphaned preceding
  `INDENT` token when deleting the last mapping entry. yaml-edit 0.3.1 otherwise
  indents the following parent key into the scenario in the starter suite.
  `pixel_match_threshold_reset_preserves_suite_override` reproduced the semantic
  guard failure; document tests and native threshold-reset interactions now pass.
- `cargo test --locked --features gui-tests --lib source_line_commands -- --test-threads=1`
  covers text boundaries, CRLF/Unicode, reverse selections, keyboard undo/redo,
  disabled/read-only/focus guards, strict JSON, and actual menu clicks.
- [Schema smoke](../../ci/desktop-smoke/flows/schema.ps1) includes line shortcuts
  and menu screenshots. Interactive execution requires the isolation guards in
  the [native UI skill](../../.github/skills/desktop-native-ui/SKILL.md).
- [Workspace saves](../../src/desktop/workspace.rs) validate the corrected draft,
  not the previous saved source. Unparseable old source skips optional report-label
  migration without weakening exact-byte external-conflict checks.
- Editor diagnostics describe the draft; suite catalog errors describe the saved
  file and refresh after Save succeeds. The `corrected_invalid_source` filter in
  `cargo test --locked --features gui-tests --lib corrected_invalid_source -- --test-threads=1`
  covers YAML/JSON repairs, conflicts, and the native Save interaction.
- Non-YAML/JSON UTF-8 files use `Format::Text`, not YAML parsing. Workspace
  discovery checks content within the 4 MiB limit, and open rechecks the bytes;
  binary controls, invalid UTF-8, and oversized files remain non-editable.
- Plain text uses Source only, including breadcrumb and Save As transitions.
  The toolkit's `text` highlighter disables configuration highlighting; schema
  completion and hover providers are cleared. The `common_text_files_` model and
  `workspace_files_text_documents_support_source_editing_and_save` native tests
  cover saves, conflicts, binary guards, and native undo/redo.
- [External reload](../../src/desktop/ui/files.rs) reads the open file off-thread
  once per second and validates workspace/path/saved-byte identity on completion.
  Clean content reloads; pending/invalid drafts and read failures retain the editor.
  `external_reload` model/native tests cover metadata-preserving edits, atomic
  replacement, raw text, recovery, stale results, and real conflict button clicks.
- Hidden Fields controls retain old values after Source edits/saves. Only inspect
  their raw text when Fields is active; `external_reload_after_source_save` failed
  without this guard. Native `set_value` is not typing: use keyboard input to
  exercise change notifications and session dirty/error state.
