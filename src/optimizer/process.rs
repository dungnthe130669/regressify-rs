use anyhow::Result;

use crate::optimizer::image_optimizer;

pub fn optimize() -> Result<()> {
    image_optimizer::optimize_images()?;

    Ok(())
}
