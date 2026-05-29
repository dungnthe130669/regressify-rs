---
mode: agent
description: Review action implementations using adversarial approach to find correctness bugs
tools:
  - read_file
  - grep_search
  - semantic_search
---

# Adversarial Code Review

Review the action implementation code with an adversarial mindset. Assume the code is WRONG until proven otherwise.

## Review Protocol

For each action in `src/test/test_action.rs`:

1. **Does it wait for the element to exist?** (polling, not one-shot)
2. **Does it check element visibility?** (bbox > 0, not hidden, opacity > 0)
3. **Does it scroll into view before interaction?**
4. **Does it verify the element is not covered?** (elementFromPoint check)
5. **Does it use the correct interaction method?** (CDP events, not JS shortcuts)
6. **Are errors properly propagated?** (no `.ok()`, no silent failures)
7. **Is selector input properly escaped?** (no injection)
8. **Does the iframe path validate each frame exists?**

## Report Format

For each issue found, report:

- **Action**: Which action (click, hover, etc.)
- **Violation**: Which real-user-behavior rule is broken
- **Current code**: What it does wrong
- **Expected behavior**: What it should do
- **Severity**: Critical (silently wrong) / High (can fail) / Medium (edge case)

## Reference

See `.github/skills/adversarial-review.md` for the full review methodology.
