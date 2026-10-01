<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 005 — Install the subscriber at CLI and worker entry points only

- Slug: `install-subscriber-entry-points`
- Implements plan section(s): `Phase 1 — Contract and Foundations#0`, `End-to-End Data Flow#0`
- Depends on: implement-progress-adapter

## Goal

Wire a single tracing-subscriber installation call into src/main.rs (CLI entry) and src/desktop/worker.rs's run_stdio() (desktop worker entry), and nowhere else.

## Why this exists

Logging Mechanism: Options Considered requires subscriber policy to be 'initialize[d]... at CLI/desktop/worker entry points, not from library functions or each scenario'; this is also Phase 1's literal exit criterion.

## Settled investigation

- src/main.rs (37 lines total) currently calls application::execute(cli).await directly with no subscriber/sink installation before it, and terminates via std::process::exit(exit_code) (line 36), which skips drop-based writer cleanup; the release profile also sets panic="abort" (Cargo.toml lines 96-98).
- src/desktop/worker.rs::run_stdio() (lines 34-120) already conditionally installs the existing progress sink via crate::progress::install(path)? (line 37) when request.events_path is Some, then calls crate::application::execute(cli) inside a tokio multi-thread runtime, then unconditionally emits Event::Finished and calls crate::progress::finish()? (lines 115-119).
- crate::application::execute() (src/application.rs line 18) is the single shared function both entry points call; it is documented as 'Called by the CLI or an isolated desktop worker, never by the GUI thread' — it is a library function and must NOT itself install the subscriber.
- src/desktop/runner.rs lines 1966-1972 already define a REGRESSIFY_PROGRESS_FIXTURE-driven fixture binary mode that prints the literal lines `fixture stdout` (println!) and `fixture stderr` (eprintln!) and installs the progress sink — this existing fixture is the concrete harness this task's repeated-invocation check should reuse.

## Inputs

- Task 1.4's adapter
- The existing src/main.rs and src/desktop/worker.rs::run_stdio control flow (read-only)

## Outputs

- A single install function called exactly once from src/main.rs before application::execute and exactly once from src/desktop/worker.rs::run_stdio before its own call to application::execute
- An explicit flush call before src/main.rs's process::exit

## Implementation steps

- Define one public install function (e.g. diagnostics::install_subscriber(...)) that configures tracing-subscriber layers: a console-formatting layer gated by display verbosity and the Task 1.4 adapter layer that feeds progress::emit.
- Call this function once near the top of src/main.rs's main(), before application::execute(cli).await, and flush before the existing exit(exit_code) call (line 36) so no buffered record is lost to the process::exit/panic="abort" combination.
- Call the same function once near the top of src/desktop/worker.rs::run_stdio(), before its existing crate::progress::install(path)? call, reusing (not duplicating) that existing progress-sink install.
- Guard repeated install calls with a OnceLock/try-init pattern so a second call in the same process is a documented no-op, not a panic.
- Run the REGRESSIFY_PROGRESS_FIXTURE fixture-binary mode twice in one test process and compare stderr line counts to confirm no duplication.

## Done when

- Both src/main.rs and src/desktop/worker.rs::run_stdio call the install function exactly once each, confirmed by `git diff --stat src/main.rs src/desktop/worker.rs` showing exactly one added call-site line per file.
- Invoking the REGRESSIFY_PROGRESS_FIXTURE fixture binary mode (src/desktop/runner.rs lines 1970-1972, which already emits the literal lines `fixture stdout` and `fixture stderr`) twice in the same process produces the same stderr line count both times, with no panic on either invocation.

## Evidence to capture

- The stderr line counts from two consecutive runs of the REGRESSIFY_PROGRESS_FIXTURE fixture binary mode, showing the literal line `fixture stderr` appears exactly once per run (not twice) in both invocations
- `git diff src/application.rs` output showing zero changes to that file, confirming the install call was not added there

## Do not change

- Do not call the install function from src/application.rs::execute or any other library function — both entry points must each own their own single call.
- Do not remove or alter the existing crate::progress::install(path)? call in run_stdio (line 37); the new subscriber installs alongside it, not instead of it.

## Reopen triggers

- A test harness that constructs Cli/invokes application::execute directly (bypassing main.rs) is found to need tracing output too, which would argue for a different installation seam.
- Task 5.1 finalizes CLI flags that change what configuration this install function must read at call time.

## Context

src/main.rs and src/desktop/worker.rs::run_stdio are the only two real process entry points in this codebase; application::execute is shared library code called by both and documented as never being called by the GUI thread directly. Installing anywhere else would either double-install under a shared test harness or miss one of the two entry points.

## Options & tradeoffs

Installing inside application::execute would be a single call site but violates the plan's explicit 'not library functions' rule and risks double-initialization if any test calls execute more than once per process; installing at each of the two real entry points is more boilerplate but matches the plan's rule and Istari's reviewed generation-check pattern for safe repeated calls.

## Decision

Install at src/main.rs and src/desktop/worker.rs::run_stdio only, each via one call to a shared install function, leaving application::execute untouched.

## Acceptance criteria

- Both src/main.rs and src/desktop/worker.rs::run_stdio call the install function exactly once each, verified by `git diff --stat src/main.rs src/desktop/worker.rs` showing exactly one new call-site line added in each file.
- Running the existing REGRESSIFY_PROGRESS_FIXTURE fixture-binary mode (src/desktop/runner.rs lines 1970-1972) twice in the same test process produces an identical stderr line count both times with zero panics.

## File touchpoints

- src/main.rs
- src/desktop/worker.rs
- src/diagnostics/install.rs (proposed)
- src/application.rs (reference)

## Tests

- cargo test --features desktop --lib desktop::worker::tests
- Manual run: set REGRESSIFY_PROGRESS_FIXTURE and invoke the existing fixture binary mode (src/desktop/runner.rs lines 1966-1972) twice in one process, and diff the two runs' `fixture stderr` occurrence counts (expected: exactly 1 each, matching).

## Dependencies

- implement-progress-adapter

## Risks

- tracing's global default subscriber can only be set once per process; if any existing test binary invokes application::execute more than once in-process, a naive global set_global_default would panic on the second call — mitigate with a OnceLock/try-init guard (mirroring src/progress.rs's SINK: OnceLock pattern at line 22) so a repeated call is a documented no-op, not a crash.
