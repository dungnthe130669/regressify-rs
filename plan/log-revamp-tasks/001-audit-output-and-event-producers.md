<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 001 — Audit existing output and event producers

- Slug: `audit-output-and-event-producers`
- Implements plan section(s): `Phase 1 — Contract and Foundations#0`, `Current Evidence#0`

## Goal

Produce a written inventory of every direct eprintln!/println! call site in engine code and every progress.rs Event variant, classified for migration, plus an inventory of existing sanitization/error-construction helpers (starting from src/desktop/deployment/diagnostics.rs) classified for reuse.

## Why this exists

Phase 1's exit criterion and every later phase depend on knowing exactly which call sites become tracing events vs. stay as presentation, and on not duplicating the already-tested diagnostics.rs sanitizer that Tasks 1.3/4.2/4.3 must build on instead of re-implementing.

## Settled investigation

- A repository-wide search for eprintln! found call sites in at least src/bin/desktop.rs, src/compare/actions.rs, src/compare/report_gen.rs, src/desktop/deployment.rs (phase() at lines 810-813, with_heartbeat()/monitor() at 815-826), src/desktop/deployment/cloudflare.rs, src/desktop/deployment/swa.rs, src/desktop/jump_list.rs, src/desktop/runner.rs, src/desktop/ui/mod.rs, src/helpers/command.rs, src/helpers/mode.rs, src/optimizer/image_optimizer.rs, src/snapshot/site.rs, and src/test/test_suites.rs; this search was truncated at 40 matches across 14 files, so the implementer must re-run an untruncated/paginated search.
- The scenario_log! macro (src/progress.rs lines 35-40) is used across src/desktop/runner.rs (test fixture), src/test/browsers.rs, src/test/browsers/install.rs, src/test/page_stability.rs, src/test/test_action.rs, src/test/test_scenario.rs, and src/test/test_suite.rs.
- progress.rs's Event enum (src/progress.rs lines 76-121) currently has exactly 11 variants: Phase, SuiteQueued, Queued, Started, Log, Captured, Comparing, Compared, SuiteFinished, Deployed, Finished.
- src/desktop/deployment/diagnostics.rs already implements sanitize() (line 173) and failure() (line 65), used by swa.rs/netlify.rs/cloudflare.rs, with an existing passing test details_preserve_provider_reason_without_credentials_or_signed_urls (line 216) proving credential/signed-URL redaction.

## Inputs

- Repository source tree at HEAD
- Capture Diagnostics and Deployment Diagnostics required-context tables from the plan

## Outputs

- A written, tracked inventory document enumerating every call site/event variant/helper with its classification column

## Implementation steps

- Run a repository-wide literal search for `eprintln!(` and `println!(` restricted to engine code and record file:line for every hit, re-running until the result set is confirmed complete (not truncated).
- Run a repository-wide literal search for `scenario_log!(` and record file:line for every hit.
- List every progress::Event variant from src/progress.rs and classify it as lifecycle-authoritative (keep) vs. also needing a new diagnostic record.
- Starting from src/desktop/deployment/diagnostics.rs's sanitize()/failure(), search for any other ad hoc redaction/error-formatting helper and classify each as reuse-as-is / reuse-with-extension / superseded.
- For each call site, classify as keep-as-presentation, replace-with-tracing, or needs-a-new-event, cross-checking against Capture Diagnostics' and Deployment Diagnostics' required-context tables.
- Write the inventory to a tracked markdown file and link it from Tasks 1.3, 1.4, 4.2, and 4.3.

## Done when

- The inventory file exists in the repository, tracked by git, and lists every file:line call site found by a fresh repository-wide search for eprintln!(, println!(, and scenario_log!( in src/.
- The inventory file lists all 11 current progress::Event variants with a classification each.
- The inventory file lists sanitize() and failure() from src/desktop/deployment/diagnostics.rs with a reuse-with-extension classification (not superseded), plus any other sanitization helper found.

## Evidence to capture

- The exact search commands used and their raw hit counts, so a reviewer can re-run the same search and get the same count
- The final inventory file's repository path

## Do not change

- Do not modify any of the audited call sites in this task; this is inventory-only, feeding Tasks 1.3/1.4/4.2/4.3.
- Do not re-implement or modify src/desktop/deployment/diagnostics.rs's sanitize()/failure().

## Reopen triggers

- A later task finds an eprintln!/scenario_log! call site or progress.rs event variant missing from the inventory.
- A later task finds a second ad hoc secret-redaction implementation not captured in the helper inventory.

## Context

Regressify's scenario_log! macro always writes to stderr and conditionally also emits a structured progress::Event::Log; src/desktop/deployment.rs writes phases/heartbeats directly with eprintln!. Before any tracing migration can start, every one of these call sites and the existing progress::Event enum must be enumerated and classified, and the already-tested deployment error-sanitization helper must be identified so later tasks reuse it instead of re-implementing redaction.

## Options & tradeoffs

n/a — this is an investigation/documentation task with no competing implementation approaches; the only choice is where to persist the inventory (a tracked markdown file versus only a PR description). A tracked file is chosen so later tasks can cite it as an input.

## Decision

Persist the inventory as a tracked markdown file (proposed path docs/plan/log-revamp-audit.md) rather than only a PR description, so it survives across the many dependent tasks in Phases 1 and 4.

## Acceptance criteria

- The inventory file covers every eprintln!(/println!(/scenario_log!( call site returned by a fresh repository-wide search of src/**/*.rs at the time the task is done.
- The inventory file classifies all 11 existing progress::Event variants and explicitly reuses (not supersedes) src/desktop/deployment/diagnostics.rs's sanitize()/failure().

## File touchpoints

- docs/plan/log-revamp-audit.md (proposed)
- src/progress.rs (reference)
- src/desktop/deployment/diagnostics.rs (reference)
- src/desktop/deployment.rs (reference)
- src/test/test_suites.rs (reference)

## Tests

- Manual verification: re-run the literal search for `eprintln!(` across src/**/*.rs and diff its hit count against the inventory's call-site count.

## Dependencies

*none*

## Risks

- The initial grep in this session was truncated at 40 matches across 14 files for eprintln! alone; the implementer must use an untruncated/paginated search to avoid under-counting call sites, or the inventory will silently miss migration candidates.
