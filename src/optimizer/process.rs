use anyhow::Result;

use crate::{cli::args::OptimizeArgs, optimizer::image_optimizer};

pub fn optimize(args: OptimizeArgs) -> Result<()> {
    image_optimizer::optimize_images(args)
}
