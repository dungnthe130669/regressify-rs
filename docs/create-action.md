---
title: Create an action
description: Reference for creating Optimizely CMS test actions such as click, input, select, wait, scroll, upload files, and frame handling.
guid: ba348ca1-eb67-4c0f-8461-8f36979aea18
date: 2023-07-03T17:00:00+00:00
updated: 2026-04-17T17:00:00+00:00
draft: false
hidden: false
weight: 40000
sort_by: weight
template: docs/page.html

extra:
  lead: This reference explains how actions shape each scenario, when to use them, and which details tend to cause avoidable failures, from navigation waits and file uploads to frame handling, persistence, and element visibility.
  toc: true
  top: false
---

## Actions

Each scenario can have zero, one or multiple actions. Each action must be one of the following types.

{% spoiler() %}

check

:::::

Check a checkbox or radio element.

Example:

```yml
actions:
  # Check the checkbox with `tos-accept` ID
  - check: checkbox#tos-accept
```

{% end %}

{% spoiler() %}

click

:::::

Click an element.

> Important:
>
> If the click leads to a navigation to other URL, please ensure that the very next action is [wait](#spoiler--wait) for that URL to fully load before performing any other steps.

Example:

```yml
actions:
  # Click the button with .submit class
  - click: button.submit
```

{% end %}

{% spoiler() %}

focus

:::::

Focus an element.

Example:

```yml
actions:
  # Focus an input element named `email`
  - focus: input[name="email"]
```

{% end %}

{% spoiler() %}

goto

:::::

Type: `string`.

Go to specific URL.

Example:

```yml
actions:
  - goto: https://www.domain.com
```

{% end %}

{% spoiler() %}

hide

:::::

Hide an element - set `visibility: hidden !important`.

Example:

```yml
actions:
  # Hide the element with `cookie-modal` ID
  - hide: '#cookie-modal'
```

{% end %}

{% spoiler() %}

hover

:::::

Hover an element.

Example:

```yml
actions:
  # Hover mouse over an element contains `my-chart` ID
  - hover: '#my-chart'
```

{% end %}

{% spoiler() %}

input

:::::

Type text in an element.

Example:

```yml
actions:
  # Type `My Name` in an input element with `username` ID
  - input: 'input#username'
    value: My Name
    # `append` is an optional property, with `fase` as the default value
    append: false

  # Type `my-name@domain.com` in an input element with `email` ID
  - input: 'input#email'
    value: my-name@domain.com
    # `append` is an optional property, with `fase` as the default value
    append: true
  
  # Load `visual_tests/file1.txt` into the input with `file` ID
  - input: 'input#file'
    file: file1.txt
    # `useFileChooser` is an optional property, with `fase` as the default value
    useFileChooser: true
  
  # Load `visual_tests/file1.txt` and `visual_tests/other/file2.pdf`
  # into the input with `files` ID
  # Note that the input must allow upload multiple files
  - input: 'input#files'
    file:
      - file1.txt
      - other/file2.pdf
    # `useFileChooser` is an optional property, with `fase` as the default value
    useFileChooser: false
```

{% end %}

{% spoiler() %}

persist

:::::

Type: `string`.

Persists (saves) the current browser state, including cookies and local storage, to disk.

Example:

```yml
actions:
  - input: input#username
    value: aaaa
  - input: input#password
    value: pa$$w0rd
  - click: button#login-btn
  - persit: user-aaaa
```

You can later reuse this saved state to test the site as if you are already logged in by [specifying the state name][test-suite-state] in your test suite:

```yaml wrap
state: user-aaaa
```

{% end %}

{% spoiler() %}

press

:::::

Press a key (or key combination) on an element.

> Important:
>
> If the press leads to a navigation to other URL, please ensure that the very next action is [wait](#spoiler--wait) for that URL to fully load before performing any other steps.

Example:

```yml
actions:
  # Select all text inside a textarea with `message` ID
  - press: 'textarea#message'
    key: Ctrl+A
```

{% end %}

{% spoiler() %}

remove

:::::

Remove an element - set `display: none !important`.

Example:

```yml
actions:
  # Remove a `div` element with class `.random-content`
  - remove: div.random-content
```

{% end %}

{% spoiler() %}

scroll

:::::

Scroll to an element.

Example:

```yml
actions:
  # Scroll to an element with `.report` class
  - scroll: .report
```

{% end %}

{% spoiler() %}

select

:::::

Select an option inside a `select` element.

Example:

Let say you have following HTML:

```html
<select name="method" multiple>
  <option value="email">Send via email</option>
  <option value="sms">Send via SMS</option>
  <option value="skype">Send via Skype</option>
</select>
```

Then you can create select actions like following sample:

```yml
actions:
  # Select the option with `email` value
  - select: select[name="method"]
    value: email

  # Select the option with `Send via email` label
  - select: select[name="method"]
    label: Send via email

  # Select options with `sms` or `skype` value
  - select: select[name="method"]
    value:
      - sms
      - skype
```

> Notes:
>
> The `select` action requires either `value` or `label` to be set, but not both. Omit or include both of them will leading to an error.

{% end %}

{% spoiler() %}

uncheck

:::::

Uncheck a checkbox or radio element.

Example:

```yml
actions:
  # Uncheck the checkbox named `subscrible-email`
  - uncheck: checkbox[name="subscrible-email"]
```

{% end %}

{% spoiler() %}

wait

:::::

Wait for X miliseconds or wait for an element to appears.  
If there is a navigration, add the URL pattern of the target page also.

> Important:
>
> Within a scenario, whenever there is an action that leads to a navigation to other URL, please ensure that the very next action wait for that URL to fully load before performing any other steps.

Example:

```yml
actions:
  # Delay 2 seconds
  - wait: 2000

  # Wait for an element with `popup` class to appears
  - wait: .popup

  # Wait for the navigation to `/quote`, then delay 2 seconds
  # `**` means a group of arbitrary characters
  - wait: 2000
    url: '**/quote'

  # Wait for the navigation to `/quote`, then wait for `#my-form` element to appears
  # `**` means a group of arbitrary characters
  - wait: '#my-form'
    url: '**/quote'
```

{% end %}

## Shared properties

The following properties are applicable to all actions.

{% spoiler() %}

frame

:::::

Type: `string | string[]`.  
Default: empty.

This property specifies the selector for the target iframe within which the current action should be executed.

Example:

```yaml
actions:
  # Click `#submit` element inside the `.child` iframe
  - click: '#submit'
    frame: .child
  
  # Click `#submit` element inside the `#grandchild` iframe
  # `grandchild` is an element within the `.child` iframe
  - click: '#submit'
    frame:
      - .child
      - '#granchild'
```

{% end %}

[test-suite-state]: @/optimizely-cms/testing/create-test-suite.md#spoiler--state
