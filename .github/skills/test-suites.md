# Skill: Create or Modify Test Suites

## When to Use

Use this skill when:

- Writing new visual test suite YAML files
- Adding scenarios to existing test suites
- Understanding the test suite configuration structure
- Debugging test suite parsing issues

## Test Suite File Location

All test suite files live in `visual_tests/` with the naming convention:

```text
visual_tests/{name}.tests.yaml
```

The Rust CLI also accepts `.tests.yml` and `.tests.json`. If multiple formats exist for the same suite name, discovery uses `.tests.yaml`, then `.tests.yml`, then `.tests.json`. Two files at the same format priority with the same suite name are rejected as ambiguous, including files in different subdirectories.

## YAML Structure

```yaml
# Suite-level configuration (all optional except scenarios)
postInteractionWait: 5000          # ms to wait after all actions (default: 1000)
misMatchThreshold: 0.1             # Pixel diff tolerance (0-100, default: 0.1)
cssOverridePath: common/_override.css  # CSS to inject into all scenarios
hideSelectors:                     # Selectors to hide in ALL scenarios
  - '.cookie-banner'
  - '#chat-widget'
removeSelectors:                   # Selectors to remove in ALL scenarios
  - '.ads'
state:                             # Load persisted state before scenarios
  - signed-in                      # Matches {REGRESSIFY_DIR}/states/signed-in.json
urlReplacements:                   # URL variable substitution
  - ref: '{{BASE_URL}}'
    test: 'https://staging.example.com'
  - ref: '^https://prod\\.example\\.com'
    test: 'https://staging.example.com'
    regex: true
    flags: i

scenarios:
  - url: https://example.com/page
    label: Descriptive name         # Used for display and ID generation
    id: custom-id                   # Optional explicit ID (auto-generated from label if omitted)
    delay: 1000                     # ms to wait before screenshot
    # Capture named CSS targets instead of the full page. Each target creates
    # its own screenshot; `viewport` captures the visible viewport.
    selectors:
      - viewport
      - main .summary
    hideSelectors:                  # Override suite-level hideSelectors
      - '.dynamic-content'
    removeSelectors:                # Override suite-level removeSelectors
      - '.timestamp'
    noScrollTop: true               # Skip auto-scroll to load lazy images
    restore: signed-in              # Restore persisted state for this scenario
    requiredLogin: true             # Exposed to jsOnReady scripts as context
    cookiePath: common/_cookies.yaml
    cssOverridePath: common/_override.css
    needs: other-scenario-id        # Run another scenario's actions first
    actions:                        # Sequential browser automation steps
      - goto: https://example.com
      - click: '#login-button'
      - input: '#email'
        value: test@example.com
      - press: '#email'
        key: Tab
      - input: '#password'
        value: secret123
      - click: '#submit'
      - wait: 2000
        url: '**/dashboard'
      - save: after-login           # Intermediate screenshot
```

## Actions Quick Reference

```yaml
# Navigation
- goto: https://url.com

# Interactions (require pre-action validation)
- click: '#selector'
- hover: '#selector'
- focus: '#selector'
- input: '#selector'
  value: "text to type"
- input: '#selector'
  value: "appended text"
  append: true
- input: '#file-input'
  file: path/to/file.pdf        # Relative to visual_tests/
- input: '#file-input'
  file:
    - file1.pdf
    - file2.pdf
  useFileChooser: true
- press: '#selector'
  key: Enter                    # Key name (Enter, Tab, Escape, ArrowDown, etc.)
- check: '#checkbox'
- uncheck: '#checkbox'
- select: '#dropdown'
  value: option-value
- select: '#dropdown'
  label: "Option Text"
- scroll: '#element'

# Waiting
- wait: 2000                    # Wait N milliseconds
- wait: '#loading-spinner'      # Wait for element to appear
- wait: 5000
  url: '**/expected-path'       # Wait for navigation to URL

# DOM Manipulation (no interactability checks)
- hide: '#dynamic-element'      # visibility: hidden
- remove: '#ad-banner'          # display: none

# Utilities
- save: screenshot-name         # Intermediate screenshot
- persist: session-name         # Save cookies + localStorage

# iframe targeting (works with any action)
- click: '#button-in-frame'
  frame: 'iframe#my-frame'
- click: '#nested-button'
  frame:
    - 'iframe.outer'
    - 'iframe.inner'
```

## Scenario Dependencies (`needs`)

Scenarios can reuse actions from other scenarios:

```yaml
scenarios:
  - url: https://example.com/login
    id: login
    label: Login flow
    actions:
      - input: '#email'
        value: user@test.com
      - input: '#password'
        value: pass123
      - click: '#submit'
      - wait: 2000
        url: '**/dashboard'

  - url: https://example.com/login
    label: Dashboard after login
    needs: login               # Executes login's actions first
    actions:
      - click: '#settings'     # Additional actions after login
```

Multiple dependencies:

```yaml
needs:
  - setup-data
  - login
```

## URL Matching in `wait`

The `url` property in wait supports:

- Exact match: `https://example.com/page`
- `*` within one path segment: `https://example.com/*/page`
- `**` across any path depth: `https://example.com/**/page`
- Wildcards in query values: `https://example.com/page?tab=*`

## Best Practices

1. **Always add a label** — Auto-generated IDs from URLs are cryptic
1. **Use `wait` after navigation-triggering clicks** — The next screenshot needs the page loaded
1. **Use `hide` for non-deterministic content** — Timestamps, counters, ads
1. **Use `save` for intermediate states** — Debug failures more easily
1. **Keep scenarios independent** — Use `needs` instead of ordering dependencies
1. **Set `misMatchThreshold`** — Anti-aliasing differences cause false failures at 0
1. **Use `noScrollTop: true`** for single-viewport captures — Avoids unnecessary scrolling

## Config Validation Notes

- Only `browser: chromium` is supported.
- Unknown configuration and action properties are errors; use documented camelCase keys.
- Unknown viewport names, duplicate viewport labels, empty labels, and zero dimensions are config errors.
- An explicit `viewportsPath` must name an existing regular file inside the workspace; the default viewport file may be absent.
- Invalid regex URL replacements are config errors.
- Website failures should be captured in reports and should not stop sibling scenarios.
- `debug: true` (suite-level) and the global `--debug` flag launch a headed (visible) Chromium window. This is automatically forced off in CI environments (Azure Pipelines, GitHub Actions, or `CI=true`) via `common::is_ci()`, because CI agents have no display — a headed launch there fails with `Missing X server or $DISPLAY`. See `LaunchOptions`/`launch_browser` in `src/test/browsers.rs`.
