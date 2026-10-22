# Licensing

Verified: 2026-10-22

- [LICENSE](../../LICENSE) is the verbatim GNU AGPL v3 text from
  <https://www.gnu.org/licenses/agpl-3.0.txt>: 34,523 bytes, LF line endings,
  SHA-256 `0D96A4FF68AD6D4B6F1F30F713B18D5184912BA8DD389F86AA7710DB079ABCB0`.
  Never edit it; compare this hash after any rewrite.
- [LICENSING.md](../../LICENSING.md) owns the "or later" notice, the section 7
  output exception, and third-party scope. When a feature starts copying
  Regressify files into reports, exports, or workspaces, check whether the
  exception's "Regressify Output" list must name it.
- [deny.toml](../../deny.toml) allows only licenses currently needed. Desktop's
  `imagequant` is GPL-3.0-or-later, allowed because AGPL section 13 permits GPLv3
  combinations.
- cargo-deny 0.20 takes `--config <path>` before `check`
  (`cargo deny --config <path> check licenses`). After `check` it is a usage
  error (exit 2), not a policy result; license rejections exit 4.
- Dropping `GPL-3.0-or-later` or `AGPL-3.0-or-later` from a temporary policy is
  rejected for `imagequant` and the root crate, proving the gate is effective.
- The report fonts are subsets whose license `name` records were stripped;
  fontTools identified Roboto Flex and JetBrains Mono 2.304. The missing notices
  are tracked in [OUTSTANDING_ISSUES.md](../../OUTSTANDING_ISSUES.md).
