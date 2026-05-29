---
mode: agent
description: Fix or implement browser automation actions with proper real-user behavior simulation
applyTo: 'src/test/test_action.rs'
tools:
  - run_in_terminal
  - read_file
  - replace_string_in_file
  - grep_search
  - semantic_search
---

# Fix Action Implementation

When working on `src/test/test_action.rs`, always apply the adversarial code review approach:

1. **Read the current implementation** of the action being modified
2. **Check against the adversarial review checklist** in `.github/skills/adversarial-review.md`
3. **Verify all pre-action validations** are implemented per `.github/skills/implement-action.md`
4. **Never use `.ok()` to swallow errors** — action failures are test failures
5. **Never use JS `.click()` for the click action** — use CDP mouse events

## Pre-Action Validation Order (MANDATORY for interactive actions)

1. Wait for element to exist (30s timeout, 100ms poll)
2. Wait for element to be visible (non-zero bbox, not hidden)
3. Wait for element to be stable (bbox not changing)
4. Scroll element into viewport
5. Check element is not obscured (elementFromPoint)
6. Check element is enabled (form controls)

## Reference

- Implementation guide: `.github/skills/implement-action.md`
- Adversarial review: `.github/skills/adversarial-review.md`
- CDP patterns: `.github/skills/chromiumoxide-patterns.md`
- Node.js reference: `~/git/regressify-ts/.engine_scripts/playwright/actions.js`
