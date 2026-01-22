use anyhow::{Result, bail};

use crate::{cli::args::OptimizeArgs, optimizer::image_optimizer};

pub fn optimize(args: OptimizeArgs) -> Result<()> {
    if args.min_quality > args.max_quality {
        bail!(
            "Minimum quality ({}) cannot be greater than maximum quality ({}).",
            args.min_quality,
            args.max_quality
        );
    }

    image_optimizer::optimize_images(args)?;

    Ok(())
}
