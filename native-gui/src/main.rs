use anyhow::Result;
use oxide86_core::logging::setup_logging;

fn main() -> Result<()> {
    setup_logging()?;

    Ok(())
}
