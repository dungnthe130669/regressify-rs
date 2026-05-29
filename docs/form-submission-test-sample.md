---
title: 'Sample: Form Submission'
description: Practice Regressify form submission testing with a sample registration form that includes validation rules, required fields, and password checks.
guid: ca4b79ad-2672-4a33-91c8-fc917fbba216
date: 2023-07-03T17:00:00+00:00
updated: 2026-04-17T17:00:00+00:00
draft: false
hidden: false
weight: 83000
sort_by: weight
template: docs/page.html
styles:
  - assets/css/b-sample-user-registration-form-critical.css

extra:
  toc: true
  top: false
---

This sample page gives you a registration form with enough validation rules to test realistic input and submit flows without having to build a throwaway form first.

## Use case

Consider the following sample form:

{{ load_asset(path='assets/js/sample-registration-form.js', module=true, defer=true) }}

```html injected
<div class="t-o-sample-user-registration-form">
  <form id="signup" class="form">
    <input type="hidden" name="action" value="signup">
    <h3>Sign Up</h3>

    <p>Fields marked with <strong>(*)</strong> are required.</p>

    <div class="form-field required">
      <label for="username">Username:</label>
      <input type="text" name="username" id="username" required autocomplete="off" placeholder="Choose a username">
      <small id="usernameHint"></small>
    </div>
    <div class="form-field required">
      <label for="email">Email:</label>
      <input type="text" name="email" id="email" required autocomplete="off" placeholder="Enter your email">
      <small id="emailHint"></small>
    </div>
    <div class="form-field required">
      <label for="password">Password:</label>
      <input type="password" name="password" id="password" required autocomplete="off" placeholder="Enter a strong password">
      <small id="passwordHint"></small>
    </div>
    <div class="form-field required">
      <label for="confirm-password">Confirm Password:</label>
      <input type="password" name="confirm-password" id="confirm-password" required autocomplete="off" placeholder="Reenter your password">
      <small id="confirmPasswordHint"></small>
    </div>
    <div class="form-field">
      <input type="submit" class="btn" value="Sign Up">
    </div>
  </form>
</div>
```

The form has the following requirements:

- [X] All fields are mandatory.

- [X] Username (after trimming leading and trailing whitespace):

  \- Must have at least 3 characters.  
  \- Must not exceed 25 characters.

- [X] Email must match the following regular expression pattern:

  ```regex wrap trim
  ^(([^<>()\[\]\\.,;:\s@"]+(\.[^<>()\[\]\\.,;:\s@"]+)*)|(".+"))@((\[[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\])|(([a-zA-Z\-0-9]+\.)+[a-zA-Z]{2,}))$
  ```

   You can independently verify emails against this pattern on [regex101.com][regex101], using **ECMAScript (JavaScript)** flavor.

- [X] Password must:

  \- Have at least 8 characters.  
  \- Have at least 1 lowercase character.  
  \- Have at least 1 uppercase character.  
  \- Have at least 1 numeric digit.  
  \- Have at least 1 special character from the set `!@#$%^&*`.  

- [X] Confirm Password must match the Password field.

## Test suite

To create the test suite, follow these steps:

1. Create a file named **visual_tests/form-submission.tests.yaml** in your test project.

1. Add the following content to the file:

   ```yml h500
   #* @include ./_form-submission.tests.yaml *#
   ```

## Verify

Execute the following command to verify the test suite:

```bash
regressify ref --test-suite form-submission
regressify approve --test-suite form-submission
regressify test --test-suite form-submission
```

[regex101]: https://regex101.com/r/iKtMXj/1 "Email Regular Expression Test"
