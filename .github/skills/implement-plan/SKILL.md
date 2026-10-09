---
name: implement-plan
description: 'Use when implementing an entire reviewed Regressify plan or resuming its task bundle, including output from istari plan-review: reconcile refined requirements, execute tasks in dependency order, verify affected CLI/desktop/frontend behavior, and record progress. Commit units or retire completed plan files only with explicit authorization. Not for creating or reviewing plans, interpreting review checkpoints as approved tasks, or implementing one isolated task.'
argument-hint: 'Path to the reviewed plan, bundle README, or task directory'
---

# Implement a Reviewed Plan

Carry the whole reviewed plan through implementation and verification. The refined plan defines requirements and design intent; task files describe how to satisfy them. Current code and reproducible evidence establish reality. Do not silently redesign the plan when they disagree.

## Establish the Contract

1. Read [AGENTS.md](../../../AGENTS.md), the [shared memory index](../../../.ai/memory/README.md), and only the topic notes and skills relevant to the next unit. Plans under [plan](../../../plan) are potentially outdated history unless selected for this work; verify their assumptions against current source.
1. Resolve the supplied path and inspect `git status --short`, the working diff, and staged changes. Preserve all existing work, including untracked plan files. Do not reset, stash, amend, switch branches, or overwrite unrelated edits.
1. Locate the final reviewed task bundle through explicit links or provenance. Local bundles can live under `plan/<name>-tasks/`; `.istari/plan-review/` also contains review checkpoints and pending tasks, which are not a substitute for the final bundle. Do not rerun `istari plan-review` over in-progress work without approval.
1. Read the bundle README, its linked refined plan (normally `plan.md`), considerations, any human-context or explicitly linked implementation guidance, and every task. Capture goals, non-goals, settled decisions, dependencies, acceptance criteria, required checks, and conditions that reopen a decision. The original proposal is provenance, not a replacement for refined requirements.
1. Reconcile actual task files with the README and dependency table; map the refined requirements to task coverage. Missing required files, uncovered requirements, unresolved review decisions, conflicting criteria, or dependency cycles require clarification, not invented tasks. If only a source proposal or checkpoint is supplied, locate its final bundle or ask which requirements are approved.
1. Establish authorization boundaries. Default to implementation and verification with changes left uncommitted and plan files retained. Executing a plan does not itself authorize commits, new branches, pushes, releases, real remote operations, credential changes, or plan deletion. When requested, record the per-unit commit policy and exact cleanup scope before either operation.
1. Verify claimed completion against code, tests, and relevant existing commits before skipping a task. Review labels such as pending, agreed, or checked boxes are not implementation evidence. Resume from the first incomplete unit and retain previous evidence.
1. Baseline the next unit's cheapest meaningful check before editing, recording existing failures. Select required gates from the owning skills and current CI; a pre-existing failure does not count as a passing gate.

Give a short execution outline: dependency order, next unit, required checks, known blockers or deferrals, and whether commits or cleanup are authorized. Continue without an approval round when the scope is unambiguous and actions are already authorized.

## Order and Progress

- Honor dependencies before filename numbering. Follow an explicit execution order when consistent with those dependencies; otherwise topologically order tasks, using README order and then task numbers to break ties. Ask about genuine conflicts rather than silently dropping a dependency.
- One task is normally one verifiable unit. Split or combine only when the plan defines independently verifiable subunits or an indivisible adjacent group, or the user approves. A compile failure caused by a later constructor update is not permission to implement later tasks silently.
- Finish and verify the current unit before starting the next. If commits are authorized, commit each unit before proceeding; otherwise leave changes in the worktree and record unit boundaries. Do not batch an authorized per-unit workflow into one final commit. Parallel implementation requires explicit user authorization and independent tasks.
- Reuse or add an implementation-status section in the bundle README. Maintain an ordered checklist with stable task IDs, outcomes, exact checks/results, approved decisions, and open deferrals as work happens. Preserve task specifications and generated provenance comments.
- Keep resume evidence in that status section, not private memory or new tracking files. Record commit IDs after creation when relevant; never attempt to embed a commit's own hash in itself. Durable project knowledge belongs in the appropriate shared memory topic.

## Implement One Unit

1. Reread the task and relevant refined requirements. Load the owning skills for the touched actions, configuration, captures, reports, desktop models/UI, deployment, or release surface. Locate the deciding code and neighboring tests; state one falsifiable local hypothesis and the cheapest check that could disprove it.
1. Verify stale paths or APIs locally, then make the smallest coherent edit using existing abstractions, helpers, and fixtures. Escalate changed behavior, scope, or acceptance criteria. Preserve the critical contracts in AGENTS.md, especially successful-only reference promotion, scenario isolation, feature boundaries, lossless desktop writes, and safe filesystem/process boundaries.
1. Immediately run the narrowest meaningful validation after the first substantive edit. Repair local defects and rerun that check before expanding scope. If evidence falsifies the hypothesis, follow one nearby controlling path rather than reopening broad exploration.
1. Add tests covering success, failures, boundaries, and relevant security, concurrency, and platform cases. Bug fixes need a regression that fails before the fix. Do not weaken assertions, relax thresholds, delete failing tests, suppress diagnostics, or substitute mocks for required integration evidence.
1. Maintain user-facing docs/examples and non-obvious comments in the same unit. Update affected skills and shared memory when verified paths, contracts, or workflows change; record confirmed or resolved gaps in [OUTSTANDING_ISSUES.md](../../../OUTSTANDING_ISSUES.md). Follow [guidance maintenance](../maintain-agent-guidance/SKILL.md) for guidance edits.
1. Review the complete unit diff against refined requirements, acceptance criteria, non-goals, and failure paths using [adversarial review](../adversarial-review/SKILL.md). Passing tests alone do not prove completeness. Record progress and run the affected gates on the final candidate before declaring the unit complete.

## Verification Gates

Choose gates by changed surface, not a blanket all-features command. Follow [Rust feature checks](../rust-feature-checks/SKILL.md) for current supported CI commands and prerequisites; default CLI tests do not cover desktop or GUI behavior.

| Surface                                          | Focused check and required follow-through                                                                                                                    |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| CLI, actions, configuration, capture, comparison | `cargo test --lib <test_filter>`; run affected default-feature gates and the owning skill's acceptance checks.                                               |
| Desktop models                                   | `cargo test --features desktop --lib <test_filter>`; run affected desktop gates.                                                                             |
| GPUI and native interactions                     | Use [native UI](../desktop-native-ui/SKILL.md) and `gui-tests` gates; native smoke needs an unlocked Windows desktop and isolated app state.                 |
| Worker processes and cancellation                | Use [worker lifecycle](../desktop-worker-lifecycle/SKILL.md); model tests alone do not cover bundled-worker integration.                                     |
| Snapshot frontend and report assets              | Use [snapshot](../snapshot-command/SKILL.md) and [frontend instructions](../../../frontend/README.md), including required production-fixture browser checks. |
| Skills, instructions, shared memory              | Run scoped `rumdl` lint plus the guidance validator and self-tests from [guidance maintenance](../maintain-agent-guidance/SKILL.md).                         |

- After Rust edits, run `cargo fmt --all -- --check` and the affected feature gates. Follow owning skills for browser, injected script, deployment, installer, and release prerequisites. Confirm test filters match nonzero tests; ignored or skipped tests are not passing coverage.
- Attempt remaining independent gates when one fails so an early failure does not hide results. Report exact commands, outcomes, and prerequisites. Missing tools, unsupported platforms, unavailable services, and failing baseline checks are unverified or blocked, never green.
- Live checks require authorized access and known side effects. Do not silently install browsers or releases, contact production sites, change credentials, publish reports, or operate on real Git remotes. Prefer isolated local fixtures. Never request secrets in chat or record tokens, cookies, or persisted browser state in logs, status, or commits.
- If a required check is unavailable, preserve the work and state the missing prerequisite; do not substitute a weaker check or mark its acceptance criterion complete without an explicit decision.

## Reality Mismatches and Deferrals

When current code, a live response, or a reproduction contradicts a plan assumption or triggers a reopen condition, stop the affected unit before committing or starting dependent work. A bounded local reproduction is fine; silently changing criteria or repeatedly forcing the planned outcome is not.

Ask for a decision with the task ID, expected requirement, observed command and redacted evidence, downstream impact, practical options, and a recommendation. Do not continue with an assumed answer. Record the approved decision in existing bundle guidance/status and update affected criteria and dependencies without erasing the rationale.

An intermediate failure is deliberately deferred only when all of these hold:

- The plan or an explicit user decision already makes the staging intentional and names an existing later owner task. An unexpected regression cannot be retroactively declared intentional by the agent.
- Before advancing or committing, record the originating task, exact path/check/diagnostic, temporary safety rationale, later owner, and closure check in the README status.
- Required checks have been attempted, everything outside the explicit exception passes, and the exception does not invalidate current acceptance evidence. If meaningful verification is blocked, seek an approved adjacent-task grouping or another decision.
- No exception bypasses safety invariants, risks data loss, exposes secrets, weakens tests, or conceals failures. Any stricter green-gate instruction still applies. Reports and authorized commit bodies name the non-green check and its owner.

Close each deferral with fresh evidence in its owner task. Missing owners, overdue deferrals, or unexpected failures are blockers. All deferrals must close and required final gates must pass before declaring completion or cleaning up.

## Authorized Commits

Skip this section unless the user explicitly requested commits; lack of commit authorization does not block implementation.

1. Update progress and verification evidence, then validate the final candidate. After further edits, rerun affected checks. Inspect both working and staged diffs; preserve pre-existing staged content and never include unrelated work.
1. Stage only the current unit's paths or hunks, including tests, docs, and status; never use blanket `git add .` or `git add -A`. If the intended commit cannot be isolated without disturbing unrelated staged work, ask for a decision.
1. Run `git diff --cached --check` and inspect the exact staged diff for scope, secrets, missing files, and generated output. Use the repository's existing commit style; include stable task IDs, rationale, checks/results, and any approved deferral in the body.
1. Commit locally, verify the resulting commit and remaining worktree, and report the unit result. A failed hook or commit is a blocker. Do not push, amend, rewrite history, or bypass hooks without separate authorization.

## Complete and Retire the Plan

1. Reconcile every refined requirement and task criterion with implementation and fresh final verification, including required end-to-end or live checks. Resolve considerations, approved decisions, and the deferral ledger. A completed checklist alone is insufficient.
1. Preserve durable decisions, constraints, usage guidance, and essential evidence in maintained docs, tests, shared memory, and authorized commit bodies before removing their only copy. Do not create a duplicate archive just to retire a plan.
1. Retain the bundle and source plan by default. Cleanup is a separate final unit requiring explicit authorization; it is forbidden for partial implementation, unresolved decisions, open deferrals, or missing required verification.
1. Inventory the exact authorized bundle, including nested, tracked, untracked, ignored, and symlink entries. Identify any separate original source plan through explicit links or provenance and confirm its inclusion. Similar filenames, generated ownership markers, and "safe to delete" comments do not establish authorization or source identity. Ask if identity or ownership is ambiguous.
1. Stop for unexpected user files, shared source plans, active review writers, or content outside the boundary. Preserve independent review records and `.istari/plan-review/` checkpoints/logs unless explicitly included. Never delete neighboring task sets, parent plan directories, or symlink targets outside the inventory.
1. Search inbound references, including Markdown inline/reference links, repository URLs, filenames, directory paths, and anchors. Update actual affected docs, skills, memory, and indexes; do not assume another repository's navigation files exist. Redirect current guidance to maintained docs; preserve historical findings with verified commit-pinned links when their citations must survive.
1. Remove only inventoried, authorized files using scoped edits/removals, never broad wildcards. Recheck references and run affected documentation/guidance checks and required final gates after deletion. Do not hand-edit generated output. If commits are also authorized, commit cleanup separately with retired task IDs and verification evidence.

## Final Report

Summarize completed units, verification and skipped checks, approved deviations, and remaining blockers. Include commit IDs only when created; otherwise say changes are uncommitted. State whether plans were retained or list the exact authorized cleanup and repaired references. When blocked, identify incomplete tasks, the required decision or prerequisite, and that cleanup has not occurred. Never claim required live verification from test doubles alone.
