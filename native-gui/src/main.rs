use anyhow::Result;
use oxide86_native_common::logging::setup_logging;

fn main() -> Result<()> {
    setup_logging()?;

    Ok(())
}
