use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::execute::transcript::StepRunRecord;
use crate::script::ScriptStep;

pub async fn run_step(
    cwd: &Path,
    step: &ScriptStep,
    env_vars: &BTreeMap<String, String>,
) -> Result<StepRunRecord> {
    let timeout_ms = step.timeout_ms.unwrap_or(15_000).max(100);
    let duration = Duration::from_millis(timeout_ms);
    let started = Instant::now();

    let mut cmd = Command::new("/bin/bash");
    cmd.arg("-lc")
        .arg(&step.run)
        .current_dir(cwd)
        .envs(env_vars)
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::run_step;
    use crate::script::ScriptStep;

    #[tokio::test]
    async fn run_step_receives_env_vars() {
        let cwd = tempfile::tempdir().expect("tempdir");
        let step = ScriptStep {
            id: "s1".to_string(),
            run: "printf '%s' \"$SESSION\"".to_string(),
            expect: None,
            timeout_ms: Some(2_000),
            source_refs: vec!["ref_help_0001".to_string()],
            manual_step: false,
            manual_reason: None,
            artifacts: Vec::new(),
        };
        let mut env_vars = BTreeMap::new();
        env_vars.insert("SESSION".to_string(), "sess_env_test".to_string());

        let record = run_step(cwd.path(), &step, &env_vars).await.expect("run");
        assert_eq!(record.status, "ok");
        assert_eq!(record.stdout, "sess_env_test");
    }
}
