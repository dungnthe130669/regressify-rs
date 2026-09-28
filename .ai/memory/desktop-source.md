# Desktop Source Editing

Verified: 2026-09-28

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
