use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::handoff::types::HandoffSession;

fn castkit_home() -> Result<PathBuf> {
    if let Ok(custom) = env::var("CASTKIT_HOME") {
        let expanded = shellexpand::tilde(&custom).to_string();
        return Ok(PathBuf::from(expanded));
    }

    Ok(env::current_dir()?.join(".castkit"))
}

pub fn session_dir() -> Result<PathBuf> {
    let dir = castkit_home()?.join("sessions");
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create session dir {}", dir.display()))?;
    Ok(dir)
}

pub fn save_session(session: &HandoffSession) -> Result<()> {
    let path = session_dir()?.join(format!("{}.json", session.session_id));
    let body = serde_json::to_string_pretty(session)?;
    fs::write(&path, body).with_context(|| format!("failed to write {}", path.display()))
}

pub fn load_session(session_id: &str) -> Result<HandoffSession> {
    let path = session_dir()?.join(format!("{}.json", session_id));
    let body = fs::read_to_string(&path)
        .with_context(|| format!("failed to read session {}", path.display()))?;
    let session: HandoffSession = serde_json::from_str(&body)
        .with_context(|| format!("failed to parse session {}", path.display()))?;
    Ok(session)
}
