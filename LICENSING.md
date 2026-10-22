# Licensing

Copyright (C) 2023-2026 Tuyen Pham ([tuyen-at-work](https://github.com/tuyen-at-work))

Regressify is free software: you can redistribute it and/or modify it under the
terms of the GNU Affero General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later version.

Regressify is distributed in the hope that it will be useful, but WITHOUT ANY
WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE. See the GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License along
with Regressify in [LICENSE](LICENSE). If not, see <https://www.gnu.org/licenses/>.

The Corresponding Source for Regressify, including its released executables, is
available at <https://github.com/precise-alloy/regressify-rs>.

This license covers the whole repository, including the Rust source, snapshot
frontend, documentation, and build scripts, except the third-party components
listed below.

## Output Exception

Additional permission under GNU AGPL version 3 section 7:

As a special exception, the copyright holder gives you permission to use, copy,
modify, and distribute Regressify Output under terms of your choice, without the
conditions of the GNU Affero General Public License. This permission also covers
works that you derive from Regressify Output.

"Regressify Output" means:

1. Report and landing-site files that Regressify writes or deploys, including the
   HTML, JavaScript, CSS, icons, and hosting configuration copied from its built
   snapshot frontend;
1. Projects exported by `regressify mcp`, including their Worker code, tests,
   configuration, guides, schemas, and examples;
1. Files written by `regressify init`, including JSON schemas, editor settings,
   MCP configuration, and agent guidance; and
1. Example suites, scripts, and code samples in the `docs/` and `visual_tests/`
   directories.

This exception does not apply to the rest of Regressify, including its
executables, its Rust and frontend source code, or modified versions of them,
which remain under the GNU Affero General Public License. Third-party components
included in Regressify Output remain under their own licenses.

If you modify Regressify, you may extend this exception to your version, but you
are not obligated to do so. If you do not wish to do so, delete this exception
statement from your version.

## Third-Party Components

Third-party components keep their own licenses:

- [Browser runtime assets](src/test/vendor/README.md): Playwright's generated
  injected script and keyboard layout, under the Apache License 2.0 with the
  accompanying NOTICE and third-party notices.
- [Report fonts](frontend/src/assets/fonts): Roboto Flex and JetBrains Mono,
  under the SIL Open Font License 1.1.
- Rust crates and npm packages, under the licenses declared by their authors.
  [deny.toml](deny.toml) lists the licenses allowed for Rust dependencies;
  `cargo deny check licenses` enforces it.
