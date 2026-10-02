# Logging Migration Inventory

## Follow-up Review: 2026-10-02

The baseline inventory below is historical. This follow-up verified the subscriber,
engine and desktop entry points, command producers, saved archive selection,
snapshot publication, and frontend loading/export boundaries.

Confirmed gaps repaired with regression coverage:

- Failed startup/runtime/journal completions used informational severity. Final
  records now retain severity consistent with their outcome.
- Non-capture commands had no operation owner. Command journals now retain typed
  identity, selected work, results, and warnings without changing capture history.
- Desktop activity only retained deployment operation IDs. Command IDs now survive
  save/reload and select the exact journal, with explicit missing/legacy states.
- Optimizer details and approval decisions existed only in console output. Typed
  records retain every completed file result/skip, including Rayon work, without
  exposing paths, failed-capture marker contents, or arbitrary error strings.
- The desktop GUI had no entry-point subscriber or application journal. Startup,
  shell, and window warnings now use a local, nonblocking application journal.
- Snapshot empty-directory cleanup warnings disappeared after completion. The
  successful command now retains warning severity and the available OS error code.
- The frontend accepted timestamps that could throw during rendering. Rust and
  frontend validation now enforce JavaScript's supported date range; browser tests
  verify the report remains usable with a retryable loading error.

### Remaining Print Calls

- Keep version/result stdout, prompts, and initialization file-operation output as
  presentation. Preserve local error fallbacks and snapshot output-location text.
- Comparison rows and command summaries remain human-readable presentation; their
  outcomes are also retained as typed command records and comparison reports.
- Capture-orchestration prints supplement per-job journals and lifecycle events;
  do not infer duplicate progress transitions from their formatted text.
- Deployment phase/heartbeat and browser setup prints are fallback paths when no
  operation context exists. Their normal execution paths already emit diagnostics.
- Keep subprocess pipe/overflow fixtures and ignored live-test output unchanged.
  Raw child output is not a safe public diagnostic payload.
- The legacy command-formatting print method and Unix permission print helper have
  no active call sites in the current tree. Do not migrate them as if they were
  reachable capture instrumentation; review their payloads before future reuse.
- Ordinary `tracing::info!`/`error!` is not enough: the subscriber deliberately
  accepts only reviewed diagnostic records. Producers must use the typed runtime,
  with explicit context propagation and a journal owner.

This is a risk-focused review, not a claim that every UI status message, browser
console entry, panic, or third-party event is archived. Browser console/network
bodies, typed input, credentials, raw scripts, and arbitrary child output remain
excluded. Abrupt termination can leave partial archives; disk/queue failures must
remain visible rather than being mislabeled as complete.

## Historical Baseline

Verified: 2026-10-01, before logging implementation. This is the baseline for
[the logging tasks](plan/log-revamp-tasks/README.md), not a claim that the listed
messages are safe to publish. Locations refer to the pre-migration source;
subsequent migrations may move or remove them.

## Reproducible Search

Run from the repository root (including test modules in `src`):

```sh
rg --line-number --no-heading --glob '*.rs' '\b(eprintln|println|scenario_log)!\(' src
rg --count-matches --glob '*.rs' '\beprintln!\(' src
rg --count-matches --glob '*.rs' '\bprintln!\(' src
rg --count-matches --glob '*.rs' '\bscenario_log!\(' src
rg --line-number --glob '*.rs' 'redact|sanitize|for_diagnostics|diagnostic_url|safe_message|error_chain|format_error' src
```

The first search has **157 call sites**: **79 `eprintln!`**, **18 `println!`**,
and **60 `scenario_log!`**. Word boundaries keep `println!` from also counting
`eprintln!`. The inventory includes presentation-only tests and feature-gated
code; it is not restricted to default-feature production paths.

## Call Sites

Each comma-separated location is an individual call site. `Presentation` means
keep as presentation, `Tracing` means replace with tracing, and `New event`
means add typed diagnostics while retaining an existing presentation or lifecycle
contract. Test fixture output stays deliberately unstructured where it tests
pipe drainage or fallback attribution. All migrated application messages must
be constructed from safe fields, not forwarded verbatim.

| Source                                                                          | Macro        | Lines                                                                                                       | Classification | Context                                                                                                    |
| ------------------------------------------------------------------------------- | ------------ | ----------------------------------------------------------------------------------------------------------- | -------------- | ---------------------------------------------------------------------------------------------------------- |
| [src/main.rs](../src/main.rs)                                                   | println      | 21                                                                                                          | Presentation   | Command result stdout.                                                                                     |
| [src/main.rs](../src/main.rs)                                                   | eprintln     | 25                                                                                                          | New event      | Command error; preserve exit status and add safe terminal outcome.                                         |
| [src/main.rs](../src/main.rs)                                                   | eprintln     | 30                                                                                                          | Presentation   | Terminal spacing.                                                                                          |
| [src/application.rs](../src/application.rs)                                     | eprintln     | 39                                                                                                          | Tracing        | Managed browser location; replace path with safe identity.                                                 |
| [src/application.rs](../src/application.rs)                                     | println      | 43                                                                                                          | Presentation   | Version stdout contract.                                                                                   |
| [src/vso/command.rs](../src/vso/command.rs)                                     | println      | 122                                                                                                         | Presentation   | External command output; local-only, never archive raw child text.                                         |
| [src/init.rs](../src/init.rs)                                                   | eprintln     | 237, 252, 261                                                                                               | Presentation   | Explicit initialization file operations; not capture timelines.                                            |
| [src/progress.rs](../src/progress.rs)                                           | eprintln     | 48, 54                                                                                                      | New event      | Existing scenario transport wrapper; migrate producers without recursive forwarding.                       |
| [src/helpers/mode.rs](../src/helpers/mode.rs)                                   | eprintln     | 40                                                                                                          | Tracing        | Browser executable permissions; omit local absolute path.                                                  |
| [src/helpers/command.rs](../src/helpers/command.rs)                             | eprintln     | 112                                                                                                         | Tracing        | Child execution diagnostic; omit arguments and payloads.                                                   |
| [src/helpers/command.rs](../src/helpers/command.rs)                             | eprintln     | 185, 218                                                                                                    | Presentation   | Legacy child stderr forwarding; local-only, sanitize before display.                                       |
| [src/optimizer/image_optimizer.rs](../src/optimizer/image_optimizer.rs)         | eprintln     | 27, 31, 38, 42, 46, 82, 99, 115, 133, 156, 157, 158, 162, 163, 167, 172                                     | Tracing        | Optimization start, decisions, failures, byte counts, and summary.                                         |
| [src/test/test_suites.rs](../src/test/test_suites.rs)                           | eprintln     | 269, 315, 325, 382, 395, 1184, 1204, 1212, 1216, 1225, 1232, 1268, 1338, 1345, 1371, 1397, 1416, 1423, 1434 | Tracing        | Queue, scenario failures, comparison, reference promotion, operation summaries.                            |
| [src/test/test_suite.rs](../src/test/test_suite.rs)                             | scenario_log | 33, 39, 164, 168, 175, 214, 219, 226, 234, 245                                                              | Tracing        | Parsing, handler outcomes, browser teardown; retain original failure alongside cleanup failure.            |
| [src/test/test_scenario.rs](../src/test/test_scenario.rs)                       | scenario_log | 104, 259, 306, 309, 339, 362, 473, 481, 643, 655, 674, 692, 706, 758, 787                                   | Tracing        | Setup, readiness, actions, captures, storage, completion.                                                  |
| [src/test/test_action.rs](../src/test/test_action.rs)                           | scenario_log | 274, 314, 327, 334, 346, 385, 412, 460, 519, 534, 542, 556, 596, 609, 680, 722, 762, 770                    | Tracing        | Action type/index and safe target fingerprints; never input, selectors, scripts, state names, or raw URLs. |
| [src/test/page_stability.rs](../src/test/page_stability.rs)                     | scenario_log | 27, 50, 57, 64, 105                                                                                         | Tracing        | Readiness and settling decisions; summarize waits instead of every poll.                                   |
| [src/test/browsers.rs](../src/test/browsers.rs)                                 | scenario_log | 85, 154, 177, 192, 198, 307, 336                                                                            | Tracing        | Browser setup/cache, handlers, launch outcomes.                                                            |
| [src/test/browsers/install.rs](../src/test/browsers/install.rs)                 | scenario_log | 70, 71, 78, 285                                                                                             | Tracing        | Managed browser download/extraction; omit full URLs and paths.                                             |
| [src/compare/actions.rs](../src/compare/actions.rs)                             | eprintln     | 133, 148, 178, 182                                                                                          | Tracing        | Comparison and approval orchestration; preserve source execution identity.                                 |
| [src/compare/report_gen.rs](../src/compare/report_gen.rs)                       | eprintln     | 613, 722, 734, 760, 803, 830, 889, 987, 994, 1001, 1026, 1047, 1050                                         | Tracing        | Image/report results, skips, approval; mismatch is not capture failure.                                    |
| [src/snapshot/site.rs](../src/snapshot/site.rs)                                 | eprintln     | 520, 606                                                                                                    | Tracing        | Snapshot generation and cleanup; report writes remain required artifacts.                                  |
| [src/desktop/deployment.rs](../src/desktop/deployment.rs)                       | eprintln     | 811, 820                                                                                                    | Tracing        | Phase and no-progress heartbeat; preserve typed Phase events.                                              |
| [src/desktop/deployment/swa.rs](../src/desktop/deployment/swa.rs)               | eprintln     | 283                                                                                                         | Tracing        | Azure retry, separate from confirmed publication.                                                          |
| [src/desktop/deployment/swa.rs](../src/desktop/deployment/swa.rs)               | println      | 530                                                                                                         | Presentation   | Ignored live verification test.                                                                            |
| [src/desktop/deployment/netlify.rs](../src/desktop/deployment/netlify.rs)       | println      | 1376, 1413, 1427, 1481, 1489, 1512                                                                          | Presentation   | Ignored live verification tests.                                                                           |
| [src/desktop/deployment/cloudflare.rs](../src/desktop/deployment/cloudflare.rs) | println      | 1748, 1785, 2229, 2321                                                                                      | Presentation   | Ignored live verification tests.                                                                           |
| [src/desktop/deployment/cloudflare.rs](../src/desktop/deployment/cloudflare.rs) | eprintln     | 1777                                                                                                        | Presentation   | Ignored live verification cleanup/error path.                                                              |
| [src/bin/desktop.rs](../src/bin/desktop.rs)                                     | eprintln     | 32, 38, 48, 55                                                                                              | New event      | Bootstrap/worker/activation failures; emergency fallback must survive sink failure.                        |
| [src/desktop/jump_list.rs](../src/desktop/jump_list.rs)                         | eprintln     | 134, 176                                                                                                    | New event      | Native shell errors belong to local operational diagnostics.                                               |
| [src/desktop/ui/mod.rs](../src/desktop/ui/mod.rs)                               | eprintln     | 2873, 2918                                                                                                  | New event      | Native startup failures, never capture ownership inferred from active view.                                |
| [src/desktop/landing.rs](../src/desktop/landing.rs)                             | println      | 591, 787                                                                                                    | Presentation   | Test/fixture output.                                                                                       |
| [src/desktop/runner.rs](../src/desktop/runner.rs)                               | println      | 1968, 1974                                                                                                  | Presentation   | Pipe/overflow test fixtures.                                                                               |
| [src/desktop/runner.rs](../src/desktop/runner.rs)                               | eprintln     | 1969                                                                                                        | Presentation   | Pipe test fixture.                                                                                         |
| [src/desktop/runner.rs](../src/desktop/runner.rs)                               | scenario_log | 1991                                                                                                        | Presentation   | Attribution regression fixture; retain legacy coverage during migration.                                   |

## Progress Protocol

All eleven [Event variants](../src/progress.rs#L76) remain authoritative on
the existing version-1 live transport. Diagnostics must not synthesize result
transitions by parsing messages or emit a second copy of existing transitions.

| Variant         | Classification          | Diagnostic requirement                                                     |
| --------------- | ----------------------- | -------------------------------------------------------------------------- |
| `Phase`         | Keep lifecycle          | Add typed operation stage/duration.                                        |
| `SuiteQueued`   | Keep lifecycle          | Preserve stable suite identity, not display name.                          |
| `Queued`        | Keep lifecycle          | Bind durable execution identity to local job ID.                           |
| `Started`       | Keep lifecycle          | Record queue wait and setup start.                                         |
| `Log`           | Keep live compatibility | Adapter emits bounded presentation, archive stores full safe record.       |
| `Captured`      | Keep lifecycle          | Preserve capture error vs infrastructure distinction and teardown outcome. |
| `Comparing`     | Keep lifecycle          | Link comparison to existing capture provenance.                            |
| `Compared`      | Keep lifecycle          | Preserve mismatch vs capture failure distinction.                          |
| `SuiteFinished` | Keep lifecycle          | Do not infer log completeness from suite completion.                       |
| `Deployed`      | Keep lifecycle          | Confirm publication; nullable URL remains successful private publication.  |
| `Finished`      | Keep lifecycle          | Separate command status, capture result, and archive health.               |

## Sanitization and Error Construction

| Helper                                                                          | Classification       | Constraint                                                                                                                                                                                 |
| ------------------------------------------------------------------------------- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| [deployment sanitize](../src/desktop/deployment/diagnostics.rs#L173)            | Reuse with extension | Extract shared non-desktop implementation; retain provider caller behavior. Registered credentials must be removed before truncation. This regex backstop is not a public field allowlist. |
| [deployment failure](../src/desktop/deployment/diagnostics.rs#L65)              | Reuse with extension | Preserve provider status/hints and bounded structured error extraction; wrap safe errors for diagnostics.                                                                                  |
| [deployment http_error](../src/desktop/deployment/diagnostics.rs#L14)           | Reuse with extension | Bounded response read, header allowlist, provider failure helper; never export response bodies.                                                                                            |
| [deployment collect_details](../src/desktop/deployment/diagnostics.rs#L124)     | Reuse as-is          | Internal depth/count-bounded provider-field selection, before sanitization; not directly publishable.                                                                                      |
| [Cloudflare credential pre-pass](../src/desktop/deployment/cloudflare.rs#L1034) | Reuse with extension | Removes all upload/session credentials before shared sanitizer truncation; retain multi-secret coverage.                                                                                   |
| [Git process redact](../src/desktop/git/process.rs#L264)                        | Reuse as-is          | Separate Git boundary, cuts authorization and remote-shaped words; no unrelated Git refactor.                                                                                              |
| [scenario_diagnostic_url](../src/test/test_suite.rs#L109)                       | Reuse with extension | Existing parser-error password masking preserves username/query/path; unsuitable as a public-log URL policy. Use fingerprinted identity in new events.                                     |
| [bounded_log_message](../src/progress.rs#L745)                                  | Reuse as-is          | Live presentation limit, not sanitization and not archive storage.                                                                                                                         |

Searches also return redaction test names. They are regression evidence, not
additional sanitizers. Generic `anyhow!`, `.context`, and `Debug` error formatting
are untrusted inputs, not sanitization helpers.

## Standard Event and Field Starting Point

The diagnostic schema must allow only reviewed keys and typed values. IDs derived
from user names, paths, URLs, and selectors need opaque fingerprints where they
would otherwise disclose payloads. Use generated UUIDs for operation/execution
identity; a process-local job ID is only an adapter lookup key.

| Stages                       | Required safe fields and transitions                                                                                                 |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Queue/setup                  | Operation/execution IDs, role, attempt, viewport dimensions, effective timeout/retry counts, queue duration, runtime/build identity. |
| Navigation                   | Destination fingerprint, redirect count, response status when observed, timeout category, duration.                                  |
| State/hooks                  | Hook/state action kind, duration, outcome/category; never contents or storage values.                                                |
| Actions/frames               | Action index/type, target fingerprint, frame/tab transition, unmet actionability condition, retries.                                 |
| Settling                     | Font/image/lazy-loading outcomes, bounded geometry, animation/caret decisions, total wait duration.                                  |
| Capture/artifacts            | Capture identity, dimensions, bytes, encoding/write duration, manifest outcome.                                                      |
| Finish                       | Original outcome plus cleanup outcome, cancellation category, completeness independent of result.                                    |
| Deployment preflight/staging | Provider/target fingerprint, credential presence only, selected counts, validated dependency counts/bytes.                           |
| Packaging/upload             | Provider budget, compression decision, before/after bytes, batches, endpoint category, status, safe request ID.                      |
| Retry/poll/completion        | Attempt/backoff, provider state, elapsed/deadline, heartbeat, confirmed publication vs local cancellation.                           |

Missing instrumentation is explicit work, not something a print replacement can
infer: hooks, actionability transitions, capture encoding/writes, per-request
provider status, and durable finalization all need new records. Standard debug
records remain archived under quiet presentation. Browser console, request bodies,
script bodies, typed input, and raw child output are excluded from public logs.
