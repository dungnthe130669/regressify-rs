---
mode: agent
description: Create new test suite YAML files or modify existing ones
tools:
  - read_file
  - create_file
  - replace_string_in_file
  - list_dir
---

# Create/Modify Test Suite

When creating or modifying test suites in `visual_tests/`:

1. Follow the YAML structure documented in `.github/skills/test-suites.md`
2. Always include a `label` for each scenario (auto-generated IDs are cryptic)
3. Add `wait` actions after clicks that trigger navigation
4. Use `hide` for non-deterministic content (timestamps, ads, random data)
5. Use `needs` for scenario dependencies instead of relying on execution order
6. Set appropriate `misMatchThreshold` (default 0.1 is very strict)

## File Naming

```
visual_tests/{descriptive-name}.tests.yaml
```

## Validation

After creating/modifying, ensure:

- All scenarios have unique IDs (auto-generated from label if not set)
- URLs are valid and accessible
- Selectors use valid CSS syntax
- `frame` selectors target actual `<iframe>` elements
- File paths in `input.file` are relative to `visual_tests/`
