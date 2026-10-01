<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 026 — Lazy-loaded log viewer component

- Slug: `frontend-log-viewer-component`
- Implements plan section(s): `Phase 6 — Frontend and Packaging#0`, `Snapshot Frontend and Publication#0`, `Desktop and CLI Experience#0`
- Depends on: frontend-report-schema-log-fields

## Goal

Build a lazy-loaded Capture Log viewer component for the snapshot frontend, including explicit legacy, missing, partial, and loading-error states, deep-linkable capture selection, and a responsive desktop/mobile layout, loading its bounded JSON chunk only when the view opens.

## Why this exists

Task 6.2's own Task Breakdown acceptance criteria require (a) lazy loading only on view-open, (b) a deep-linkable URL that survives a full page refresh without losing or misattributing the selected log, and (c) usable filter/search/content layout at both mobile and desktop viewport widths; Desktop and CLI Experience additionally requires this viewer to distinguish all four of legacy, missing, partial, and loading-error states explicitly, matching Task 2.4's engine-side model.

## Settled investigation

- frontend/src/pages/ImagePage.tsx (90 lines) is the existing per-capture review page (useParams() for suiteId/imageId, useSuite(suiteId) from frontend/src/data/store.tsx, filterCaptures/imagePath/suiteLabel/suitePath from report.ts) — this is the natural existing route/page this task's Capture Log view attaches to.
- frontend/src/data/report.ts::fetchReport<T>(url, schema, signal?, reload?) (lines 135-145) is the existing generic lazy-fetch-plus-zod-validate helper, already used for suite report JSON; this task's log-chunk fetch should reuse this same function.
- React Router (imported in ImagePage.tsx line 2) is this frontend's existing routing library; a deep-linkable URL should be expressed as a route/search param this router already supports, not client-only in-memory state that a refresh would lose.
- Task 2.4 (legacy-migration-behavior) resolves a 5-variant ArchiveState (Complete, Partial, Missing, LegacyUnavailable, LoadingError); Task 6.1's completeness metadata is required to match these variants, so this viewer must render four distinct non-complete messages, not only a partial/legacy pair.

## Inputs

- Task 6.1's execution-log schema/asset reference
- frontend/src/pages/ImagePage.tsx and frontend/src/data/store.tsx (extended, not replaced)
- frontend/src/data/report.ts::fetchReport (reused)

## Outputs

- A new Capture Log viewer section/component reachable from ImagePage.tsx, lazily fetching its log chunk via fetchReport, with a deep-linkable URL and explicit, distinct rendering for each of the legacy, missing, partial, and loading-error completeness states

## Implementation steps

- Add a log-view route/search-param (e.g. a mode=log query param, distinct from ImagePage.tsx's existing mode param stripped at line 32) so the selected capture's log view is addressable by URL.
- Build the viewer component to call fetchReport with the Task 6.1 log-asset URL only when this log view is actually opened, and to render each of Task 2.4's four non-complete states explicitly — legacy, missing, partial, and loading-error — matching the schema's completeness metadata; never render a missing/corrupt case as an empty-but-complete log.
- Render log content as plain text, never HTML/Markdown/executable.
- Verify the deep-linked URL survives a full page refresh by loading the viewer directly at that URL and confirming it resolves to the same capture's log.
- Test the filter/search controls and log content layout at a 375x667 mobile viewport and a 1280x800 desktop viewport, confirming no clipped or overlapping elements at either.
- Test the viewer against four fixture captures whose schema completeness metadata is each one of legacy, missing, partial, and loading-error in turn, confirming each produces its own distinct, explicit message with none rendering as a successful/complete log.

## Done when

- A browser test (capture-log lazy loading spec) shows zero network requests for the log JSON chunk's URL before the Capture Log view is opened, and exactly one request after it opens.
- A browser test opens a specific capture's log view via its deep-linked URL (fresh load) and separately performs a full page refresh at that same URL, with both cases rendering that same capture's log, not a different or missing one.
- A browser test rendered at a 375x667 viewport (mobile) and separately at a 1280x800 viewport (desktop) both show the log view's filter/search controls and log content without any element clipped or overlapping.
- A browser test renders the viewer against four fixture captures — one each for legacy, missing, partial, and loading-error completeness metadata — and asserts each shows its own distinct, explicit state message, with none of the four rendering as a successful/complete log.

## Evidence to capture

- Network-request evidence from the e2e spec showing 0 requests for the log chunk before open and exactly 1 after
- The exact deep-link URL format chosen (e.g. a `mode=log` query parameter alongside ImagePage's existing route), for Task 7.3's documentation
- The four fixture completeness states (legacy, missing, partial, loading-error) used in the archive-states browser test and their four distinct resulting messages

## Do not change

- Do not render log message content as HTML or Markdown, and do not execute any content from the log payload.
- Do not fetch the log chunk eagerly as part of the suite's initial data load; it must be lazy, on view-open only.
- Do not render legacy, missing, partial, or loading-error with the same message text as another of the four, or as the successful/complete state.

## Reopen triggers

- Task 6.3's packaging work finds the chosen log-asset URL/versioning scheme does not fit cleanly into the existing content-hash/staging pipeline, requiring this task's asset-reference shape to change.

## Context

This is the user-facing payoff of Task 6.1's schema work on the web side, mirroring Task 5.3/5.4's desktop Capture Log view with the frontend's own existing lazy-fetch and routing conventions, per the snapshot-command skill's explicit requirement to match desktop concepts for attempts, severity, stage, search, download, and partial/legacy states.

## Options & tradeoffs

Eagerly including log content in the already-fetched suite report JSON would avoid a second fetch but directly violates Snapshot Frontend and Publication's 'Prefer separately fetched, bounded JSON chunks... rather than inflating every suite JSON'; a separate on-demand fetch via the existing fetchReport helper (chosen) adds one more network round-trip per log view but keeps the common case fast.

## Decision

Fetch the log chunk lazily via the existing fetchReport helper only when the Capture Log view opens, addressed by a deep-linkable route/query parameter on the existing ImagePage route, rendering each of Task 2.4's four non-complete states (legacy, missing, partial, loading-error) with its own explicit message.

## Acceptance criteria

- A browser test asserts zero requests for the log JSON chunk occur before the Capture Log view is explicitly opened, and exactly one request occurs after it opens.
- A browser test navigates directly to a capture's deep-linked log URL (fresh load) and separately full-page-refreshes that same URL, confirming the correct capture's log is shown both times, not a different or missing one.
- A browser test against four fixture captures — legacy, missing, partial, and loading-error completeness metadata respectively — shows four distinct, explicit state messages, with none of the four indistinguishable from a successful/complete log.

## File touchpoints

- frontend/src/pages/ImagePage.tsx
- frontend/src/data/store.tsx
- frontend/src/components/CaptureInfo.tsx
- frontend/src/data/report.ts (reference)

## Tests

- bun run test:e2e --grep 'capture log' (new e2e spec for lazy loading, deep link refresh, and mobile/desktop layout)
- bun run test:e2e --grep 'capture log archive states' (new e2e spec asserting four distinct messages for legacy/missing/partial/loading-error completeness metadata)

## Dependencies

- frontend-report-schema-log-fields

## Risks

- Expressing the deep link as a query param that collides with the existing mode/filter query params (query, status from useFilters()) could cause state to leak between unrelated UI concerns; mitigate by namespacing the new param distinctly and covering it in the e2e test alongside the existing filter params to confirm no interference.
- Hand-writing four separate state messages risks two of them converging on similar wording that a user (or a loose test assertion) cannot actually distinguish; mitigate by asserting on a distinct state-identifier attribute/test-id per message in the browser test, not only on visible text.
