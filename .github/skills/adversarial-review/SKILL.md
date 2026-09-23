---
name: adversarial-review
description: 'Use when reviewing Regressify code for correctness, security, data loss, regressions, concurrency, or missing tests across CLI and desktop features.'
---

# Adversarial Review

## Start Here

Read [AGENTS.md](../../../AGENTS.md), the owning module's skill, and
[OUTSTANDING_ISSUES.md](../../../OUTSTANDING_ISSUES.md). Review requests are
read-only unless fixes or supporting documentation changes are authorized.

## Procedure

1. Identify the affected feature, caller, input trust level, and documented contract.
1. State one falsifiable failure hypothesis. Follow the input through parsing,
   validation, execution, persistence, error reporting, and cleanup.
1. Check the actual caller and existing guards before declaring a defect. A grep
   match, stale comment, or theoretical race is not sufficient evidence.
1. Reproduce cheaply with existing tests, temporary workspaces, synthetic secrets,
   local Git repositories, and loopback fixtures. Do not contact real remotes,
   publish reports, install updates, or print browser state/credentials.
1. Distinguish current defects, already tracked gaps, and unverified hypotheses.
   Record confirmed unresolved defects in the issue list when edits are in scope.

## Boundaries to Challenge

- Configuration and action precedence, missing/unknown fields, numeric limits.
- Path traversal, symlinks, Windows device names, glob/pathspec interpretation.
- JS/CSS/HTML sink escaping, unsafe report URLs, oversized responses/artifacts.
- Infrastructure versus website failure exits; failed screenshot promotion.
- Unsaved documents, external edits, stale async completion, confirmation identity.
- Process groups, pipe draining after parent exit, cancellation, locks, retries.
- Credential lifetime/redaction and update verification immediately before launch.

## Evidence and Checks

For each finding give severity, exact source location, reachable trigger, impact,
and the smallest regression test or repair. State what was executed versus inferred.
Do not treat bounded buffers, intended compatibility defaults, or protections in
nearby code as bugs. Do not inflate severity just because input is untrusted.

Use the owning skill's focused test first, then its affected feature gates.
For fixes, demonstrate a regression test fails before and passes after the change;
cover success, failure, boundary, and applicable platform/concurrency cases.

## Maintain

Keep this methodology and the relevant domain skill current when new reusable
failure patterns are established. Keep session-specific findings in the issue
list, not in this skill. Report findings first; state review coverage and untested
paths even when no defects are found.
