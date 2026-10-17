---
name: desktop-worker-lifecycle
description: 'Use when changing desktop RunRequest arguments, bundled worker protocol, process groups, cancellation, child cleanup, stdin payloads, or log drainage.'
---

# Desktop Worker Lifecycle

## Start Here

[Runner](../../../src/desktop/runner.rs), [worker protocol](../../../src/desktop/worker.rs), [process groups](../../../src/helpers/command.rs), and [worker integration tests](../../../tests/desktop_worker.rs).

## Procedure and Invariants

1. Construct arguments separately and validate them with the real Clap parser before process launch. Keep command-specific option applicability consistent.
1. Preserve omitted versus explicit report-text options for Snapshot/Deploy, including empty strings. Follow [snapshot precedence](../snapshot-command/SKILL.md); never turn desktop defaults into CLI overrides of workspace settings.
1. Use the bundled executable's worker mode and structured stdin payload, not an unrelated CLI on PATH or a shell string. Bound and validate protocol input.
1. Keep engine work off the UI thread. Track process, readers, event file, scratch data, and activity persistence until their actual work has completed.
1. Workers create managed deployment staging after preflight; normal requests omit legacy `scratchDirectory`. Sweep abandoned containers before deployment and remove owned payloads best effort on completion. Provider temporaries remain beneath the same payload. Follow [run retention](../run-retention/SKILL.md).
1. Cancellation must terminate and reap descendants. Check cleanup after launch, payload write, event decoding, child failure, and UI/window closure.
1. Drain stdout/stderr even when retained logs are full. Surface omitted chunks. Parent exit is not pipe EOF: keep checking cancellation while waiting for stdin/stdout/stderr threads after parent exit, and terminate the process group if descendants retain inherited handles past `PIPE_DRAIN_TIMEOUT`.
1. Exit zero means execution completed, not that visual tests passed. Restore saved activity without restarting work and validate workspace identity/version.
1. Worker startup and deployment journals are operation-owned. Saved `diagnosticOperationId` identifies deployment cancellation recovery after the process group is reaped; never scan for the latest journal or infer remote failure. See [logging docs](../../../docs/commands.md#diagnostic-storage-limits).
1. Internal report approval carries `reportCapture` through structured options and `--report-capture` through Clap. Require one suite/scenario and an explicit run; clear the capture restriction when constructing a subsequent Compare request.

## Checks

```sh
cargo test --features desktop --lib desktop::runner::tests
cargo test --features desktop --lib desktop::worker::tests
cargo test --features gui --test desktop_worker
```

The integration target requires the GUI-enabled worker binary; inspect its ignored browser cases before running them. Cover malformed payloads, failed writes, chatty children, descendant pipes, cancel-before-start, cancel-during-run, and close cleanup.

## Maintain

Update [desktop docs](../../../docs/desktop.md) and this skill. Explain process-tree ownership, protocol bounds, and threading decisions in concise code comments.
