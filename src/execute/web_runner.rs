use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::execute::transcript::WebActionRecord;
use crate::script::{WebActionType, WebConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebRunnerInput {
    web: WebConfig,
}

#[derive(Debug, Clone, Deserialize)]
struct WebRunnerOutput {
    ok: bool,
    actions: Vec<WebActionRecord>,
    error: Option<String>,
}

pub async fn run_web_actions(cwd: &Path, web: &WebConfig) -> Result<Vec<WebActionRecord>> {
    if std::env::var("CASTKIT_WEB_RUNNER_STUB")
        .ok()
        .is_some_and(|v| v == "1")
    {
        return Ok(stub_actions(web));
    }

    let renderer_home = resolve_renderer_home()?;
    let runner_script = renderer_home.join("web-runner.mjs");
    if !runner_script.exists() {
        anyhow::bail!("web runner script missing: {}", runner_script.display());
    }

    let input_path = std::env::temp_dir().join(format!(
        "castkit-web-runner-input-{}.json",
        uuid::Uuid::new_v4().simple()
    ));
    let output_path = std::env::temp_dir().join(format!(
        "castkit-web-runner-output-{}.json",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::write(
        &input_path,
        serde_json::to_vec(&WebRunnerInput { web: web.clone() })?,
    )?;

    let output = Command::new("node")
        .arg(&runner_script)
        .arg("--config")
        .arg(&input_path)
        .arg("--output")
        .arg(&output_path)
        .arg("--cwd")
        .arg(cwd)
        .output()
        .await
        .context("failed to run web runner")?;

    if !output.status.success() {
        anyhow::bail!(
            "web runner failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let body = std::fs::read_to_string(&output_path)
        .with_context(|| format!("failed reading web runner output {}", output_path.display()))?;
    let parsed: WebRunnerOutput =
        serde_json::from_str(&body).context("invalid web runner output JSON")?;
    if !parsed.ok {
        anyhow::bail!(
            "web runner reported failure: {}",
            parsed
                .error
                .unwrap_or_else(|| "unknown web runner error".to_string())
        );
    }

    Ok(parsed.actions)
}

fn stub_actions(web: &WebConfig) -> Vec<WebActionRecord> {
    web.actions
        .iter()
        .enumerate()
        .map(|(idx, action)| {
            let t_ms = idx as u64 * 280;
            WebActionRecord {
                id: action.id.clone(),
                action_type: web_action_type_label(action.action_type).to_string(),
                status: "ok".to_string(),
                error: None,
                t_ms,
                duration_ms: 160,
                selector: action.selector.clone(),
                cursor_x: action.selector.as_ref().map(|_| 640.0 + (idx as f32 * 8.0)),
                cursor_y: action.selector.as_ref().map(|_| 340.0 + (idx as f32 * 6.0)),
                target_x: action.selector.as_ref().map(|_| 600.0),
                target_y: action.selector.as_ref().map(|_| 320.0),
                target_w: action.selector.as_ref().map(|_| 220.0),
                target_h: action.selector.as_ref().map(|_| 58.0),
                screenshot_path: action.path.clone(),
            }
        })
        .collect()
}

fn web_action_type_label(action_type: WebActionType) -> &'static str {
    match action_type {
        WebActionType::Goto => "goto",
        WebActionType::Click => "click",
        WebActionType::Type => "type",
        WebActionType::Press => "press",
        WebActionType::WaitForSelector => "wait_for_selector",
        WebActionType::WaitMs => "wait_ms",
        WebActionType::AssertText => "assert_text",
        WebActionType::Screenshot => "screenshot",
        WebActionType::ScrollTo => "scroll_to",
    }
}

fn resolve_renderer_home() -> Result<PathBuf> {
    if let Ok(home) = std::env::var("CASTKIT_RENDERER_HOME") {
        return Ok(PathBuf::from(home));
    }

    let candidate = std::env::current_dir()?.join("renderer-runtime");
    if candidate.join("render.mjs").exists() {
        return Ok(candidate);
    }

    anyhow::bail!("no renderer home found; expected ./renderer-runtime with render.mjs")
}
