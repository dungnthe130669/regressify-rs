# Logging Revamp Plan

Status: Proposed, with product decisions confirmed on 2026-10-01; ready to refine
into implementation tasks.

## Goals

- Make capture and deployment failures diagnosable from recorded stages, timings,
  decisions, retries, and error causes, without reproducing the run first.
- Treat the CLI, native desktop app, and snapshot frontend as first-class log
  consumers. Engine code must not depend on a terminal or UI framework.
- Retain the full supported scenario capture log for passed and failed captures,
  including reference captures, and make it available in desktop and web reports.
- Preserve existing exit semantics, scenario isolation, worker cancellation,
  report compatibility, and credential boundaries.

This is a design plan, not a change to current behavior. It does not propose a
hosted observability service, a new deployment command, or unrestricted collection
of page content, browser console output, or network bodies.

## Current Evidence

- [Cargo dependencies](../../Cargo.toml) do not currently declare `tracing`,
  `tracing-subscriber`, or `log` as direct dependencies.
- [Progress telemetry](../../src/progress.rs) already separates typed lifecycle
  events from UI rendering and writes version-1 JSON records to an installed file
  sink. Records contain elapsed time; scenario messages carry a process-local job
  ID but no severity, structured fields, stage, or durable execution identity.
- The same module's `scenario_log!` always writes to stderr. It also emits a
  structured `Log` event when a sink and task-local scenario context are present.
  `LiveProgress` retains a bounded shared log history: 2 MiB, 4,096 entries, and
  16 KiB per retained message. That buffer cannot be the full-log source of truth.
- The existing [desktop scenario-log view](../../src/desktop/ui/runs/log.rs)
  filters that retained history by job and exposes omitted-entry notices.
  [Saved Activity](../../src/desktop/runner.rs) serializes `LiveProgress` and
  bounded console output, not a capture-owned full archive. Extend these surfaces
  rather than introducing a competing log viewer.
- The [bundled worker](../../src/desktop/worker.rs) installs and finishes the event
  transport. The [CLI entry point](../../src/main.rs) does not install that sink
  and terminates with `process::exit`, which skips drop-based writer cleanup.
- [Capture orchestration](../../src/test/test_suites.rs) already scopes spawned
  scenario tasks by job and owns capture-run manifests. These are the starting
  points for durable identity and capture-to-log linkage.
- [Deployment orchestration](../../src/desktop/deployment.rs) emits typed phase
  events but also writes phases and heartbeat messages directly to stderr.
- The [frontend capture schema](../../frontend/src/data/report.ts) exposes capture
  results and an error string, but has no execution-log reference or log schema.
- [Snapshot publication](../../src/snapshot/site.rs) and
  [deployment staging](../../src/desktop/deployment.rs) use explicit, versioned
  report dependencies. New public log assets must join that contract, not be
  collected by recursively copying a diagnostics directory.

Working hypothesis: extending the existing structured event pipeline with
durable, capture-linked diagnostics will solve the main visibility gap. Replacing
`eprintln!` alone, or increasing the desktop ring-buffer limits, will not make full
logs available in saved and deployed reports.

The first implementation check should follow two interleaved scenario jobs, one
passing and one failing, from emission through persistence and report loading.
Both clients must recover each complete timeline after live buffers are evicted.

## Istari Design Review

Review date: 2026-10-01. Compare the implemented `istari::common::logging` module
and its callers/tests, not just Istari's historical logging proposal. This review
uses source and synthetic tests only, not private run logs or credentials.

Source baseline: Istari commit `cd7870a0192fa0fb78abfd23e1e5992960fc6522`.
Evidence anchors below are Rust symbols in that repository, not proposed
Regressify APIs. The inspected logging, redaction, and entry-point sources were
unchanged from that revision at review time.

### Findings and Limits

- **Bootstrap handoff can discard recovery data.**
  `common::logging::LoggingGuard::attach_project_log` warns when the bootstrap
  copy fails, but then reports that it copied the log and attempts to remove the
  source once the destination writer opens. Retain the source until a verified
  successful handoff, and never emit a successful-copy event on that error path.
  This is a source-control-flow finding; failed-copy injection was not executed.
- **Non-lossy enqueueing is not a persistence acknowledgement.**
  `common::logging::LoggingState::emit` ignores write results, and
  `LoggingGuard::shutdown` returns no writer-health result. Regressify must expose
  degraded logging and incomplete archives, even though diagnostic-only write
  failures are warnings rather than command failures under the decision below.
- **A shared lock spans potentially blocking output.** `LoggingState::emit`
  holds its mutex during formatting, terminal writes, and non-lossy queue writes.
  Do not copy this onto the GPUI thread or let disk pressure serialize all capture
  work indefinitely. Choose bounded worker-side backpressure with explicit
  degradation, and test slow writers and cancellation.
- **The custom layer is not span-aware.** `IstariLayer::on_event` ignores its
  context and records only fields on each event; `EventVisitor` converts field
  values to text. Regressify must resolve inherited span fields into typed
  records and propagate context across async/blocking work explicitly.
- **Verbosity controls both sinks.** `LoggingState::emit` returns before terminal
  and file output for debug/trace events when verbose mode is off. That is a
  deliberate Istari policy, not a bug, but it would violate Regressify's promise
  to archive the full standard timeline independently of display verbosity.
- **Redaction is a backstop, not a public-export policy.**
  `common::redact::redact_text` replaces registered values, authorization-shaped
  text, and URL passwords. It does not classify arbitrary structured fields or
  universally remove signed-query values and page-derived private data. Reuse
  the centralized-sanitization idea, not that coverage as a publication guarantee.

### Adopt With Adaptation

- Stamp the first operation record and archive metadata with application version,
  source commit, and explicit dirty/unknown build identity. Istari's
  `log_build_identity` makes a saved log traceable to the code that produced it.
- Separate terminal presentation from persistent output. Istari's logging config
  distinguishes terminal output and file destinations; Regressify must preserve
  capture archives when console output is quiet or the desktop owns execution.
- Initialize diagnostics early enough to explain startup/configuration failures.
  Istari's bootstrap-to-project handoff is a useful lifecycle model, but a failed
  handoff must retain its source and must never claim a successful copy.
- Adopt explicit writer ownership and shutdown. Istari's `LoggingGuard` owns
  background writer guards, and generation checks keep an old guard from clearing
  a newer sink. Regressify still needs operation-scoped ownership across its
  long-lived desktop process and isolated workers.
- Reject silent queue loss. Istari's `open_log_writer` explicitly sets
  `NonBlockingBuilder::lossy(false)`, unlike the appender's default. Treat this as
  a starting point, not proof of durable writes or safe UI-thread backpressure.
- Give success, failure, and cancellation explicit terminal records. Istari's
  `main::report_outcome` uses consistent status/exit fields, and `report_interrupt`
  explains an operator stop. Regressify must additionally separate command exit,
  visual/capture outcome, and log completeness; absence of a terminal record is
  interrupted/unknown, never an inferred pass.
- Reuse the test patterns: temporary log roots, injected writers, deterministic
  time, redaction fixtures, and scoped subscriber capture. Adapt the latter for
  Regressify's multi-threaded runtime rather than holding a thread-local default
  guard across a task that can move between threads.
- Keep direct terminal writes as narrow presentation exceptions: machine-readable
  stdout, interactive prompts, and emergency logging-init diagnostics. Istari's
  stream mirroring also demonstrates the need to avoid duplicate records, but
  Regressify does not need a token-stream subsystem for capture stage events.

### Deliberate Differences

- Istari's human-readable file is an operational diagnostic stream. Regressify's
  capture archive needs a versioned structured schema consumed directly by both
  report clients; do not parse rendered text to reconstruct capture history.
- Istari identifies a run through a date/time, subcommand, and PID filename.
  Regressify needs explicit unique operation/execution identities inside its
  records and manifests for merged workers, concurrent captures, and old reports.
- Istari defaults operational logs to 15-day age-based retention. Capture logs
  must follow their referenced captures/reports instead. By confirmed user
  decision, separate local operational logs also have no automatic age-based
  pruning. Never import Istari's startup retention cleanup unchanged.
- Keep shared identity/time in data, not only presentation. Use UTC timestamps
  and monotonic durations/sequence numbers in archives; clients may render local
  time. Preserve multi-line messages as one structured event instead of splitting
  them into several records or dropping blank lines as the text formatter does.
- Register any known secrets before diagnostics can contain them, and sanitize
  application records before fan-out to CLI, worker transport, UI, and archives.
  A file-layer scrubber cannot be the only boundary for independently forwarded
  UI or child-process output. Never publish arbitrary debug-formatted objects.

### Review Validation

From the Istari checkout, the following passed on Windows: 14 tests, no failures.
Fixtures use temporary directories and synthetic secrets.

```sh
cargo test --locked --offline --bin istari -- common::logging::tests common::redact::tests --test-threads=1
```

This validates the existing normal handoff/flush, formatting, registered-secret
redaction, and retention behavior. It does not prove failed-copy recovery,
background disk-error reporting, queue saturation, or multi-threaded span
attribution; Unix-only symlink tests were not exercised on Windows. Those are
explicit regression cases for Regressify, not guarantees borrowed from Istari.

## Confirmed Product Decisions

Confirmed with the user on 2026-10-01:

- **Logging failure is warning-only.** If required capture diagnostics cannot be
  fully written, preserve capture results, continue the operation, and surface
  an explicit incomplete/unavailable-log warning. Diagnostic failure alone does
  not change the CLI exit status or turn a passed capture into a failure. Actual
  capture, report, or other required artifact failures retain existing semantics.
- **Full capture logs cover Regressify's execution timeline.** Include its stages,
  actions, waits, retries, timings, safe navigation details, errors, and cleanup.
  Arbitrary browser console output and network request/response collection are
  outside this revamp; do not add an opt-in collector as an implicit extra task.
- **No automatic operational-log pruning.** Keep separate local startup and
  deployment logs until explicitly removed. Capture logs remain tied to their
  captures/reports. Report regeneration may still prune stale generated copies
  under the existing inventory rules, without deleting retained source history.

## Logging Mechanism

Use `tracing` for application diagnostics and spans, with `tracing-subscriber`
providing independently filtered outputs. Retain typed progress events for
lifecycle and result transitions; never derive pass/fail or progress counts by
parsing human-readable log messages.

| Option                        | Assessment                                                                                                               |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `println!` / `eprintln!`      | Presentation only; poor attribution under concurrency and no reusable structured fields.                                 |
| `log` + a logger              | Useful ecosystem facade, but async span context and multiple structured consumers require more custom plumbing.          |
| `tracing` + subscriber layers | Recommended: structured events, nested spans, async context propagation, and separate CLI, archive, and desktop outputs. |
| Another structured logger     | Possible, but no identified advantage over `tracing` for the existing Tokio/CDP worker architecture.                     |

Recommended dependencies to evaluate during implementation:

- `tracing` and `tracing-subscriber`, available to the shared engine rather than
  gated on `desktop` or `gui`.
- A `log`-to-`tracing` bridge only if required for useful dependency diagnostics.
  Avoid duplicate bridges and recursive forwarding.
- `tracing-appender` only if its shutdown and queue behavior satisfy the archive
  contract. Do not accept a lossy queue as the authoritative full-log store.
- No OpenTelemetry exporter or remote collection in the initial scope.

Initialize subscriber policy at CLI/desktop/worker entry points, not from library
functions or each scenario. Preserve test isolation and avoid repeated global
subscriber initialization. Use a typed, allowlisted diagnostic representation at
the archive boundary; arbitrary dependency fields must not become public logs.

Prefer `#[instrument(skip_all)]` with explicit safe fields: the default attribute
records arguments via `Debug`, which can expose configuration and credentials.
Do not mechanically replace direct prints with unreviewed structured dumps.

Subscriber callbacks cannot return an I/O error to the emitting operation. Track
writer health independently, surface warnings through operation finalization,
and avoid logging recursively into a failed sink. A drop guard alone does not
prove that every required record reached disk.

Official references:
[tracing](https://docs.rs/tracing/),
[tracing-subscriber](https://docs.rs/tracing-subscriber/), and
[tracing-appender](https://docs.rs/tracing-appender/).

## Shared Architecture

### Producers and Consumers

The shared capture/deployment engine emits structured diagnostics once. An
adapter preserves the existing typed progress protocol while consumers migrate.
Do not add a second implementation of capture instrumentation for the desktop.

Separate three concerns:

1. **Lifecycle events:** queued, started, captured, compared, cancelled, deployed,
   and finished. These remain authoritative for status and counts.
1. **Diagnostic records:** stages, durations, attempts, decisions, warnings, and
   sanitized error chains, attached to explicit operation/execution identities.
1. **Presentation:** CLI formatting, bounded desktop live activity, and lazy-loaded
   archived timelines in desktop and snapshot reports.

Use `info` for milestones/outcomes, `warn` for recoverable problems and retries,
`error` for terminal stage failures, and `debug` for detailed safe decisions and
timings. Keep noisy `trace` diagnostics opt-in. Required standard events, including
their `debug` details, are always archived regardless of display-level filtering.
Do not compile them out of release builds.

The durable diagnostic archive is independent of UI retention and console
verbosity. A quiet CLI still records the complete standard capture timeline.
Per-layer filters must not accidentally disable persistence through a global
console filter. Optional third-party debug/trace noise is local-only by default.

### Identity and Record Contract

Define a versioned diagnostic record independently of the progress transport and
snapshot schema versions. Suggested fields:

- Operation ID, command/operation kind, and capture run ID where applicable.
- Stable suite ID, scenario ID, viewport identity/dimensions, and execution ID.
- Capture role (`reference` or `test`), attempt number, and capture IDs when known.
- Sequence number, UTC timestamp, monotonic elapsed time, severity, and stage.
- Stable event name, safe human-readable message, and typed diagnostic fields.
- Parent stage/span identity, duration, outcome, and sanitized error causes.
- Application version/source commit and build provenance in operation metadata;
  link each archive to it without repeating the entire header in every event.

Do not use display names, filenames, URLs, or a process-local numeric job ID alone
as durable identity. One scenario/viewport execution can produce several images;
those images should reference one shared timeline with optional capture-specific
markers. A capture must retain its original execution identity across later runs.

Use async span instrumentation across awaits. Explicitly propagate context into
spawned tasks, blocking work, and parallel workers; do not hold an entered span
guard across an await. Global browser startup and deployment records belong to
their operation, not whichever scenario happens to be active.

Define ordering within an execution using sequence numbers, not wall-clock sort.
Give retries distinct attempt identities and keep their history. No frontend
heuristics should infer scenario ownership from interleaved text.

### Full-Log Persistence

"Full" means every standard application diagnostic event for the execution from
queue/start through teardown and terminal outcome, for both successes and
failures. It does not mean secrets, arbitrary payloads, every readiness poll, or
all third-party trace output.

- Persist incrementally to an operation-owned journal under the validated active
  output root. Finalize an index/manifest atomically; do not wait until failure to
  decide whether to save a scenario's history.
- Use streaming or segmented storage with bounded memory. Live UI buffers may
  evict records; the durable archive must not silently do so. Paginate the archive
  rather than raising in-memory caps indefinitely.
- Mark archive state explicitly: complete, partial/interrupted, unavailable for
  legacy captures, or intentionally excluded local-only diagnostics. Preserve
  successfully written records after cancellation, crashes, and failed cleanup.
- Drain and flush writers before declaring completion or releasing worker/run
  ownership. Reader, writer, and child-process lifetimes must not introduce a
  cancellation deadlock.
- Finalize each capture execution after teardown and before automatic comparison
  or snapshot generation consumes its logs. Flush explicitly before CLI
  `process::exit`; release builds use `panic = "abort"`, so crash recovery must not
  assume destructors ran or promise recovery of unflushed records.
- Treat diagnostic archive creation, write, quota, and flush failures as visible
  warnings, per the confirmed decision. Preserve captures and the original
  command/capture outcomes; mark affected logs partial or unavailable. This does
  not downgrade image/manifest/report artifact failures to warnings.
- Maintain writer health and warning delivery outside the failing writer. Notify
  the CLI/desktop through a surviving channel, and carry log availability into
  report metadata where writable. Missing expected logs must also be detectable
  on later load. Never recurse into the failed sink or fabricate a durable warning
  when the output device itself is unavailable.
- Bounded queue exhaustion or a stalled writer must not hang the UI or retain
  unbounded memory. Stop/degrade that diagnostic sink with an explicit warning
  and missing-range metadata when available; retain successfully written records.
  A degraded archive is not a complete log, even when its capture passed.
- Track byte/record counts and any known missing ranges. Overlong diagnostic data
  needs a defined safe representation; never silently truncate standard history
  and label it complete. Abrupt termination may leave the missing count unknown.
- Keep logs while their captures/reports remain referenced. Cleanup must respect
  historical runs, reference provenance, selected output roots, and active writes.

Choose exact journal layout, chunk size, disk budget, and retention configuration
in the storage task, without introducing automatic pruning. Test these decisions
with large synthetic executions before
freezing a public schema. Never store the only copy in temporary worker transport
or bounded saved Activity.

Keep bootstrap/session diagnostics separate from capture archives. Prefer an
operation-bound writer over a single replaceable process-wide destination; the
desktop can outlive many runs. Any bootstrap handoff must preserve operation ID,
sequence, and the original file until the destination is confirmed written. A
failed handoff falls back to the retained source with a visible warning. This is
diagnostic continuity, not permission to write arbitrary workspace directories.

## Capture Diagnostics

Instrument shared reference/test capture paths, including successful executions.
Every meaningful stage should record start, duration/outcome, and enough safe
context to distinguish timeout, website failure, and infrastructure failure.

| Stage                 | Required diagnostic context                                                                                                                   |
| --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Queue and setup       | IDs, viewport, effective non-secret timeout/retry settings, queue wait, browser/page setup, and worker/runtime version.                       |
| Navigation            | Sanitized destination identity, redirect/navigation milestones, observed response status when available, waits, and timeout cause.            |
| State and hooks       | Restore/persist and before/ready hook stage, duration, outcome, and safe error category; never state or script contents.                      |
| Actions and frames    | Action index/type, safe target identity, frame/tab transitions, actionability condition being awaited, retries, and the last unmet condition. |
| Screenshot settling   | Lazy-loading work, font/image readiness, animation/caret handling, viewport/selector bounds, and settling decisions.                          |
| Capture and artifacts | Capture ID, dimensions, encode/write duration, safe artifact identity, manifest completion, and write/cleanup failures.                       |
| Finish                | Capture outcome, teardown, terminal error chain, cancellation cause, and diagnostic archive completeness.                                     |

Record wait/retry transitions and a final summary instead of logging every poll.
Preserve causality when cleanup also fails: do not overwrite the original error.
Link comparison outcomes to the capture execution, but keep visual mismatch
distinct from capture failure and infrastructure failure.

## Deployment Diagnostics

Use the same diagnostic model for all existing deployment entry points. Keep the
operation timeline in CLI/worker output and desktop Activity, with durable history
and sanitized export. Deployment operational logs need not be embedded in the
public report being uploaded; that report cannot contain its own eventual upload
completion without a second publication cycle.

| Stage                      | Required diagnostic context                                                                                                                  |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Preflight                  | Provider, safe target identity, selected suites/run, validation outcome, and credential presence only.                                       |
| Snapshot and staging       | Generation/staging duration, selected dependency counts, bytes, version checks, and failing operation/artifact.                              |
| Packaging and optimization | Applicable provider budget, before/after sizes, file counts, compression decision, and package/hash duration.                                |
| API and uploads            | Operation name, safe endpoint category, HTTP status, safe provider error code/request ID, batch/file counts, bytes, and attempt duration.    |
| Retry and polling          | Attempt number, retry reason/backoff, provider state transitions, elapsed/deadline time, and periodic no-progress heartbeat.                 |
| Completion and cleanup     | Confirmed publication state, validated public URL or successful private publication, local metadata write, cleanup, and cancellation status. |

Keep provider-specific detail explicit: Azure ZIP/upload/status stages, Netlify
manifest/missing-file/upload retries, and Cloudflare session/bucket/upload/final
publication stages. Do not report success merely because upload bytes completed.
Cancellation must distinguish local termination from confirmed remote state.

Aggregate high-volume progress; retain every failure, retry, state transition, and
final summary. Never log credentials, authorization headers, signed URLs, upload
tokens/JWTs, raw response bodies, or whole request/config objects.

## Desktop and CLI Experience

- CLI output is a subscriber/presentation concern. Preserve existing stdout
  contracts; formatted diagnostics can use stderr without engine `eprintln!`
  calls. Include severity, stage, and enough context to separate parallel jobs.
- Offer concise/default and verbose diagnostic views. Console verbosity must not
  decide whether passed scenarios get complete archived logs. Define exact flags
  and environment-variable precedence before adding public options.
- Desktop Activity receives structured live records with bounded, nonblocking
  polling. Keep draining legacy child stdout/stderr even when buffers are full;
  label unattributed fallback output instead of guessing scenario ownership.
- Evolve the existing scoped log screen and copy action to load archived records;
  preserve its scenario/job navigation and explicit omitted-live-history notices.
- Desktop Results offers a Capture Log view for passed and failed items, grouped
  by execution/viewport/attempt. Reference and test executions are independently
  selectable where both exist, with shared scenario history deduplicated.
- Support severity/stage filtering, search, timestamps/durations, follow-tail for
  active runs, and copy/download of sanitized logs. Filtering changes the view,
  not the stored timeline. Large logs load progressively without blocking the UI.
- Reopening the workspace or selecting an older report must recover logs from
  capture artifacts even without saved Activity. Show explicit legacy, partial,
  missing, and loading-error states; never invent a successful historical log.
- Show log-health warnings independently of pass/fail badges. A passed capture
  with an incomplete log stays passed, but its log view and run summary must make
  the missing diagnostics visible. Disable unavailable exports rather than
  presenting a shortened download as complete.

## Snapshot Frontend and Publication

- Extend synchronized Rust/TypeScript report contracts with optional execution-log
  references and explicit availability/completeness metadata. Keep old reports
  readable; verify whether an additive version-2 extension is sufficient before
  choosing a schema bump. Do not overload the existing `error` string.
- Publish only sanitized standard capture timelines as explicit report assets,
  including passed captures. Prefer separately fetched, bounded JSON chunks under
  the existing versioned data namespace rather than inflating every suite JSON.
- Preserve capture-to-log provenance through compare, snapshot regeneration, and
  reference promotion. A successful approval does not create a new browser run;
  never substitute the latest Activity log for the selected capture's source.
- Show a Capture Log view from scenario/capture review in every pass/fail filter.
  Match desktop concepts for attempts, severity, stage, search, download, and
  partial/legacy states. Share timelines across images from one execution.
- Load logs on demand. Render messages as text, never HTML/Markdown or executable
  data. Keep desktop/mobile layout, keyboard navigation, focus, and scrolling
  usable with long lines and large histories.
- Add log dependencies to snapshot inventory, content-version hashing, deployment
  staging, exact package manifests, cleanup, and suite rename/delete handling.
  Preserve source archives when refreshing published assets.
- Include only logs referenced by selected captures/suites, with safe paths and
  symlink checks. Never upload raw journals, unrelated runs, dependency debug
  output, browser state, deployment credentials, or arbitrary workspace files.
- Budget log bytes against provider limits before upload. Do not silently exclude
  passed logs or clip histories to fit; surface an actionable packaging failure.
  This is a provider/package constraint, not a diagnostic-writer failure. For
  already-partial archives, publish the readable records with explicit incomplete
  metadata and warnings; never pretend the absent records were uploaded.

## Privacy and Safety

Redaction and data minimization happen before persistence, transport, or display,
not only in the UI. Use allowlisted structured fields and sanitize error chains.
Unstructured errors and child output need explicit treatment before any export.

Exclude typed input, cookies, storage, authentication data, script bodies, page
content, and request/response bodies. URLs must omit credentials and sensitive
query/fragment values; paths/selectors can also contain private values, so use
safe identities or fingerprints where necessary. Verbose mode is not permission
to record secrets.

Treat log messages as untrusted data in terminal and UI renderers, including
control characters and markup. Keep any opt-in third-party/browser diagnostics
separate and local-only unless a dedicated, reviewed export policy is introduced.
Published standard logs must never depend on best-effort regex redaction alone.

## Delivery Phases

| Phase                         | Scope                                                                                                                                                     | Exit Criterion                                                                                                                     |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| 1. Contract and foundations   | Audit direct prints and existing event producers; define safe fields, identities, levels, archive states, and subscriber ownership; add tracing adapters. | CLI and worker emit equivalent attributed records without duplicate output or changed result semantics.                            |
| 2. Durable capture history    | Incremental journal, finalization/recovery, capture-manifest linkage, reference/test provenance, and migration behavior.                                  | Passed/failed concurrent captures reload complete logs after live-buffer eviction; interrupted/write-failed archives are explicit. |
| 3. Capture instrumentation    | Stage/action/wait/artifact diagnostics in shared capture paths.                                                                                           | Synthetic success and targeted failure cases identify the failing stage and cause without sensitive payloads.                      |
| 4. Deployment instrumentation | Shared/provider-specific stages, HTTP diagnostics, retries, heartbeat, completion, and cleanup.                                                           | Mocked providers expose actionable failure timelines and never leak injected test secrets.                                         |
| 5. Desktop and CLI views      | Live filtering, archive-backed Results/Activity, history reload, sanitized export, and verbosity controls.                                                | Both entry points work independently; desktop remains responsive and cancellation drains/flushing complete.                        |
| 6. Frontend and packaging     | Report schema, lazy log assets/viewer, versioning, selected-only packaging, and legacy support.                                                           | Generated and deployed-fixture reports expose complete passed/failed capture logs on desktop and mobile.                           |
| 7. Hardening and docs         | Concurrency, crash recovery, limits, compatibility, performance, and user/developer documentation.                                                        | Affected gates pass; documentation explains completeness, privacy, retention, and diagnostic workflow.                             |

Dependencies: phase 1 precedes storage and instrumentation; phase 2 precedes
archive-backed client/report work. Deployment instrumentation can proceed after
phase 1 without waiting for the frontend. Define task-level ownership and estimates
later; these phases are not a pre-expanded implementation task list.

## Verification Strategy

Extend existing test modules and fixtures. Start with focused regressions, then
run the affected feature gates; default CLI tests alone do not cover the GUI.

- Interleave passed/failed scenarios across suites, viewports, attempts, and
  multiple captures. Assert identity, ordering, complete history, and no duplicate
  records through CLI, worker transport, archive, and both report consumers.
- Exceed the current live byte/entry limits; verify archive-backed reads and
  downloads still return every standard event. Exercise Unicode, oversized
  records, slow readers, bounded queues, and disk-budget exhaustion.
- Cover setup/action/navigation/capture/teardown failures, zero-image failures,
  cancel-before-start, cancellation mid-write, abrupt worker exit, partial final
  records, flush failures, and artifact permission/sharing errors.
- Inject diagnostic-only create/write/flush failures and assert warning delivery,
  partial/unavailable metadata, preserved capture results, and unchanged exit
  status. Separately prove that image/report write failures remain process errors.
- Inject bootstrap-copy/open failures: preserve the original records and identity,
  never announce successful handoff, and ensure retries cannot duplicate events.
  Test slow/full writer queues and cancellation without UI or task deadlocks.
- Test span-only identities across multi-threaded tasks and blocking work, with
  concurrent operations and repeated desktop runs. Debug-level standard events
  must reach archives even when terminal/UI detail is quiet or disabled.
- Verify standalone CLI runs persist logs without desktop initialization; reload
  desktop history without rerunning; ensure skipped/not-started/legacy work is
  distinguishable from successful capture with an empty log.
- Cover compare/snapshot regeneration, approval provenance, custom output roots,
  suite mutation, older captures, missing/corrupt log assets, and legacy schemas.
- Mock provider timeouts, rate limits, transient/permanent errors, malformed
  responses, partial uploads, failed polling, private publication, cancellation,
  and metadata-write failures. No live credentials or real deployments required.
- Inject synthetic secrets into input, URLs, headers, hooks, errors, and child
  output; inspect all sinks and exact published archives for leaks. Test path
  traversal, symlink escapes, HTML injection, and terminal control sequences.
- Browser-test passed/failed logs, lazy loading, filtering/search, downloads,
  refresh/deep links, incomplete states, long content, and desktop/mobile layouts.
- Verify logs survive age changes and repeated startup without automatic pruning;
  capture/report deletion or explicit cleanup must respect reference provenance.

Affected checks to turn into task-specific gates:

```sh
cargo test --lib progress::tests
cargo test --lib test::test_suites::tests
cargo test --lib test::test_scenario::tests
cargo test --lib test::capture::tests
cargo test --features desktop --lib desktop::runner::tests
cargo test --features desktop --lib desktop::worker::tests
cargo test --features gui --test desktop_worker
cargo test --lib snapshot::
cargo test --features desktop --lib snapshot::
cargo test --features desktop --lib desktop::deployment::
cargo fmt --all -- --check
```

Select narrower capture and GUI test filters once the touched modules are known.
In `frontend`, run `bun run lint`, `bun run test`, `bun run build`, and
`bun run test:e2e`; check both development fixtures and Rust-generated reports.
GUI/worker/browser gates require the prerequisites in the relevant task skills.
Build embedded assets from frontend source, never by editing generated files.

## Decisions to Finalize Before Task Breakdown

1. Standard capture event inventory and safe field allowlist: what is guaranteed
  within the confirmed Regressify-only execution timeline?
1. Durable layout, chunk sizes, resource budgets, and a bounded writer/health
  protocol implementing warning-only failures without automatic retention cleanup.
1. Capture-manifest/progress/report schema migration and provenance for promoted
   references, interrupted jobs, and failures before an image exists.
1. CLI verbosity/filter option names, environment precedence, and desktop export
   behavior, without coupling either to mandatory archive collection.
1. Whether report-log publication needs additional workspace policy beyond the
   sanitized default. Any exclusion must be explicit in the UI and must not
   silently weaken the passed/failed full-log requirement.

Implementation should update [command docs](../commands.md),
[desktop docs](../desktop.md), [frontend guidance](../../frontend/README.md), and
the affected task skills when contracts change. Update known issues when concrete
gaps are confirmed or resolved; this proposal itself does not change those
contracts.
