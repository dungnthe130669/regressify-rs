use std::io::{self, Write};

fn main() -> anyhow::Result<()> {
    io::stdout().write_all(&regressify::mcp::catalog_json()?)?;
    Ok(())
}
