---
name: git-process-safety
description: 'Use when changing Git subprocess execution, environment isolation, credential redaction, pipe limits, SSH prompting, timeouts, or process-group cancellation.'
---

# Git Process Safety

## Start Here

[Process wrapper](../../../src/desktop/git/process.rs), [remote operations](../../../src/desktop/git.rs), and [background groups](../../../src/helpers/command.rs).

## Procedure and Invariants

1. Use argument arrays, literal pathspecs, and NUL-delimited stdin where appropriate. Strip repository-redirection and askpass environment overrides before spawn.
1. Keep external-protocol execution disabled and remote SSH noninteractive with host-key verification. Do not weaken authentication checks to make tests pass.
1. Bound retained output while draining pipes fully. Apply deadlines/cancellation to stdin writers and reader joins as well as the parent process wait loop.
1. Descendants can inherit stdout/stderr after the parent exits. Do not declare cleanup complete or drop the group handle while unbounded readers still wait.
1. Sanitize error output before display/persistence. Redact complete authorization values, not just a header name or the word containing a URL; test mixed case, whitespace, multiline diagnostics, and synthetic secret variants.
1. Stream only diagnostic command output, never status/config/diff payloads. Buffer complete CR/LF-delimited lines before redaction so chunk boundaries cannot leak URL credentials. Drop the remainder of a stream after an authorization header or oversized line; keep draining the pipe and report the omission.
1. Preserve Windows no-console process creation and group cleanup on all exits.

## Checks

```sh
cargo test --features desktop --lib desktop::git::process::tests
```

Use synthetic credentials and local helper processes only. Cover environment redirection, large output, startup errors, cancellation, descendants retaining pipes, and secrets split across whitespace. Never inspect real credential output.

## Maintain

Update [desktop docs](../../../docs/desktop.md) if Git behavior changes and keep this skill aligned. Explain process ownership and redaction threat assumptions.
