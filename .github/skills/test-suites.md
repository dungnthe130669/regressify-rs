# Skill: Create or Modify Test Suites

## When to Use

Use this skill when:
- Writing new visual test suite YAML files
- Adding scenarios to existing test suites
- Understanding the test suite configuration structure
- Debugging test suite parsing issues

## Test Suite File Location

All test suite files live in `visual_tests/` with the naming convention:
```
visual_tests/{name}.tests.yaml
```

## YAML Structure

```yaml
# Suite-level configuration (all optional except scenarios)
postInteractionWait: 5000          # ms to wait after all actions (default: 0)
misMatchThreshold: 0.1             # Pixel diff tolerance (0-100, default: 0.1)
cssOverridePath: common/_override.css  # CSS to inject into all scenarios
hideSelectors:                     # Selectors to hide in ALL scenarios
  - '.cookie-banner'
  - '#chat-widget'
removeSelectors:                   # Selectors to remove in ALL scenarios
  - '.ads'
state:                             # Load persisted state before scenarios
  - signed-in                      # Matches .regressify/states/signed-in.json
urlReplacements:                   # URL variable substitution
  - ref: '{{BASE_URL}}'
    test: 'https://staging.example.com'

scenarios:
  - url: https://example.com/page
    label: Descriptive name         # Used for display and ID generation
    id: custom-id                   # Optional explicit ID (auto-generated from label if omitted)
    delay: 1000                     # ms to wait before screenshot
    selectors:                      # What to capture (default: full document)
      - viewport                    # Capture viewport size only
    hideSelectors:                  # Override suite-level hideSelectors
      - '.dynamic-content'
    removeSelectors:                # Override suite-level removeSelectors
      - '.timestamp'
    noScrollTop: true               # Skip auto-scroll to load lazy images
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
- Glob suffix: `**/page` (matches any URL ending with `/page`)
- Glob contains: `**/dashboard` + `**` patterns

## Best Practices

1. **Always add a label** — Auto-generated IDs from URLs are cryptic
2. **Use `wait` after navigation-triggering clicks** — The next screenshot needs the page loaded
3. **Use `hide` for non-deterministic content** — Timestamps, counters, ads
4. **Use `save` for intermediate states** — Debug failures more easily
5. **Keep scenarios independent** — Use `needs` instead of ordering dependencies
6. **Set `misMatchThreshold`** — Anti-aliasing differences cause false failures at 0
7. **Use `noScrollTop: true`** for single-viewport captures — Avoids unnecessary scrolling
