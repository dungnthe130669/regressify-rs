---
title: Create an action
description: Learn how to create test actions for Regressify CLI scenarios including click, input, select, wait, scroll, and more.
---

## Actions

Each scenario can have zero, one or multiple actions. Each action must be one of the following types.

### check

Check a checkbox or radio element.

```yaml
actions:
  - check: checkbox#tos-accept
```

### click

Click an element.

> **Important:** If the click leads to a navigation to other URL, ensure that the very next action is a [wait](#wait) for that URL to fully load.

```yaml
actions:
  - click: button.submit
```

### focus

Focus an element.

```yaml
actions:
  - focus: input[name="email"]
```

### goto

Navigate to a specific URL.

```yaml
actions:
  - goto: https://www.domain.com
```

### hide

Hide an element — set `visibility: hidden !important`.

```yaml
actions:
  - hide: '#cookie-modal'
```

### hover

Hover an element.

```yaml
actions:
  - hover: '#my-chart'
```

### input

Type text in an element, or upload files to a file input.

```yaml
actions:
  # Type text (clears existing value first)
  - input: 'input#username'
    value: My Name

  # Append to existing value
  - input: 'input#email'
    value: my-name@domain.com
    append: true

  # Upload a single file (path relative to visual_tests/)
  - input: 'input#file'
    file: file1.txt
    useFileChooser: true

  # Upload multiple files
  - input: 'input#files'
    file:
      - file1.txt
      - other/file2.pdf
```

### persist

Save the current browser state (cookies + localStorage) to disk. This allows reusing the state in other test suites via the `state` property.

```yaml
actions:
  - input: input#username
    value: aaaa
  - input: input#password
    value: pa$$w0rd
  - click: button#login-btn
  - persist: user-aaaa
```

To reuse the saved state in another test suite:

```yaml
state: user-aaaa
```

### press

Press a key or key combination on an element.

> **Important:** If the press leads to navigation, ensure the very next action is a [wait](#wait) for that URL.

```yaml
actions:
  - press: '#text-area'
    key: Ctrl+A
```

### remove

Remove an element from view — set `display: none !important`.

```yaml
actions:
  - remove: div.random-content
```

### save

Take an intermediate screenshot during the action sequence.

```yaml
actions:
  - save: before-submit
  - click: button.submit
  - save: after-submit
```

### scroll

Scroll to an element.

```yaml
actions:
  - scroll: .report
```

### select

Select an option inside a `<select>` element.

```yaml
actions:
  # Select by value
  - select: select[name="method"]
    value: email

  # Select by label
  - select: select[name="method"]
    label: Send via email

  # Select multiple values
  - select: select[name="method"]
    value:
      - sms
      - skype
```

### uncheck

Uncheck a checkbox or radio element.

```yaml
actions:
  - uncheck: checkbox[name="subscribe-email"]
```

### wait

Wait for a delay (milliseconds), an element to appear, or a navigation to complete.

```yaml
actions:
  # Delay 2 seconds
  - wait: 2000

  # Wait for an element to appear
  - wait: .popup

  # Wait for navigation to /quote, then delay 2 seconds
  - wait: 2000
    url: '**/quote'
```

> **Important:** Whenever an action causes navigation to another URL, the very next action should be a `wait` with the target `url` to ensure the page loads before continuing.

### frame (shared property)

All actions support an optional `frame` property to target elements inside iframes. For nested iframes, provide an array.

```yaml
actions:
  # Click inside a single iframe
  - click: '#submit'
    frame: .child

  # Click inside a nested iframe
  - click: '#submit'
    frame:
      - .child
      - '#grandchild'
```
