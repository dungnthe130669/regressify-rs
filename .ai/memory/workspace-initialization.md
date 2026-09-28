# Workspace Initialization

Verified: 2026-09-28

- [Initialization](../../src/init.rs) shares runtime defaults between CLI `init`
  and desktop setup: `regressify.yaml`, five `common/` files, and `visual_tests/`.
  The workspace config starts as `{}` so it inherits runtime defaults. Existing
  workspace and replacement-profile `.yml` files suppress generated `.yaml`
  defaults. Existing file contents, including invalid drafts, are preserved.
- [Desktop documents](../../src/desktop/document.rs) also accept blank/comment-only
  workspace YAML as defaults while preserving source for save and undo. They use
  the CLI parser to distinguish omitted content from explicit nulls; do not use
  global null normalization or assume tab indentation is valid YAML whitespace.
  `desktop::document::tests::blank_workspace_yaml` covers normalization, first-field
  edits, resetting the last field, and rejection boundaries.
- [Session connection](../../src/desktop/session.rs) initializes before listing
  entries and only replaces the current session after setup succeeds. The
  [UI loader](../../src/desktop/ui/mod.rs) invokes it in a background task after
  acquiring workspace ownership and checking unsaved work. `Workspace::open`
  remains read-only because activation/ownership probes also call it.
- Automatic desktop setup creates no example suites, exported schemas, VS Code
  files, Node helpers, or run/state artifacts. Explicit CLI `init` retains its
  sample, editor integration, and legacy migration. See the
  [desktop workflow](../../docs/desktop.md#workspace-and-files).
- [Workspace discovery](../../src/desktop/workspace.rs) lists all regular files
  under `common`, including `_on-ready.js`, `_override.css`, and nested/unknown
  formats. Only `visual_tests` entries become suites. `Entry::supports_editing`
  reflects bounded UTF-8 text detection, so JavaScript/CSS helpers are editable
  in Source while binary/unreadable/oversized files remain visible but disabled.
  Session tests validate only editable entries; `workspace_files_` GUI tests
  verify raw editing, mouse/keyboard guards, and both sizes and themes.
- Starter writes use same-directory temporary files with `persist_noclobber`;
  existing regular files are skipped and symlink/non-file destinations rejected.
  This prevents partial starter publication and clobbering competing files, not
  a transaction across the whole workspace or a guarantee against ancestor races.
- Evidence: `cargo test --locked --features desktop --lib init::tests` and
  `cargo test --locked --features desktop --lib desktop::session::tests` cover
  the inventory, repeated/concurrent setup, missing defaults, `.yml` preservation,
  read-only files, failed opens, and draft guards. Symlink regressions are Unix-only.
- Windows path assertions must compare canonical paths consistently: joining a
  slash-containing relative string can retain mixed separators, while diagnostics
  use the canonical path. Avoid raw display-string comparisons between the two.
- [GUI fixtures](../../src/desktop/ui/tests.rs) connect through the same initializer,
  so their entry lists already contain default non-suite files. Suite creation
  checks the previous inventory plus the new file; sidebar-scroll tests count all
  non-suite entries and cover seven/eight/nine/twelve total rows. These focused
  `gui-tests` regressions pass without changing runtime discovery or layout.
