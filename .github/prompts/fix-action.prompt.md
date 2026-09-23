---
agent: agent
description: 'Fix or implement a browser action with realistic interaction, a regression test, and maintained docs.'
---

# Fix Action Implementation

When working on `src/test/test_action.rs`, always apply the adversarial code review approach:

1. Read [AGENTS.md](../../AGENTS.md) and the current implementation.
1. Use the [review skill](../skills/adversarial-review/SKILL.md) to identify a checkable failure.
1. Follow the [action skill](../skills/implement-action/SKILL.md) and reuse its validation helpers.
1. **Never use `.ok()` to swallow errors** — action failures fail the scenario
1. **Never use JS `.click()` for the click action** — use CDP mouse events
1. **Never use `panic!` or `.unwrap()` on fallible ops** — use `?` to propagate errors

## Error Model

- Action error → scenario fails (remaining actions in that scenario are skipped)
- Scenario failure MUST NOT stop other scenarios — each is an independent test
- All scenarios always execute so the user gets a full pass/fail report

## Pre-Action Validation Order (MANDATORY for interactive actions)

1. Wait for the element using the configured deadlines and bounded polling
1. Wait for element to be visible (non-zero bbox, not hidden)
1. Wait for element to be stable (bbox not changing)
1. Scroll element into viewport
1. Check element is not obscured (elementFromPoint)
1. Check element is enabled (form controls)

## Completion

Add a regression test that fails before the fix, run the focused checks, and
maintain action docs/examples and the relevant skill. Explain non-obvious event
or platform decisions with concise comments. Consult the
[CDP skill](../skills/chromiumoxide-patterns/SKILL.md) for browser ownership changes.
