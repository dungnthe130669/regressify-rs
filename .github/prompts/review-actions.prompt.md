---
agent: agent
description: 'Read-only adversarial review of browser actions, with evidence, severity, and regression-test recommendations.'
---

# Adversarial Code Review

Read [AGENTS.md](../../AGENTS.md), the
[review skill](../skills/adversarial-review/SKILL.md), and the
[action skill](../skills/implement-action/SKILL.md). Challenge the implementation
with concrete failure hypotheses; verify callers and guards before reporting a bug.
Do not edit source or perform destructive/external operations unless authorized.

## Review Protocol

For each action in `src/test/test_action.rs`:

1. **Does it wait for the element to exist?** (polling, not one-shot)
1. **Does it check element visibility?** (bbox > 0, not hidden, opacity > 0)
1. **Does it scroll into view before interaction?**
1. **Does it verify the element is not covered?** (elementFromPoint check)
1. **Does it use the correct interaction method?** (CDP events, not JS shortcuts)
1. **Are errors properly propagated?** (no `.ok()`, no silent failures)
1. **Is selector input properly escaped?** (no injection)
1. **Does the iframe path validate each frame exists?**

## Report Format

For each issue found, report:

- **Action**: Which action (click, hover, etc.)
- **Violation**: Which real-user-behavior rule is broken
- **Current code**: What it does wrong
- **Trigger**: Reachable input/state and exact source location
- **Expected behavior**: What it should do
- **Severity**: Based on impact, reachability, and existing mitigations
- **Regression**: Smallest test that would demonstrate the defect

## Reference

Keep confirmed defects separate from hypotheses and existing issues. State which
checks ran and what remains unverified, including when no findings are confirmed.
