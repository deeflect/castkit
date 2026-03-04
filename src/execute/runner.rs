use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::execute::transcript::StepRunRecord;
use crate::script::ScriptStep;

pub async fn run_step(cwd: &Path, step: &ScriptStep) -> Result<StepRunRecord> {
    let timeout_ms = step.timeout_ms.unwrap_or(15_000).max(100);
    let duration = Duration::from_millis(timeout_ms);
    let started = Instant::now();

    let mut cmd = Command::new("/bin/bash");
    cmd.arg("-lc")
        .arg(&step.run)
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    let child = cmd
        .spawn()
        .with_context(|| format!("failed spawning command: {}", step.run))?;

    let output = match timeout(duration, child.wait_with_output()).await {
        Ok(result) => result.with_context(|| format!("failed waiting command: {}", step.run))?,
        Err(_) => {
            return Ok(StepRunRecord {
                id: step.id.clone(),
                run: step.run.clone(),
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 124,
                duration_ms: started.elapsed().as_millis(),
                status: "timeout".to_string(),
                error: Some(format!("command timed out after {timeout_ms}ms")),
            });
        }
    };

    Ok(StepRunRecord {
        id: step.id.clone(),
        run: step.run.clone(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(1),
        duration_ms: started.elapsed().as_millis(),
        status: if output.status.success() {
            "ok".to_string()
        } else {
            "failed".to_string()
        },
        error: None,
    })
}
