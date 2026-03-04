pub mod types;

use anyhow::{Context, Result};

pub use types::*;

pub fn parse_script(body: &str) -> Result<DemoScript> {
    let parsed: DemoScript =
        serde_json::from_str(body).context("invalid DemoScript JSON (strict schema)")?;

    if parsed.version.trim().is_empty() {
        anyhow::bail!("script version must be non-empty");
    }

    Ok(parsed)
}
