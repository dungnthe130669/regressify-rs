# AGENTS.md — AI Session Guide for Regressify CLI

This file helps AI coding assistants (GitHub Copilot, Claude, etc.) understand the project's AI configuration files and when to use them.

---

## Quick Reference

| File                                          | Type         | Purpose                                                                  |
| --------------------------------------------- | ------------ | ------------------------------------------------------------------------ |
| `.github/copilot-instructions.md`             | Instructions | **Always loaded.** Core project context, design philosophy, known issues |
| `.github/skills/implement-action.md`          | Skill        | How to create/fix browser automation actions                             |
| `.github/skills/adversarial-review.md`        | Skill        | How to review actions for correctness/security                           |
| `.github/skills/chromiumoxide-patterns.md`    | Skill        | CDP/chromiumoxide API patterns and examples                              |
| `.github/skills/test-suites.md`               | Skill        | Test suite YAML structure and conventions                                |
| `.github/skills/ref-command.md`               | Skill        | `ref` command capture/promotion behavior                                 |
| `.github/skills/test-command.md`              | Skill        | `test` command capture/compare orchestration                             |
| `.github/skills/compare-command.md`           | Skill        | `compare` command report generation behavior                             |
| `.github/skills/approve-command.md`           | Skill        | `approve` command promotion behavior                                     |
| `.github/skills/snapshot-command.md`          | Skill        | `snapshot` command HTML summary generation                               |
| `.github/skills/optimize-command.md`          | Skill        | `optimize` command PNG optimization behavior                             |
| `.github/skills/init-command.md`              | Skill        | `init` command starter-file generation behavior                          |
| `.github/skills/install-command.md`           | Skill        | `install` command browser fetch behavior                                 |
| `.github/skills/version-command.md`           | Skill        | `version` command behavior                                               |
| `.github/prompts/create-test-suite.prompt.md` | Prompt       | Agent mode: create/modify test suite YAML files                          |
| `.github/prompts/fix-action.prompt.md`        | Prompt       | Agent mode: fix action implementations                                   |
| `.github/prompts/review-actions.prompt.md`    | Prompt       | Agent mode: adversarial review of action code                            |
| `OUTSTANDING_ISSUES.md`                       | Reference    | Known bugs/gaps from latest code review                                  |
| `docs/create-action.md`                       | Reference    | Complete user-facing action documentation                                |
| `docs/commands.md`                            | Reference    | CLI command behavior, flags, outputs, and exit-code rules                |

---

## File Types Explained

### Instructions (`.github/copilot-instructions.md`)

**Loaded automatically** by VS Code Copilot for every interaction in this workspace.

Contains:

- Project overview and architecture
- Key design philosophy (real-user behavior simulation)
- Pre-action validation rules (mandatory for all interactive actions)
- Adversarial code review checklist
- Known implementation issues
- File-to-purpose mapping

**When relevant:** Always. This is the baseline context for any work in this repo.

---

### Skills (`.github/skills/*.md`)

**Domain knowledge** files. These are reference material that an AI should read when performing specific tasks. They are NOT auto-loaded — the AI must look them up when the task matches.

#### `adversarial-review.md`

- **Use when:** Reviewing action code, auditing security, checking correctness
- **Contains:** Review methodology, action categories, validation order, common bugs to look for

#### `chromiumoxide-patterns.md`

- **Use when:** Writing code that interacts with the browser via CDP
- **Contains:** API patterns for Page, Element, CDP commands, mouse/keyboard dispatch, cookie management, file upload, screenshots

#### `implement-action.md`

- **Use when:** Creating a new action or fixing an existing one
- **Contains:** Full implementation checklist — type definition, enum variant ordering, handler implementation, actionability integration, testing

#### `test-suites.md`

- **Use when:** Writing or debugging YAML test suite files
- **Contains:** YAML schema, field reference, scenario configuration, `needs` dependencies, URL replacements, viewport configuration

#### Command skills

- **Use when:** Changing or reviewing one CLI command end to end
- **Files:** `init-command.md`, `install-command.md`, `version-command.md`, `ref-command.md`, `test-command.md`, `compare-command.md`, `approve-command.md`, `snapshot-command.md`, `optimize-command.md`
- **Contains:** Command-specific entry points, behavior contracts, error semantics, and verification steps

---

### Prompts (`.github/prompts/*.prompt.md`)

**Agent-mode task definitions** with YAML frontmatter. These define focused AI workflows with restricted tool access. Use them via Copilot Chat's `@workspace /prompt-name` or when the AI should operate in a constrained mode.

#### `create-test-suite.prompt.md`

- **Mode:** Agent
- **Tools:** read_file, create_file, replace_string_in_file, list_dir
- **Use when:** User wants to create or modify `visual_tests/*.tests.yaml` files
- **Behavior:** Follows test suite conventions, adds labels, includes waits after navigation, sets thresholds

#### `fix-action.prompt.md`

- **Mode:** Agent
- **ApplyTo:** `src/test/test_action.rs`
- **Tools:** run_in_terminal, read_file, replace_string_in_file, grep_search, semantic_search
- **Use when:** An action is broken, fails the adversarial checklist, or needs new actionability checks
- **Behavior:** Reads implementation, checks against adversarial checklist, ensures all validations are present

#### `review-actions.prompt.md`

- **Mode:** Agent
- **Tools:** read_file, grep_search, semantic_search
- **Use when:** User asks for a code review of action implementations
- **Behavior:** Reviews each action against the adversarial methodology, reports issues with severity/location/fix

---

## Key Architecture Facts for AI Sessions

1. **Browser automation:** `chromiumoxide` (Rust CDP client), NOT Playwright
1. **Browser support:** Chromium only; Firefox/WebKit config values are rejected early
1. **Action types:** Defined in `src/types/actions.rs`, `TestAction` enum is `#[serde(untagged)]` — **variant order matters**
1. **Actionability module:** `src/test/actionability.rs` provides `ensure_actionable`, `ensure_focusable`, `ensure_exists`, `cdp_click`, `cdp_hover`
1. **Config cascade:** scenario → suite → workspace → defaults (see `src/config/mod.rs`)
1. **Error handling:** Actions MUST propagate errors with `?`, never `.ok()`
1. **Scenario isolation:** Each scenario is independent. A failed scenario MUST NOT stop other scenarios. All scenarios always execute so the user gets a full pass/fail report. Action error → scenario fails → error placeholder/report result → other scenarios continue.
1. **Exit codes:** Config/infra/artifact errors are non-zero; website/scenario/visual failures are report-only zero-exit results
1. **Security:** All user-provided selectors go through `escape_js_string` or `escape_css_rule` before injection into JS/CSS strings
1. **Iframes:** Frame traversal via `contentDocument` chains built by `build_frame_document_js`

## Common Tasks and Which Files to Read

| Task                  | Read These Files                                                                                                 |
| --------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Add a new action      | `docs/create-action.md`, `.github/skills/implement-action.md`, `src/types/actions.rs`, `src/test/test_action.rs` |
| Fix a broken action   | `.github/skills/adversarial-review.md`, `src/test/actionability.rs`, `src/test/test_action.rs`                   |
| Work on a CLI command | The matching `.github/skills/*-command.md`, `src/cli/args.rs`, command module source, `docs/commands.md`         |
| Create test YAML      | `.github/skills/test-suites.md`, existing files in `visual_tests/`                                               |
| Modify config system  | `src/config/mod.rs`, `src/types/regressify.rs`, `src/test/test_scenario.rs`                                      |
| Debug browser issues  | `.github/skills/chromiumoxide-patterns.md`, `src/test/browsers.rs`                                               |
| Review for security   | `.github/skills/adversarial-review.md`, `OUTSTANDING_ISSUES.md`                                                  |

---

## Workflows (`.github/workflows/`)

### `tuyen-release.yml`

- **Purpose:** GitHub Actions release workflow
- **Trigger:** Manual or via `tuyen release` command
- **Not AI-relevant** — standard CI/CD, no special instructions needed
