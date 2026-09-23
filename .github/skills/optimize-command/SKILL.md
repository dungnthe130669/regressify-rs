---
name: optimize-command
description: 'Use when changing regressify optimize, PNG compression, speed/minimum-size settings, decoded pixel preservation, or optimization resource limits.'
---

# Optimize PNGs

## Start Here

[Optimizer](../../../src/optimizer/image_optimizer.rs),
[argument validation](../../../src/cli/args.rs), and [PNG helpers](../../../src/helpers/png.rs).

## Procedure and Invariants

1. Preserve the CLI's lossless compression contract; compare decoded pixels, not
   compressed bytes. Do not import lossy deployment settings into this command.
1. Validate speed bounds and minimum size; avoid rewriting skipped files.
   Preserve deterministic discovery and ignore non-PNG files.
1. Enforce image resource limits before expensive decoding/optimization and
   preserve required PNG representation/color metadata.
1. Surface read/decode/write failures. Use safe replacement so interrupted work
   does not corrupt the only copy of a baseline or test image.

## Checks

```sh
cargo test --lib optimizer::image_optimizer::tests
cargo test --lib cli::args::tests::optimization_speed_is_limited_to_supported_range
```

Cover decoded-pixel equality, small skipped files, oversized/corrupt PNGs, and
sorted collection. Use temporary copies, never live reference images.

## Maintain

Update [command docs](../../../docs/commands.md) and this skill for option or
output changes. Comment non-obvious encoding-preservation decisions.
