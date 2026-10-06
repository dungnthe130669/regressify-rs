<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 031 — Documentation updates

- Slug: `documentation-updates`
- Implements plan section(s): `Phase 7 — Hardening and Docs#0`, `Definition of Done#0`
- Depends on: cli-verbosity-flags-precedence, frontend-report-schema-log-fields, desktop-log-screen-archive-loading, desktop-results-capture-log-view, desktop-log-health-warnings, frontend-log-viewer-component

## Goal

Update docs/commands.md, docs/desktop.md, frontend/README.md, and affected task skills to describe the new flags, views, and completeness/privacy guarantees this plan introduces, after the desktop and frontend features it documents actually exist.

## Why this exists

Task 7.3's own Task Breakdown acceptance criterion requires each doc updated and cross-linked so a reviewer can find the new flags/behavior from existing docs entry points; Definition of Done item 8 makes this a hard completion gate.

## Settled investigation

- docs/commands.md, docs/desktop.md, and frontend/README.md all already exist as tracked files and are already the documented 'Maintain' targets for several directly-relevant existing skills (.github/skills/desktop-progress-events/SKILL.md, desktop-results-review/SKILL.md, desktop-worker-lifecycle/SKILL.md, snapshot-command/SKILL.md, deployment-packaging/SKILL.md).
- No existing skill is specifically named 'logging' or 'diagnostics' in .github/skills/ (43 skill files currently tracked); depending on how much new, logging-specific procedure this plan's implementation accumulates, this task may need to create a new skill file following the existing <topic>/SKILL.md format with a matching YAML name and specific description.
- AGENTS.md itself documents the project's Entry Points table (lines 63-76) that currently has no row for logging/diagnostics; Required Maintenance explicitly requires updating AGENTS.md itself when paths, commands, contracts, or workflows become stale.
- This task's Implementation steps require documenting Task 5.2's (desktop-log-screen-archive-loading) evolved Activity log screen, Task 5.3's (desktop-results-capture-log-view) Results Capture Log view, Task 5.5's (desktop-log-health-warnings) log-health warning behavior, and Task 6.2's (frontend-log-viewer-component) frontend Capture Log viewer by name — all four are now formal Dependencies, not only prose references, since this task is the Definition-of-Done-gating final task and must not be schedulable before any of the four features it documents exist.

## Inputs

- Task 5.1's (cli-verbosity-flags-precedence) final CLI flag names/precedence
- Task 6.1's (frontend-report-schema-log-fields) final report schema field names
- Task 5.2's (desktop-log-screen-archive-loading) evolved Activity log screen behavior
- Task 5.3's (desktop-results-capture-log-view) Results Capture Log view
- Task 5.5's (desktop-log-health-warnings) log-health warning behavior
- Task 6.2's (frontend-log-viewer-component) frontend Capture Log viewer, its deep-link URL shape, and lazy-loading behavior
- Every other task's evidence_to_capture/acceptance criteria as the source material for what changed

## Outputs

- Updated docs/commands.md (new CLI flags)
- Updated docs/desktop.md (new Results/Activity views, log-health warnings)
- Updated frontend/README.md (new Capture Log viewer)
- Updated/new relevant .github/skills/*/SKILL.md files
- A possible new Entry Points row in AGENTS.md

## Implementation steps

- Document Task 5.1's finalized --log-level/--log-verbose flags and REGRESSIFY_LOG_LEVEL environment variable, and their precedence order, in docs/commands.md alongside existing flag documentation.
- Document the new desktop Results Capture Log view (Task 5.3 / desktop-results-capture-log-view), evolved Activity log screen (Task 5.2 / desktop-log-screen-archive-loading), and log-health warning behavior (Task 5.5 / desktop-log-health-warnings) in docs/desktop.md, cross-linking to the desktop-progress-events and desktop-results-review skills.
- Document the new frontend Capture Log viewer, its deep-link URL shape, and lazy-loading behavior (Task 6.2 / frontend-log-viewer-component) in frontend/README.md.
- Update .github/skills/desktop-progress-events/SKILL.md, desktop-results-review/SKILL.md, desktop-worker-lifecycle/SKILL.md, snapshot-command/SKILL.md, and deployment-packaging/SKILL.md with the new invariants this plan's implementation actually introduced, per each skill's own existing 'Maintain' instruction.
- Decide whether a new dedicated skill (e.g. logging-diagnostics/SKILL.md) is warranted given the accumulated procedure, and create it following the existing skill format if so.
- Cross-check every new flag/behavior mentioned in these docs against what actually shipped in Tasks 1.1-7.2, correcting any doc text that drifted from the final implementation during development.

## Done when

- docs/commands.md contains a new section documenting --log-level, --log-verbose, and REGRESSIFY_LOG_LEVEL with their exact precedence order as shipped by Task 5.1.
- docs/desktop.md documents the new Results Capture Log view (Task 5.3), evolved Activity log screen (Task 5.2), and log-health warning behavior (Task 5.5), cross-linked to the desktop-progress-events and desktop-results-review skills.
- frontend/README.md documents the new Capture Log viewer (Task 6.2), its deep-link URL shape, and lazy-loading behavior.

## Evidence to capture

- A list of every doc/skill file touched (docs/commands.md, docs/desktop.md, frontend/README.md, and the 5 named skill files) and a one-line summary of what changed in each

## Do not change

- Do not document a flag name, schema field, or behavior different from what the implementing tasks actually shipped — cross-check against Tasks 5.1/5.2/5.3/5.5/6.1/6.2's final evidence_to_capture before writing.
- Do not duplicate skill-level procedural detail into AGENTS.md itself; AGENTS.md's own instructions require it to stay a rulebook that references skills, not restate them.

## Reopen triggers

- A later bugfix or follow-up task changes a flag name/behavior this documentation already describes, per each skill's own 'Maintain' instruction to keep docs aligned with actual parsing/defaults/behavior.

## Context

This is the plan's final task, gating Definition of Done item 8; it is also where AGENTS.md's own 'Required Maintenance' rule (update this file, relevant skills, and shared memory in the same change) gets applied to the entire logging revamp at once, after every other task has already landed its code-level changes — which is why it now formally depends on the four desktop/frontend feature tasks (5.2, 5.3, 5.5, 6.2) it describes, not only the two schema/flag tasks (5.1, 6.1) it previously depended on.

## Options & tradeoffs

Writing documentation incrementally alongside each implementing task would keep docs fresher throughout, but the plan explicitly scopes all documentation to this final Phase 7 task, likely so the docs reflect the final, settled flag names/behavior rather than an early draft that changes mid-implementation; following the plan's explicit phase placement (chosen) risks needing to re-verify prior tasks' final state, which this task's implementation_steps account for.

## Decision

Consolidate all documentation updates into this single Phase 7 task, cross-checking final flag names/behavior against each implementing task's own evidence before writing, rather than documenting incrementally per task; depend formally on all four desktop/frontend feature tasks this task documents, not only the two schema/flag tasks, so a scheduler cannot run this task before those features exist.

## Acceptance criteria

- docs/commands.md documents the exact final --log-level/--log-verbose/REGRESSIFY_LOG_LEVEL names and precedence order Task 5.1 shipped, not the Interfaces section's earlier proposed sketch if it changed during implementation.
- A reviewer starting from AGENTS.md's Entry Points table or any of the 5 named existing skills (desktop-progress-events, desktop-results-review, desktop-worker-lifecycle, snapshot-command, deployment-packaging) can reach the new logging/diagnostics documentation within one link hop.

## File touchpoints

- docs/commands.md
- docs/desktop.md
- frontend/README.md
- .github/skills/desktop-progress-events/SKILL.md
- .github/skills/desktop-results-review/SKILL.md
- .github/skills/snapshot-command/SKILL.md
- .github/skills/deployment-packaging/SKILL.md
- AGENTS.md
- .github/skills/logging-diagnostics/SKILL.md (proposed)

## Tests

- Manual verification: open docs/commands.md and confirm the documented --log-level flag name/values/precedence matches Task 5.1's shipped src/cli/root.rs field exactly.
- Manual verification: open frontend/README.md and confirm the documented Capture Log viewer's deep-link URL shape matches Task 6.2's (frontend-log-viewer-component) recorded evidence_to_capture exactly (e.g. its exact query-parameter name).

## Dependencies

- cli-verbosity-flags-precedence
- frontend-report-schema-log-fields
- desktop-log-screen-archive-loading
- desktop-results-capture-log-view
- desktop-log-health-warnings
- frontend-log-viewer-component

## Risks

- Documentation written against an assumed final flag name/schema shape before Tasks 5.1/6.1 actually close could drift from what shipped; mitigate by making this task's last implementation step an explicit cross-check against those tasks' recorded evidence_to_capture rather than against the plan's original Interfaces sketch.
- With six dependencies instead of two, this task becomes schedulable only very late; mitigate by treating that as correct (it is the plan's final task and Definition of Done item 8 depends on the features it documents actually existing), not as a scheduling defect to work around.
