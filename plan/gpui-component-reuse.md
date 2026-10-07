# GPUI Kit Component Reuse Plan

Date: 2026-10-06

Status: Approved for implementation subject to the verification gates below.
This document records approved scope and behavior; no implementation is claimed.

## Recommendation

Upgrade GPUI Kit to the latest published stable version, verified as **0.7.1**
on 2026-10-06, then prefer its matching components throughout the desktop. The scope
includes progress, numeric and binary controls, forms, toolbars, managed menus,
value pickers, clipboard controls, status/footer elements, log navigation,
metadata, empty states, and controlled collapsible sections. Lists, trees,
application menus, and tables should be assessed per surface, not rejected
because Regressify still owns application state.

Do not rewrite the whole workbench. Regressify already uses toolkit buttons,
checkboxes, inputs, textareas, code editors, menus, dialogs, tabs, title bars,
hover cards, resizable panels, and scrollbars. Much of the remaining UI code
connects those components to application behavior, not a custom widget library.

Prefer toolkit components wherever they meet the project's use case, including
through a small application adapter. The primary objective is maintainable code
reduction, measured as deleted rendering code against added adapters, state, and
subscriptions. A correct toolkit migration remains acceptable even if it
increases net code. Keeping domain state or adding a modest subscription is not,
by itself, a reason to retain a custom widget.

Preserve application policy: source fidelity, invalid drafts, operation guards,
selection scope, artifact identity, and asynchronous ownership. Distinguish a
real API/behavior blocker from an untested integration or a cosmetic difference.
The latter should become a targeted pilot or review question, not an automatic
rejection. Source-code reduction does not imply a smaller binary; binary size
and build-time effects would need separate measurement.

## Review Baseline

- Pre-implementation Regressify revision: `be843359a593b19d155c82bdcea0ee163e015423`.
- Original proposal revision: `5802db8e68231d1e0f10422b39c176ff197f70f3`
  (historical, not the current implementation baseline).
- Before implementation, [Cargo.toml](../Cargo.toml#L38) pinned `gpui-kit = "=0.6.1"`
  and the lockfile resolved Kit/Component/Base 0.6.1 with GPUI Pre 0.3.4.
  Current implementation and gate status live in the task bundle.
- The read-only GPUI Kit clone HEAD was
  `c0bebdc8a8cf9a74f9f52feaca278a52a9cfcca2`.
- Also compared the relevant source paths with release tag `v0.7.1`, commit
  `87d10ae5e1299d1be18670a37570c5402f952e34`. Recommendations below reference
  that release rather than assuming every feature at HEAD has been published.
- The [registry API][kit-registry], rechecked 2026-10-06, reported 0.7.1 as the
  latest stable, non-yanked release, published 2026-10-05. This migration is
  frozen at 0.7.1; do not chase newer releases during implementation.
- Reviewed native UI rendering, callbacks, selection helpers, source commands,
  representative native tests, and the toolkit implementations. This is a static
  source/API review, not a compiled or interactively tested migration.

Scope is the Rust desktop UI. The React report frontend is not a GPUI consumer.
Consume `gpui_kit::component` through Cargo; do not copy component source into
Regressify or add a machine-local path dependency on the clone.

### Review Findings And Decision Ledger

- The reported baseline was 61 broken relative links after the plan moved from
  `docs/plan/` to `plan/`. The first focused check after fixing the Cargo link
  found 60 remaining; all now resolve from this directory, including desktop
  docs at `../docs/desktop.md`.
- The original baseline `5802db8e68231d1e0f10422b39c176ff197f70f3` is historical.
  Current Regressify is `be843359a593b19d155c82bdcea0ee163e015423`.
- Several pilots lacked exact outcomes, and old open questions left important
  interaction choices unsettled. The interview now supplies those contracts;
  they replace contrary recommendations elsewhere in this document.
- The registry was rechecked 2026-10-06: 0.7.1 remains latest stable and
  non-yanked. Kit HEAD and published `v0.7.1` were compared; use only the
  published release. No speculative context-menu generation-ID failure,
  log-filter reset defect, or other new runtime bug is established here.
- API feasibility and lifecycle are pilot verification risks, not confirmed
  defects. If a genuine public API or behavior blocker remains after bounded
  investigation, stop that dependent migration and ask the user before deferring
  it or retaining a custom component. Unaffected tasks may continue.

Approved scope is the full native desktop plan: R1-R12, Other Matching
Components, and every collection/workflow pilot. React and browser automation
are out of scope. Prefer maintainable code reduction, but keep a correct
migration even if net code increases. Use stock kit styling, palette, typography,
and normal sizing for all desktop controls, including existing kit controls;
preserve Source editor font preferences and specialized image/editor geometry.
Keep toolbar commands visible and wrapping. Remaining detailed behavior choices
are recorded at their owning pilots below.

This is static source/API review, not a compiled or interactively tested
migration. The 0.7.1 `open_window(options, &mut App, builder)` returns
`Result<(AnyWindowHandle, Entity<V>)>`; the builder returns content, not `Root`,
and async callers use `cx.update`. Current launch manually mounts overlay layers;
the kit entry point owns them. Existing UI fixtures return `WindowHandle<Root>`
and must move with launch. The read-only clone was compared with published 0.7.1;
no clone changes or path dependencies are authorized.

## Component Map

| Existing UI                                       | Built-in candidate                                          | Recommendation                                                        |
| ------------------------------------------------- | ----------------------------------------------------------- | --------------------------------------------------------------------- |
| Run/update progress rectangles                    | `Progress`                                                  | Adopt, R1.                                                            |
| Numeric fields with custom step buttons           | `NumberInput`                                               | Adopt with draft/step adapter, R2.                                    |
| Field layouts and static sections                 | `Form`, `Field`, normal `GroupBox`                          | Adopt presentation, R3.                                               |
| Editor/run/source/log command bars                | `Toolbar`, `ToolbarGroup`                                   | Adopt with scoped keyboard/width checks, R4.                          |
| Manually owned suite/image popups                 | `ContextMenuExt`                                            | Prefer; image invalidation needs a lifecycle pilot, R5.               |
| Single/multiple value pickers                     | `Select`, `Combobox`                                        | Prefer with controlled values, R6.                                    |
| Always-available copy actions                     | `Clipboard`                                                 | Adopt where feedback timing fits; preserve gated Buttons, R7.         |
| Status footers, notices, labels, dots, dividers   | `StatusBar`, `Marker`, `Tag`, `Badge`, `Separator`, `Label` | Adopt matching primitives, R8.                                        |
| File/Publish/Help strip                           | `AppMenuBar`                                                | Pilot with exact action targeting and menu refresh, R9.               |
| Hand-composed dialog footers                      | `DialogFooter`, limited default button props                | Adopt footer layout; retain busy guards, R10.                         |
| Individual Git-file selection                     | Styled `Checkbox`                                           | Adopt; this call site does not need mixed state, R11.                 |
| Message-only errors                               | `Alert`                                                     | Adopt where alert semantics fit.                                      |
| Capture metadata                                  | `DescriptionList`                                           | Adopt with existing URL/copy children.                                |
| Expandable field/activity groups                  | `Accordion`, `Collapsible`                                  | Force full open for errors/search; restore user collapse state.       |
| Log source switch and archive paging              | `TabBar`, `Pagination`                                      | Use page-aligned transition from Follow, R12.                         |
| Command palette                                   | `Command`                                                   | Stock substring search; Escape clears, then dismisses.                |
| Whole-pane empty states                           | `Empty`                                                     | Adopt; optional slots need not all be populated.                      |
| Flat file/capture/action lists                    | `List`, `ListItem`                                          | Prefer targeted migration with scoped selection/activation.           |
| Checkable suite/scenario rows                     | `ListItem` and retained selection policy                    | Prefer row presentation; keep checked/open/focused states separate.   |
| Hierarchical Git change trees                     | `Tree` plus checkbox/action adapter                         | Pilot; keyboard locks and refresh identity need explicit handling.    |
| Overview/diagnostics tables                       | `Table`                                                     | Uniform overview rows with complete details dialog; keep diagnostics. |
| Mixed folder/suite selection                      | Base-backed checkbox adapter                                | Retain only mixed-state uses; styled API is insufficient.             |
| Breadcrumbs and guarded links                     | Existing toolkit Buttons/link variant                       | Retain; legacy Breadcrumb/Link lose required interaction behavior.    |
| Source commands, image comparison, split Git diff | Existing toolkit editor/virtualization                      | Retain domain integrations, not a competing widget framework.         |

The 0.6.1 source already exposes most of these component families. `Toolbar` and
`Empty` are absent from its public module list. The detailed adapters below were
checked against 0.7.1 and should follow the upgrade, not be assumed compatible
with the old pin.

## Phase 1: Upgrade GPUI Kit

This is an explicit prerequisite, independently reviewable from component reuse.

- Change the exact version pin to the latest published stable release. As of
  this review, the intended declaration is:

  ```toml
  gpui-kit = { version = "=0.7.1", optional = true, features = ["tree-sitter-yaml"] }
  ```

- Regenerate the dependency lockfile with `cargo update -p gpui-kit` after
  changing the constraint. That command alone cannot move past `=0.6.1`.
- Preserve `gui` as the optional dependency boundary and preserve
  `gui-tests = ["gui", "gpui-kit/test-support"]`. Do not add direct, independently
  versioned GPUI/Base/Component dependencies.
- Review release notes and the resolved dependency graph. Do not enable an
  alternative backend or unrelated optional features as part of this cleanup.
- Fix the startup incompatibility in [launch](../src/desktop/ui/mod.rs): use
  [gpui_kit::open_window][kit-window], whose builder returns workbench content
  rather than constructing `Root`. The release returns an
  `AnyWindowHandle` and content `Entity`; retain the entity for fixture access,
  use `cx.update` from async callers, and remove manual overlay mounting now
  owned by the kit entry point.
- Apply the same root/window migration to native test fixtures, including
  [UI tests](../src/desktop/ui/tests.rs). Retain the returned content entity
  for fixture access. Do not weaken interaction assertions to make tests compile.
- Preserve all startup and close behavior: workspace ownership/activation,
  Windows taskbar setup, unsaved-document and landing guards, cancellation,
  serialized preference saves, and update-install handoff.
- Resolve remaining compiler/API changes narrowly, then run the upgrade gates
  below before starting presentation migrations. A successful dependency resolve
  is not evidence of GUI compatibility.

No unpublished API, Git/path dependency, clone edit, fork, or vendoring is in
scope. No dependency upgrade was performed during this review.

## Phase 2: Small Replacements

### R1: Progress Bars

**Owners:** [run activity](../src/desktop/ui/runs/activity.rs#L300) and
[update download progress](../src/desktop/ui/updates.rs#L233).

**Replacement:** [Progress][kit-progress] supports a value, custom color, size,
accessible name, and optional indeterminate loading. It supplies the track,
indicator, animation, and progress accessibility behavior.

- Replace only the nested painted `div` elements. Retain application progress
  calculations, textual counts, cancellation, and update lifecycle logic.
- Convert fractions to percentages: `Progress::value` expects **0..100**, not
  0..1. Preserve the existing update ratio's denominator guard and clamping.
- Preserve the run bar's failure/cancelled/captured color distinctions. Use
  normal toolkit sizing and give each bar a stable ID.
- Preserve `update-download-progress` as a discoverable test boundary, using an
  outer wrapper if necessary. Supply a meaningful progress accessibility name.
- Do not invent an indeterminate state or equate download completion with
  verification/install completion. Those are separate application states.

**Expected deletion:** hand-painted tracks and width/style chains at both sites.
No progress accounting or update model should disappear.

**Checks:** zero/partial/full progress, zero totals, failed/cancelled/captured
runs, animation settling, and the existing `update_progress_` native regression.

### R2: Numeric Field Controls

**Owner:** numeric suffix buttons in
[render_fields](../src/desktop/ui/mod.rs#L2030), plus the existing field input
subscriptions and `adjust_field` in the same module.

**Replacement:** [NumberInput][kit-number] accepts the existing
`Entity<InputState>`. A supported 0.7.1 adapter can preserve Regressify's semantics:

- Explicitly construct numeric field states with
  `.mask_pattern(MaskPattern::None)`. [Mask selection][kit-input-state] records
  that the application selected a mask, so `ensure_number_mask` will not install
  its default numeric mask. Merely relying on the default mask is insufficient.
- Explicitly call `InputState::set_step(None, window, cx)` and leave `min` and
  `max` unset. Published 0.7.1 defaults to a fixed step of one; merely omitting
  the builder does not disable it. [Base stepping][kit-number-base] then emits
  `NumberInputEvent::Step` instead of calculating/replacing the text. This
  correction was verified by the inherited-value pilot and approved by the owner.
- Subscribe to that event and call the existing `adjust_field`, distinguishing
  increment/decrement. Keep `InputEvent::Change` routed through `change_field`.
- Retain inherited-value lookup, integer/fractional steps, schema limits,
  explicit-versus-inherited state, invalid text, reset, and document history.
- Render `NumberInput` only for numeric fields. Keep paths and other text fields
  as `Input`; do not introduce masking or numeric coercion there.
- Preserve field names, focus, and disabled state. Keep stable field wrappers;
  scope tests to the built-in increment/decrement controls instead of restoring
  custom buttons just to retain their old IDs. Verify the actual editable child,
  not only wrapper metadata.

**Expected deletion:** custom plus/minus button layout and callbacks. The
`adjust_field` method is application policy and must remain. The net saving must
include the new event subscription, not just the removed render chain.

**Checks:** [numeric stepping](../src/desktop/ui/tests.rs#L5533) and
[threshold inheritance/reset](../src/desktop/ui/tests.rs#L5405), extended for
invalid/intermediate text, exponent notation, keyboard stepping, busy state,
blur without coercion, and undo/redo. Pilot before applying to every number field.

The [Source font-size controls](../src/desktop/ui/source.rs#L1160) are another
eligible numeric surface. Replace the minus/value/plus composition with direct
integer NumberInput entry in the inclusive range 10..24. Valid integers apply and
persist; invalid or out-of-range drafts never persist, and blur restores the
last valid size. Preserve reset and preference behavior. Do not coerce
configuration numeric drafts: they remain lossless, including invalid text.
Image fit-mode percentage remains derived from laid-out image bounds and is not
the same preference control.

## Phase 3: Shared Presentation

### R3: Form And Field Layout

**Owners:** [Fields](../src/desktop/ui/mod.rs#L1834),
[run options](../src/desktop/ui/runs/settings.rs),
[deployment forms](../src/desktop/ui/deployment.rs),
[token forms](../src/desktop/ui/tokens.rs), and
[landing forms](../src/desktop/ui/landing.rs#L217).

**Replacement:** [Form][kit-form] and [Field][kit-field] provide label placement,
column layout, required markers, descriptions, custom label/description builders,
and field spans. They accept existing input entities and custom children.

- Start with one small run-options form, then assess the actual reduction before
  converting the larger configuration editor and dialogs.
- Alias toolkit `Field` as `FormField` to avoid confusion with the existing schema
  `Field`. Do not create a new form framework or move source into new UI modules
  merely to make the old module shorter.
- Use [GroupBox][kit-group-box] in its normal, unframed variant for static named
  sections such as run-option groups. Give repeated groups explicit stable IDs.
  Use controlled Accordion sections where expansion is part of the use case;
  do not add nested filled/outlined cards to the flat workbench.
- Use `Form::columns(2)` and field spans where the current editor has two columns;
  keep URL, description, list, structured, and action controls appropriately wide.
- Retain the inherited-value hint, reset button, and validation message using
  the existing child content or `label_fn`/`description_fn`. The toolkit form is
  not the validation engine and does not replace `Binding` or `field_errors`.
- Preserve stable input IDs and accessibility names explicitly; rendering a
  visible toolkit label alone does not establish the existing accessibility
  relationship or test contract.
- Retain masking/password content types, saved-secret placeholders, dynamic
  provider fields, asynchronous save locks, conflict handling, and draft guards.
- Keep landing and expanded deployment forms scrollable with their current
  retained handles and outer accessibility IDs. Preserve the flat page layout.

**Expected deletion:** repeated label/control wrappers and grid/spacing code.
Session, schema, credential, persistence, and asynchronous state code remains.
Form/Field is the best fit for the current grid. The Settings DSL is not wholly
incompatible: [SettingField::element][kit-setting-element] accepts existing custom
controls, so a settings presentation could preserve the current InputState and
callbacks. Do not substitute its typed fields for lossless draft/session policy.
Treat it as an alternative presentation, not a reason to build a second editor.

**Checks:** setting search, required/wide fields, hidden errors, inheritance and
reset, pending input on Save/navigation, password masking, provider switching,
long labels, offscreen fields, and compact/large light/dark layouts.

### R4: Toolbars

Toolbar controls wrap at the chosen minimum width; keep every command visible.
Use normal toolkit sizing and do not redesign the bars as overflow menus.

**Owners:** [editor toolbar](../src/desktop/ui/mod.rs#L1523),
[Source toolbar](../src/desktop/ui/source.rs#L1011),
[run command bar](../src/desktop/ui/runs.rs#L1648),
[log controls](../src/desktop/ui/log_controls.rs), capture-log headers, and
the command rows in [Git review](../src/desktop/ui/git/view.rs#L1257).

**Replacement:** [Toolbar and ToolbarGroup][kit-toolbar] own consistent control
sizing, layout, semantic groups, and roving keyboard focus.

- Pilot the editor toolbar first. Use inherent `child`/`children` for sized
  controls and `content` for spacers, labels, separators, or custom wrappers.
- Keep every command callback, tooltip, accessible name, and individual disabled
  condition. `Toolbar::disabled` disables toolbar navigation, **not its children**.
- Exercise arrow/Tab navigation and hosted input editing. Toolkit guidance puts
  inputs at the trailing end; do not consume search-input caret shortcuts.
- Retain application action contexts and the existing title-bar mouse-down
  propagation guard. The title bar itself already uses `TitleBar` and is not a
  replacement target.

**Expected deletion:** repeated bar geometry and compatible per-control sizing,
not button definitions or operation guards. A small compatibility wrapper is
acceptable when it transfers control behavior and styling to the toolkit.

## Phase 4: Additional Toolkit Adoption

The additional review follows the preference for toolkit ownership, not just
immediate line savings. These candidates supplement R1-R4; their behavior checks
are implementation gates, not claims that a migration has already been tested.

### R5: Managed Context Menus

**Owners:** `SuiteMenu`, `show_suite_menu`, and manual popup mounting in
[navigation](../src/desktop/ui/navigation.rs#L8); `ImageMenu`,
`show_image_menu`, and its overlay in [results](../src/desktop/ui/reports.rs#L953).

**Prefer:** [ContextMenuExt][kit-context-menu] on the actual row/preview trigger.
It owns right-click handling, popup placement, menu focus, dismissal subscription,
and the deferred overlay. Its internal dismissal subscription uses weak ownership.

- Start with suite menus. Retain the existing Rename/Delete callbacks and capture
  the clicked entry's path, not the active editor or all checked suites.
- Give every trigger an explicit stable ID. Unidentified triggers from the same
  render call site can share context-menu state. Do not use a filtered row index
  alone as identity when data can change while the menu is open.
- Preserve the busy/running guard and update the list's focused item on a valid
  right-click without opening the document or changing its checked selection.
  Reconcile the existing right-mouse propagation handler with the toolkit's
  bubble-phase handler; do not leave a handler that prevents the menu opening.
- Keep `action_context` and weak workbench references in callbacks. Verify Escape,
  focus restoration, edge-of-window positioning, and repeated open/close.
- For image menus, freeze capture/image identity and retain validated path/URL
  handling. Cancel panning when opening the menu. A capture refresh, mode change,
  or Results exit must dismiss it, as clearing `image_menu` does today.
- Treat explicit dismissal and invalidation as lifecycle pilot gates. Test
  capture/image identity across refresh, mode change, Results exit, focus return,
  and workbench release. Do not assume a generation-ID workaround is impossible
  or proven; investigate the released public API and ask the user before
  deferring or retaining a custom menu if parity is genuinely blocked.

**Expected deletion:** application popup structs, coordinates, subscriptions, and
manual `anchored`/`deferred` mounting where the lifecycle tests pass. Keep native
`suite_panel_` and image-menu interaction tests, including row reorder/refresh
while open and workbench release after closing the window.

### R6: Value Selectors

**Single values:** use [Select][kit-select] for `Control::Choice` in
[Fields](../src/desktop/ui/mod.rs#L2119), followed by value pickers in
[deployment](../src/desktop/ui/deployment.rs),
[tokens](../src/desktop/ui/tokens.rs), and
[Git remotes](../src/desktop/ui/git/view.rs#L18).

- Keep real command menus as menus: Add, Publish, Help, Run, and destructive
  actions are commands, not select values. Existing toolkit dropdowns need not
  be replaced merely because both APIs can display a popup.
- Retain `SelectState` with each binding/panel and subscribe only to user
  confirmation for writes. Programmatic `set_selected_value` does not emit a
  confirmation; use it to synchronize rebuilt or externally refreshed data.
- Refresh items and selected values by stable identity, not filtered indices.
  Keep reset separate from selection unless clearing has the same field policy.
- Preserve provider/token identity locks, draft guards, and operation-time
  validation. Route selection through the existing callbacks/guarded requests;
  it must not bypass those paths to perform a Git write or deployment.

**Multiple values:** pilot [Combobox][kit-combobox] as the existing guided picker
beside the free-form `viewportNames` and `needs` controls in
[choices](../src/desktop/ui/choices.rs). Do not remove the raw textarea.

- Use multi-select mode with an optional search field. This can keep the picker
  open across toggles instead of rebuilding the checked-menu interaction.
- Apply each accepted toggle immediately through the existing list-choice
  policy; create exactly one document-history entry per accepted toggle. Escape
  closes the picker without rollback. Keep the document authoritative rather
  than serializing the combobox's full selected-value list.
- Preserve unknown entries, scalar/list syntax, case-insensitive viewport labels,
  case-sensitive dependency IDs, inherited defaults, and the last-viewport guard.
- Distinguish `ComboboxEvent::Change` from `Confirm`: Change is emitted per toggle;
  Confirm is emitted on closing, including cancellation. Do not apply changes
  twice or interpret Escape as rollback after accepted toggles.
- Retain invalid-draft disabling and use a controlled selection/guard adapter to
- Retain invalid-draft disabling and use a controlled selection/guard adapter to
  reconcile a rejected toggle. Preserve concise trigger labels and full option
  descriptions; the kit provides a custom trigger builder.

**Checks:** native pointer and keyboard choice, same-value confirmation, data
refresh while searching, unknown source values, inherited resets, forbidden last
viewport removal, failed writes, and no duplicate document-history entries.

### R7: Clipboard Controls

**Prefer:** [Clipboard][kit-clipboard] for always-available text copy actions,
starting with the saved publication URL in
[deployment](../src/desktop/ui/deployment.rs#L407). Keep its existing accessible
name and use `value` for immutable text or `value_fn` for a current snapshot.

The component supplies the Copy/Check feedback and reset timer. Its two-second
repeat suppression is acceptable only for unchanged text: a changed selection
or value must be immediately copyable, using value/generation identity as
needed. It has no disabled API, so keep gated copies as guarded Buttons for
incomplete-log export or unavailable image URLs.

Reevaluate Git activity, source-side diff copy, update failure details, and
results URL copies individually. Copy exact source or validated URLs, not
rendered line numbers, display names, truncated labels, or masked credentials.
Test exact clipboard contents, selection changes during feedback, and failure
states; do not add copy actions for secrets that are not currently copyable.

### R8: Status, Labels, And Dividers

- **StatusBar:** replace the manual footer in
  [Source](../src/desktop/ui/source.rs#L1215) with
  [StatusBar][kit-status-bar]. Its left/center/right slots accept the existing
  language, caret position, completion status, and issue-navigation controls.
  Keep each child's status role/name and stable ID; the container does not add
  those semantics for the application. Use normal toolkit sizing and preserve
  layout fit without compact control overrides.
- **Marker:** [Marker][kit-marker] accepts arbitrary children and an explicit
  status role, making it a better candidate than Alert for passive operational
  notices with actions, such as update-ready and external-file notices. Keep
  existing wording, warning levels, accessible labels, and action callbacks.
  Do not enable streaming/shimmer behavior for ordinary status messages.
- **Tag:** use [Tag][kit-tag] for compact outcome/severity labels in overview and
  activity rows where a status label already exists. Keep outcome calculation in
  the models and test the chosen theme variant's contrast; do not turn all text
  into badges or change what counts as passed, captured, or failed.
- **Badge:** [Badge][kit-badge] can supply existing small status dots, but its
  indicator is absolutely positioned. It needs a nonzero-sized host and the
  existing accessible severity/status wrapper. Its numeric mode hides zero and
  truncates above a maximum, so it is not a replacement for exact run counters.
- **Separator:** replace standalone decorative line elements in Source, field
  group headings, and run-option headings with [Separator][kit-separator]. Give
  it explicit thickness/length where the old line occupies layout space. Do not
  replace panel borders, image rulers, or pointer guides with separator widgets.
- **Label:** [Label][kit-label] is a presentation/highlighting utility, not a
  form-label/focus-association primitive. It can consolidate an existing primary
  plus secondary text run, but does not replace input accessibility names or the
  multiline selectable diagnostics renderer by itself.

The relevant owners are [source UI](../src/desktop/ui/source.rs),
[run activity](../src/desktop/ui/runs/activity.rs),
[run options](../src/desktop/ui/runs/settings.rs), and
[external-file notices](../src/desktop/ui/files.rs#L239). Prefer these small
components even when an application still supplies domain text and callbacks.

### R9: Application Menu Bar

**Prefer a pilot:** [AppMenuBar][kit-app-menu-bar] for File/Publish/Help in
[the title bar](../src/desktop/ui/overview.rs#L65). It supplies cross-menu
keyboard navigation, hover switching, popup focus, and menu-bar semantics on
Windows/Linux. [Owned menu conversion][kit-menu-items] preserves checked and
disabled item flags, so dynamic enablement is supported.

- Populate the toolkit's [app-menu registry][kit-global-menus] and reload the
  retained menu-bar entity when relevant definitions/flags change, not every
  render. Preserve menu names, commands, busy states, and compact title-bar space.
- Preserve platform behavior explicitly; using AppMenuBar on Windows/Linux does
  not itself establish the macOS native-menu integration.
- Critical target difference: [File menu](../src/desktop/ui/files.rs#L384)
  explicitly uses the workbench focus context, so Rename/Delete target the open
  document. AppMenuBar captures the focused control, which might be the suite
  list, whose FileCommand handler targets its focused row. Use explicit
  application routing/target identity so adoption cannot rename another suite.
- Keep source-edit menu actions scoped to the source editor. Test menu switching,
  disabled items, stale menu snapshots, unsaved guards, focus restoration, and
  entity release after window close. A private action-context field is not an
  application API to depend on.

### R10: Dialog Footers And Actions

**Adopt:** [DialogFooter][kit-dialog-footer] for repeated footer alignment in
[file dialogs](../src/desktop/ui/files.rs), deployment/token/landing dialogs,
Git confirmation, and update dialogs. It accepts the existing Button children;
preserve wrapping, names, disabled states, and callbacks.

For simple non-busy dialogs, the toolkit's default button props can supply labels,
variants, and OK/Cancel handlers. Do not apply that indiscriminately: the default
button props have no disabled/busy setting, while New/Rename/Save and publishing
must block repeated submission and dismissal during asynchronous writes.

The public DialogAction/DialogClose composition is worth a focused pilot, but
keep the existing [name-dialog focus guard](../src/desktop/ui/files.rs#L632)
until Enter on input/primary action, Enter on Cancel/X, and busy-state tests prove
parity. `DialogDispatchAnchor` is private in 0.7.1; do not plan an application
integration using it. Save/Discard/Cancel and inline approval confirmations keep
their distinct application policies rather than becoming generic two-button prompts.

### R11: Binary Git Checkboxes

**Adopt:** styled Checkbox for individual files in
[tree_row](../src/desktop/ui/git/view.rs#L699). This call passes `partial = false`
and ignores the Base callback's event arguments. It can use the Boolean callback
without losing an existing mixed-state or modifier behavior.

Retain the file path, accessible name, conflict/submodule restrictions, and
active-operation disabling. Test keyboard/pointer toggling in both Changes and
Comparison Changes without opening the file or affecting the other group.
Folder/suite aggregate checkboxes remain on the mixed-state adapter. This is a
small independent migration; it need not wait for a full Tree replacement.

### R12: Log Source Tabs And Pagination

**Adopt:** segmented TabBar for the manually paired Reference/Test buttons in
[Capture Log](../src/desktop/ui/reports/log.rs#L133). Keep the existing source
selection and load callbacks, reset offset on source change, preserve unavailable
source messages, and reject stale reads. Use a stable tab-bar scope and update
native test selectors without dropping source-identity assertions.

**Adopt for non-tail archive pages:** [Pagination][kit-pagination] in Capture Log
and [run logs](../src/desktop/ui/runs/log.rs#L290). Use normal toolkit sizing.
The [reader](../src/desktop/logs.rs#L114) already returns `matched`, `offset`,
and bounded records, so no new full-log allocation or guessed total is necessary.

- For page-aligned results, use `matched.div_ceil(PAGE_RECORDS).max(1)` pages and
  `offset / PAGE_RECORDS + 1` as the current page. The kit API is one-based.
- Convert requested pages back to offsets with checked arithmetic, invalidate
  the existing archive key where required, and use the existing async loader.
- Reconcile/refetch when a filter or changed total makes the requested offset
  invalid; do not let the widget clamp its label while the model stays on an
  out-of-range page. Keep source/filter changes resetting offset.
- Preserve loading/availability guards and the partial/missing/legacy/error
  messages and export restrictions. Matched records are not proof that an
  incomplete archive contains every original record.
- Live follow-tail uses a rolling offset (`matched - returned_records`), which
  need not be divisible by PAGE_RECORDS. When leaving Follow, deliberately load
  the last numbered page using its page-aligned offset; do not freeze a rolling
  tail window and label it as a numbered page. Keep pagination non-tail only.

Test 0/1/500/501 matching records, first/last page, filtering on a later page,
shrinking totals, loading clicks, partial archives, source switching, and live
follow mode. Keep the bounded reader and full-filter export implementations.

## Other Matching Components

Prefer these where their presentation matches. They are not blocked merely
because they save fewer lines than the first batch.

- **Message-only errors:** [Alert][kit-alert] can replace simple error bands in
  run activity. Use it only when `Role::Alert` is appropriate; routine progress
  and log-health status must not become repeated assertive announcements. It has
  no general action-content slot, so use R8's Marker/container approach for
  update-install and external-reload notices, retaining their action buttons.
  Keep diagnostics and log health separate from test outcomes.
  Task 005's owner-approved exception uses presentational Marker inside the
  existing named `Role::Alert` wrapper for capture/approval errors: the published
  Alert plus a neutral wrapper lost the native accessible error name. Preserve
  one assertive region; full native comparison smoke verifies the replacement.
- **Capture metadata:** adopt [DescriptionList][kit-description] for matching
  label/value layout in [capture information](../src/desktop/ui/reports.rs#L1724).
  Use its stock styling and custom value elements for URL/copy controls.
  Preserve raw reference versus test URLs, signed dimension differences,
  unavailable values, safe URL handling, and existing test IDs.
- **Empty panes:** adopt [Empty][kit-empty] for no-workspace, no-capture, no-command,
  and comparable whole-pane states. Keep the real Open Workspace action and
  existing status/accessibility wrappers. Slots are optional: a simple message
  can be a direct child; it does not require a large header/media composition.
  Keep ordinary inline absence labels inline rather than stretching them into
  whole-pane empty states.
- **Expandable groups:** prefer controlled [Accordion][kit-accordion] triggers
  for Fields, workspace-files expansion, and applicable activity groups; use
  [Collapsible][kit-collapsible] when the trigger needs richer children. Keep
  multiple groups open, stable group IDs, filtered-index mapping, and Expand All.
  Force the full group open while it contains visible errors or search results,
  retaining the user's underlying collapse preference. Restore that preference
  when the condition clears; do not add a partial-error adapter.
- **Theme defaults:** adopt the stock toolkit palette with no brand color
  overrides and stock toolkit UI typography. Preserve Source editor font
  preferences separately. Use normal toolkit sizing for all desktop controls,
  including existing kit Buttons and inputs; retain specialized image/editor
  geometry, not former compact button sizes. Start with the attempted 1000x640
  minimum and increase only when layout evidence requires it, up to 1200x720.
  Test the chosen minimum and large windows in both themes; do not raise the
  minimum automatically. Visual review needs no per-surface approval pause.

## Collection And Workflow Pilots

### Flat Lists And Row Presentation

Prefer toolkit [List][kit-list] and [ListItem][kit-list-item] for the flat
[workspace files](../src/desktop/ui/navigation.rs#L1342),
[captures](../src/desktop/ui/reports.rs#L1081), and
[action sequence](../src/desktop/ui/sequence.rs#L107), one surface at a time.
Retaining application callbacks is normal integration, not a blocker.

- Keep current searches/filters or connect them to the delegate; do not render a
  second search input. Map positions to stable paths/capture/action identity.
- Preserve the distinction between changing the keyboard cursor and activating
  an item. File browsing must keep binary rows inert; capture changes must call
  `select_capture` and dismiss pending approval through the existing path.
- Preserve Home/End, scoped focus, and distinct activation. Single-selection
  lists and the Git tree may use the toolkit's wrapping Up/Down behavior. Keep
  suite/scenario checked-range policy authoritative; do not move shortcuts into
  search, commit-message, or source inputs.
- Keep virtualization and scroll-to-selection. Defer parent mutations when a
  delegate callback would otherwise re-enter the currently borrowed list entity.
- Use ListItem presentation for checkable suite/scenario rows even if their
  current selection controller remains. Checked, focused, and opened are separate
  states; a single toolkit selected index cannot represent all three.
- ListItem's inherent click/mouse handlers respect its disabled state; general
  InteractiveElement listeners and embedded children need their own guards.
  Retain tooltips and row names. A standalone ListItem is not a substitute for
  the keyboard-owning list container.

[SidebarMenuItem][kit-sidebar-menu] is useful presentation but is not a direct
keyboard-equivalent replacement for existing Buttons: its main row is rendered
as a clickable div without the corresponding focus/key setup. Prefer List for
keyboard-driven collections, or reuse sidebar groups around existing controls.
Do not remove [lists.rs](../src/desktop/ui/lists.rs) until its remaining callers
no longer need the shared application-specific operations.

### Git Tree

Pilot [Tree][kit-tree] for the existing Changes and Comparison Changes hierarchies.
It can own visible flattening, expansion, scrolling, and standard tree structure,
replacing parts of [rebuild_tree](../src/desktop/ui/git/view.rs#L570).

Use two tree states, IDs based on group plus repository path, and retained
application checkbox selection. Preserve folder aggregation, conflict/submodule
exclusions, group-specific Select All, and reviewed request identity.

The [TreeState implementation][kit-tree-state] establishes concrete adapter work:
`set_items` clears cursor selection; restore it by stable item ID and preserve
expansion. Permit wrapping Up/Down, provide Home/End, and keep cursor movement
distinct from leaf activation. Item `disabled` gates pointer handling, not all
keyboard navigation/expansion, so enforce mutation locks in application policy.
Keep read-only folder expansion and diff opening available during Git operations;
lock checkbox changes and mutation commands. Keep the in-flight reviewed request
identity immutable, reject stale diff responses, and test these effects. Prefer the kit tree even if
checkbox policy still occupies application code.

### Tables

Pilot [Table][kit-table] for the [overview](../src/desktop/ui/overview.rs#L307)
with uniform rows. Keep the suite checkbox, summary, status, and Edit/Open Report
actions separate while the kit owns columns, header alignment, and scrolling.

The limitation is row geometry, not merely delegate boilerplate:
[TableState][kit-table-state] uses uniform rows derived from a single Size;
custom Size changes every row's height, not individual row heights. Preserve
complete selectable long summaries and errors in an explicit details dialog.
Open it with a Details control or Enter on the focused row; row selection/click
alone must not open it. Keep row checkboxes/actions separate and discard no data.

The [diagnostics table](../src/desktop/ui/source.rs#L829) has independently
wrapping, selectable messages. Do not force those into clipped uniform rows.
Keep its current layout unless the resolved toolkit provides the required height
behavior or the owner approves an equally accessible full-message presentation.

### Command Palette

Prefer a guarded [Command][kit-command] prototype over a permanent custom
ListDelegate. Keep the command catalog, enablement, and existing execution paths.
An actionless CommandItem plus explicit `on_confirm` can close the palette,
restore the workbench context, and dispatch exactly once in the required order.

Use the toolkit's stock substring search, not an all-words compatibility
adapter. Escape clears a nonempty search first; the next Escape dismisses the
palette. Use an actionless item with `on_confirm` to close, restore workbench
focus, and dispatch exactly once in the approved order. Add no second
command-routing framework.

## Specific Retentions

### Tri-State Selection Checkbox

[selection_checkbox](../src/desktop/ui/design.rs#L13) paints a Base checkbox
for suite, run, and Git selection. The release's [styled Checkbox][kit-checkbox]
has Boolean checked state but no indeterminate-state builder. Its callback also
does not expose the same Base state/event signature used by these callers.

Keep this small, centralized adapter for mixed-state uses only; R11 removes a
binary use from it. Do not show partial selection as simply checked or unchecked.
Retire it when an upstream release supplies mixed-state styling/semantics and
the relevant interaction tests pass, not by creating a local component fork.

### Breadcrumbs

Current [editor breadcrumbs](../src/desktop/ui/mod.rs#L2298) use actual Buttons
with focus behavior, accessible names, tooltips, disabled state, and stable IDs.
[BreadcrumbItem][kit-breadcrumb] renders a clickable `div`; its ID setter is
private and it does not install the corresponding focus/keyboard behavior.

Keep the current buttons. A similarly named component is not enough to justify
losing keyboard activation, long-label handling, or testability.

The [legacy Link][kit-link] is also not the right replacement for guarded links:
the release preserves pointer-only behavior and its stored disabled flag is
ineffective, as its own tests document. It invokes both href opening and the
callback when both are supplied. Prefer the existing accessible `Button::link`
style with the current validated URL/opening callback instead.

### Command Palette

[commands.rs](../src/desktop/ui/commands.rs) already uses toolkit
`List`, `ListState`, and `ListItem`. Most of the command catalog and enablement
rules must remain even if [Command][kit-command] replaces the delegate.

Concrete differences to resolve before a migration:

Approved behavior is the toolkit's stock substring search, not all-words
matching. Escape clears a nonempty search, then a subsequent Escape dismisses.
Use an actionless item and `on_confirm` to close, restore workbench focus, and
dispatch exactly once in the approved order. Keep `command_palette_` tests and
add regressions for substring search, Escape sequencing, focus restoration, and
single dispatch. This is a pilot gate, not a confirmed API defect.

### Source, Logs, And Comparison Views

- [Source editing](../src/desktop/ui/source.rs) already uses the toolkit editor,
  undo/redo, completion, hover, and diagnostics APIs. Schema providers and draft
  synchronization are application integrations, not replacement widgets.
- Keep literal-diagnostic escaping for the current TextView path. The released
  [TextViewState][kit-text-state] exposes Markdown and HTML construction, not a
  plain-input format. `SelectionFormat::Plain` changes copied output, not parsing.
  Plain Label/Text does not supply the same selectable wrapped rich-text behavior;
  removing escaping would let diagnostic text become markup or links.
- Keep the line-operation helpers. The 0.7.1 [block-indent code][kit-indent] still
  includes `start_row..=end_row`, whereas Regressify excludes a selection's
  column-zero exclusive end line. No matching built-in comment/move/duplicate
  line action was found in the inspected input action implementation. Retain
  strict JSON behavior, CRLF/Unicode handling, focus guards, and native history.
  Task 001 exposed a published setter regression: reverse ranges collapse.
  The owner explicitly approved restoring forward-oriented selections after
  line commands, preserving selected text and exact edits. Use ascending ranges
  and retain a keyboard-created reverse-selection regression; no toolkit edit.
- [Run logs](../src/desktop/ui/runs/log.rs), deployment logs, and Git activity
  already use read-only `CodeEditor`. Their filters, full-text copying, archive
  completeness, follow-tail behavior, and bounded loading remain application work.
- [Image comparison](../src/desktop/ui/reports.rs#L1468) includes pointer-centered
  zoom, panning, independent fit modes, synchronized comparison behavior, and
  approval identity. A carousel or generic image element is not an equivalent.
- [Split Git diff](../src/desktop/ui/git/view.rs#L1257) already uses virtual lists
  and shared scroll geometry. Syntax highlighting for a raw diff does not replace
  aligned before/after rows, original line numbers, change navigation, and exact
  source copying. Keep the bounded diff model and fallback behavior.
- Dialog containers already come from the toolkit; filesystem pickers use `rfd`.
  Keep the application-specific pending actions, busy dismissal guards, conflict
  checks, and asynchronous save acknowledgements.

## Catalog Coverage

The [released public catalog][kit-catalog] was screened beyond the initially
obvious components. No new product feature is proposed merely to consume a
component. This is a family/use-case inventory, not a claim that every upstream
implementation was runtime-tested.

- **Forms:** input, form, group_box, setting, select, combobox, searchable_list are
  covered by R2/R3/R6 and the Settings custom-element alternative.
- **Navigation:** list, tree, sidebar, table, command, breadcrumb, tab, pagination
  are covered by the collection pilots, R12, and the specific keyboard limits.
- **Commands and overlays:** button, menu, native_menu, dialog, tooltip, hover_card,
  popover are existing reuse or R4/R5/R9/R10. Sheet would be a workflow redesign,
  not a missing replacement for a custom sheet implementation.
- **Feedback/content:** progress, status_bar, marker, tag, badge, alert, empty,
  clipboard, label, link, and text have specific candidates/limits above.
  Spinner, skeleton, shimmer, and notification can serve appropriate future
  states, but there is no custom animation/toast engine here to replace. Do not
  turn persistent errors or completion status into disappearing notifications.
- **Existing choice controls:** checkbox, radio, switch, slider, color_picker,
  calendar, date_picker, time_field, rating, and stepper were considered by use
  case. Existing Boolean/mode controls already use kit components. A calendar for
  Cloudflare's date string or sliders for numeric settings would be a separate
  input-design choice, not removal of a custom date/slider widget. The released
  [Stepper][kit-stepper] derives passed-step presentation from the selected index;
  editing action 4 does not mean actions 1-3 passed. Prefer ListItem presentation
  for the editable/reorderable action sequence rather than a progress stepper.
- **Layout/infrastructure:** resizable, scroll, virtual lists, Root, title bar,
  window border, theme, global state, highlighter, history, and kbd are already
  toolkit infrastructure or part of the upgrade/theme review. Keep the headless
  document history separate from GUI editor history. Dock is not needed merely
  to replace existing toolkit resizable panels.
- **No matching current surface:** chart/plot, avatar, attachment, carousel,
  bubble, message, message_scroller, questionnaire, and speech do not replace an
  existing custom chart, avatar, attachment manager, chat, wizard, or voice UI.
  The comparison inspector is not an image carousel; workspace path inputs and
  native file pickers are not an attachment-upload workflow.

## Interview Decision Ledger

The completed 36-answer interview is the implementation contract; these choices
supersede old proposal text. Full scope is R1-R12, Other Matching Components,
and all collection/workflow pilots. Use stock palette, typography, and normal
control sizing throughout, with no brand color overrides; retain Source font
preferences and specialized image/editor geometry. Preserve toolbar wrapping and
visible commands. Start with 1000x640 and increase only on layout evidence, up
to 1200x720; review both themes at the chosen minimum and a large window.

- Command uses stock substring search. Escape clears a nonempty query, then
  dismisses; an actionless item closes, restores focus, and dispatches exactly
  once. Controlled field groups fully open for errors/search while retaining the
  user's collapse preference.
- Overview Table uses uniform rows and a Details dialog opened by its control or
  Enter on the focused row. Selection alone does not open it; long content stays
  complete and selectable. Diagnostics remain variable-height and selectable.
- Source font size is an integer NumberInput from 10 through 24; invalid values
  never persist and blur restores the last valid size. Config numeric drafts stay
  lossless. Single-choice unknown values remain visible and invalid until an
  explicit user choice. Multi-choice accepted toggles write immediately, one
  history entry each; Escape closes without rollback, retaining raw text and
  unknown values.
- Single-selection lists and Git tree may wrap Up/Down while preserving Home/End,
  focus scope, activation, stable identity, and selection rules. During Git
  operations permit read-only navigation, expansion, and diff opening; lock
  selection mutations and commands, preserve request identity, reject stale diffs.
- Leaving log Follow loads the last page-aligned numbered page. Clipboard's
  two-second suppression applies only to unchanged text; changed content remains
  immediately copyable. Keep tri-state Checkbox, keyboard Breadcrumb/guarded
  links, diagnostic escaping, line-operation policy, image comparison, and split
  Git diff.

Genuine public API or behavior blockers require bounded investigation, then a
user decision before deferring or retaining a custom component. No toolkit edits,
issues, PRs, vendoring, forks, or path dependencies are authorized. Unaffected
tasks may proceed while awaiting a blocker decision.

## Delivery And Acceptance

The [task bundle](gpui-component-reuse-tasks/README.md) defines 20 separately
verifiable implementation units, their dependency order, acceptance checks,
and the live implementation/verification ledger. Its task order refines these
delivery groups without changing their approved behavior:

1. **Baseline and upgrade:** record the current feature/test baseline, recheck
   the published release, update the exact 0.7.1 pin and lockfile, and migrate
   launch plus UI fixtures to `open_window`. Preserve startup, close, and overlay
   ownership; resolve compile/API changes before presentation work.
1. **Foundational controls:** implement R1 progress and R11 binary checkboxes,
   then R2 numeric controls including Source font size. Retain application
   accounting, draft/history policy, selection semantics, and operation guards.
1. **Shared presentation:** pilot R3 forms and R4 toolbars; migrate R8 status,
   labels, separators, and matching components using normal toolkit sizing.
1. **Stock theme and layout:** separately verify stock palette, typography, and
   both themes. Collect layout evidence before changing the 1000x640 minimum;
   increase only as needed and never beyond 1200x720.
1. **Menus, values, and dialogs:** implement R5-R7 and R9-R10. Follow exact
   action targets, selection identity, guarded writes, per-toggle history,
   immediate changed-value copying, and close/focus/dispatch ordering. Ask the
   user before deferring any genuine blocker.
1. **Collections and workflows:** pilot flat lists, R12, overview Table, Git
   Tree, and Command. Preserve checked-range policy, full details, read-only Git
   navigation during operations, immutable request identity, and stale-response
   rejection. Keep diagnostics variable-height and selectable.
1. **Complete adoption and parity:** cover remaining Other Matching Components,
   verify all explicit retention decisions, update affected desktop docs/tests/
   smoke flows and stale skill or memory guidance, then run focused and required
  feature/platform gates. Record results in the task bundle.

For each task, compare deleted rendering code with added adapters, state,
subscriptions, and wrappers. Maintainability is the objective, not an arbitrary
net-line threshold; do not count relocated code or removed tests as savings.
Keep public CLI behavior and headless desktop models independent of GUI code.
No toolkit fork, component vendoring, generic UI framework, or dependency beyond
the approved version update is in scope.

After each task is verified, the user authorizes a local commit for that task.
Do not create branches, push, publish, install releases, change credentials, or
operate on real Git remotes. Delegation is allowed for independent tasks; avoid
shared-file conflicts and serialize builds/commits.

Native Windows smoke may use isolated fixtures and temporarily take focus,
pointer, or clipboard only with backup and restore. Honor the unlocked-desktop and
no-other-instance guards; never close user windows. On Linux, use an existing
local environment if available; otherwise leave the Linux gate explicitly
pending for CI. Do not install WSL or report Windows as Linux verification.

Implementation follow-up decisions are recorded in the task ledger. The owner
approved forward-oriented Source selections after line commands because the
0.7.1 setter collapses reverse ranges, the bounded schema/smoke baseline repairs,
and preserving input focus during Git activity refresh. Linux baseline failures
reproduced on the pre-upgrade export are explicitly allowed to remain separately
tracked while Windows migration continues; current Linux CI is required before
release, and the local Linux gate remains non-green.

## Verification Plan

The commands below are future implementation gates, not test runs claimed by
this review. Reuse existing test modules rather than creating a parallel suite.

Start with a GUI compile for the upgrade, then behavior-scoped native tests:

```sh
cargo check --locked --features gui --bin regressify-desktop
cargo test --locked --features gui-tests --lib numeric_stepper_ -- --test-threads=1
cargo test --locked --features gui-tests --lib pixel_match_threshold_ -- --test-threads=1
cargo test --locked --features gui-tests --lib update_progress_ -- --test-threads=1
```

For the affected slice, also use existing filters such as `setting_search_`,
`source_line_commands`, `external_reload`, `suite_name_dialog_`,
`command_palette_`, `navigator_`, `image_previews_`, `deployment_activity_`, and
`desktop::ui::git::tests`. Confirm a nonzero test count. Add targeted regressions
for NumberInput, toolbar keyboard/wrapping, menu target/invalidation, controlled
pickers, binary versus mixed checkboxes, tree keyboard locks, and collection
refresh identity. For R12 use `desktop::logs::tests` with the desktop feature,
and `run_log_` / `log_health_warning_` with gui-tests, extending existing modules
for page boundaries, follow-tail, source tabs, and shrinking filtered totals.

After focused checks, run the affected gates from
[Desktop Checks](../.github/workflows/desktop-checks.yml):

```sh
cargo fmt --all -- --check
cargo test --locked --all-targets
cargo test --locked --features gui-tests --all-targets -- --test-threads=1
cargo clippy --locked --features gui --lib -- -D warnings
cargo build --locked --features gui --bin regressify-desktop
cargo test --locked --features desktop --lib
```

The final command also belongs on the Linux headless-model CI job; a Windows
run alone does not establish Linux coverage. Preserve supported feature gates
rather than substituting an all-features build.

Run native Windows smoke flows relevant to changed surfaces: `-SchemaFlows`,
`-ComparisonFlows`, `-GitFlows`, `-PublishingFlows`, and `-DeploymentFlows` on
[desktop-smoke.ps1](../ci/desktop-smoke.ps1). Root migration affects all of
these surfaces. Inspect compact/large light/dark screenshots, long labels,
keyboard focus, actual input values, scrolling, and clipboard behavior.

Smoke requires an unlocked, isolated desktop and no other app instance. Do not
bypass its guards or close the user's windows. Use `-SkipBuild` only with a
current binary. Browser/Playwright tests are not a substitute for native GPUI
interaction tests, and live publishing is not required for this migration.

Planning and interview work was committed as `278be67`. Task 001 has a compiled
0.7.1 candidate, focused native regressions and passing Windows smoke flows;
the task bundle records exact outcomes, approved exceptions and remaining gates.
Component adoption has not started. No binary-size measurement is claimed.

[kit-registry]: https://crates.io/api/v1/crates/gpui-kit
[kit-window]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/kit/src/lib.rs#L149
[kit-progress]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/progress/progress.rs
[kit-number]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/input/number_input.rs
[kit-number-base]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/base/src/number_input.rs
[kit-input-state]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/base/src/input/base/state.rs#L3565
[kit-form]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/form/form.rs
[kit-field]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/form/field.rs
[kit-toolbar]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/toolbar.rs
[kit-alert]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/alert.rs
[kit-description]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/description_list.rs
[kit-empty]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/empty.rs
[kit-accordion]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/accordion.rs
[kit-collapsible]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/collapsible.rs
[kit-checkbox]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/checkbox.rs
[kit-breadcrumb]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/breadcrumb.rs
[kit-list]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/list/delegate.rs
[kit-tree]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/tree.rs
[kit-table]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/table/delegate.rs
[kit-command]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/command/command.rs
[kit-indent]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/base/src/input/editor/indent.rs#L290
[kit-context-menu]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/menu/context_menu.rs
[kit-select]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/select.rs
[kit-combobox]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/combobox.rs
[kit-clipboard]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/clipboard.rs
[kit-status-bar]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/status_bar.rs
[kit-marker]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/marker.rs
[kit-tag]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/tag.rs
[kit-separator]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/separator.rs
[kit-group-box]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/group_box.rs
[kit-setting-element]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/setting/fields/element.rs
[kit-badge]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/badge.rs
[kit-label]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/label.rs
[kit-app-menu-bar]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/menu/app_menu_bar.rs
[kit-menu-items]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/menu/popup_menu.rs#L784
[kit-global-menus]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/base/src/global_state.rs#L112
[kit-dialog-footer]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/dialog/footer.rs
[kit-pagination]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/pagination.rs
[kit-list-item]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/list/list_item.rs
[kit-sidebar-menu]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/sidebar/menu.rs
[kit-tree-state]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/base/src/tree.rs
[kit-table-state]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/table/state.rs
[kit-link]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/link.rs
[kit-text-state]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/base/src/text/state.rs
[kit-catalog]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/lib.rs
[kit-stepper]: https://github.com/longbridge/gpui-kit/blob/v0.7.1/crates/component/src/stepper/item.rs
