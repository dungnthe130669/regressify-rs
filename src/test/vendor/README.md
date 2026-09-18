# Browser Runtime Assets

The generated `injected.js` bundle and `keyboard.json` layout are vendored from
`playwright-core` 1.59.1. They provide selector parsing, Shadow DOM traversal,
and key definitions inside Chromium; browser launch, transport, input, and
scenario execution remain implemented by Rust and Chromium CDP.

The package's LICENSE, NOTICE, and ThirdPartyNotices.txt accompany these files.
To update, extract the `source` export of `lib/generated/injectedScriptSource.js`
and serialize the `USKeyboardLayout` export of `lib/server/usKeyboardLayout.js`
from the same pinned npm release, then rerun the browser compatibility tests.
Do not edit generated assets by hand.