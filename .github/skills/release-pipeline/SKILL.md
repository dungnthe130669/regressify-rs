---
name: release-pipeline
description: 'Use when changing release workflow matrices, Rust target builds, artifact names, Tuyen Drive manifests, installer hashes, or publish verification.'
---

# Release Pipeline

## Start Here

[Release workflow](../../workflows/tuyen-release.yml),
[build script](../../../ci/make-release.sh),
[Drive helper](../../../ci/drive_release.py), and
[offline release tests](../../../ci/test-release.py).

## Procedure and Invariants

1. Trace matrix inputs to target/toolchain/features, artifact names, upload fields,
   installer versions, and update manifests. Do not assume CLI/desktop targets match.
1. Preserve `--locked`, static-runtime flags where required, archive contents,
   executable resource versions, and immutable installer naming/hash/size contracts.
1. Keep preparation/verification separate from publication. A script or workflow
   edit does not authorize a tag, release upload, real remote operation, or install.
1. Validate external responses and URLs with bounded reads and clear errors.
   Never print publishing secrets or embed credentials in artifacts/manifests.
1. Inspect path filters so changed release files actually trigger their intended
   offline gates. Do not claim a release-only installer test is a pull-request gate.

## Checks

```sh
python ci/test-release.py
```

Requires PyYAML. Tests should use synthetic credentials/local fixture responses,
not live publishing commands. Run affected target build checks only when the
toolchain exists; explicitly report untested cross-platform packaging.

## Maintain

Update [README](../../../README.md), [desktop docs](../../../docs/desktop.md),
applicable install docs, and this skill for release-contract changes. Keep scripts,
workflow inputs, updater expectations, and test fixtures synchronized.
