---
name: maintain-agent-guidance
description: 'Use when creating, updating, or reviewing AGENTS.md, Copilot instructions, GitHub skills, prompt links, discovery metadata, or stale AI workflow knowledge.'
---

# Maintain Agent Guidance

## Start Here

[AGENTS.md](../../../AGENTS.md) owns shared rules. The
[Copilot entry point](../../copilot-instructions.md) only routes to it.
Keep procedures in `.github/skills/<topic>/SKILL.md`, not flat Markdown files.

## Procedure

1. Verify the affected source, nearby tests, docs, and CI before recording a rule.
   Do not turn a hypothesis, old review, or memory note into a current guarantee.
1. Keep AGENTS.md under 90 lines and each skill under 120 lines. Use one concrete
   task per skill with entry points, a procedure, invariants, checks, and doc owners.
1. Include YAML `name` matching the directory and a quoted, keyword-rich
   `description` explaining when to load the skill. Keep automatic invocation
   enabled. Reference bundled resources with `./` and repository files relatively.
1. Keep mandatory comments, comprehensive behavior tests, documentation, and
   same-change guidance maintenance in the shared guide. Explain non-obvious
   reasons in code; do not demand narration of self-explanatory statements.
1. When moving skills, update prompt links and remove obsolete copies. Preserve
   still-valid contracts; do not maintain duplicate rulebooks.
1. Verify each test filter names real tests. List prerequisites and ignored tests;
   a command that succeeds with zero matching tests is not verification.

## Checks

Use `rumdl` for Markdown formatting and linting; scope fixes to changed files.
The validator uses Python 3.11+ and PyYAML 6.0.3, also used by release checks.
Use an existing test environment or an isolated environment, not a global install.

```sh
rumdl check AGENTS.md OUTSTANDING_ISSUES.md .github/copilot-instructions.md .github/skills .github/prompts
python .github/skills/maintain-agent-guidance/scripts/validate.py --self-test
python .github/skills/maintain-agent-guidance/scripts/validate.py
```

Use `rumdl check --fix <changed-paths>` for formatting. Do not rewrite unrelated
docs or disable rules to conceal errors. CI pins rumdl 0.2.63 for reproducibility.

[Validator](./scripts/validate.py) checks metadata, discovery layout, file sizes,
local links, and stale legacy references. It cannot verify the truth of prose or
test coverage; inspect the source and run relevant focused checks as well.
[Guidance CI](../../workflows/agent-guidance-checks.yml) runs all checks on pull
requests and master pushes, including source changes that can break skill links.

## Maintain

Update the validator's self-tests when changing its rules. Keep the Copilot entry
point, prompts, skill links, and CI filters in sync with discovery/layout changes.
