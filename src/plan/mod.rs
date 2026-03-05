use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::branding::BrandingConfig;
use crate::cli::PlanScaffoldArgs;
use crate::handoff::session_store::load_session;
use crate::script::{AudioConfig, DemoMode, DemoScript, ExpectCondition, ScriptScene, ScriptStep};

#[derive(Debug, Clone, Serialize)]
pub struct ScaffoldResponse {
    pub ok: bool,
    pub session_id: String,
    pub output: PathBuf,
    pub scenes: usize,
    pub setup_steps: usize,
    pub checks_steps: usize,
}

pub fn scaffold(args: PlanScaffoldArgs) -> Result<ScaffoldResponse> {
    let session = load_session(&args.session)?;
    let fallback_ref = session
        .refs
        .first()
        .ok_or_else(|| anyhow!("session has no refs: {}", args.session))?;
    let help_ref = session
        .refs
        .iter()
        .find(|r| r.source.eq_ignore_ascii_case("help"))
        .unwrap_or(fallback_ref);
    let max_scenes = args.max_scenes.clamp(1, 8);
    let binary = infer_binary_name(&session.target);

    let mut setup = Vec::new();
    if let Some(env_ref) = session.refs.iter().find(|r| {
        r.metadata
            .path
            .as_deref()
            .is_some_and(|p| p.ends_with(".env.example") || p.ends_with(".env.sample"))
    }) {
        setup.push(make_step(
            "setup_env",
            "cp .env.example .env",
            env_ref.ref_id.as_str(),
            Some("setup environment file"),
        ));
    }

    let mut scenes = Vec::new();
    scenes.push(ScriptScene {
        id: "scene_01".to_string(),
        title: "CLI Overview".to_string(),
        steps: vec![make_step(
            "step_01_help",
            format!("{binary} --help"),
            help_ref.ref_id.as_str(),
            Some("usage"),
        )],
    });

    let mut cmd_idx = 2;
    for cmd in scene_commands(&session.discovered_commands, &binary)
        .into_iter()
        .take(max_scenes.saturating_sub(1))
    {
        let run = format!("{binary} {cmd} --help");
        scenes.push(ScriptScene {
            id: format!("scene_{cmd_idx:02}"),
            title: format!("Explore '{cmd}'"),
            steps: vec![make_step(
                &format!("step_{cmd_idx:02}_{cmd}"),
                run,
                help_ref.ref_id.as_str(),
                Some(cmd.as_str()),
            )],
        });
        cmd_idx += 1;
    }

    let checks = vec![make_step(
        "check_version",
        format!("{binary} --version"),
        help_ref.ref_id.as_str(),
        None,
    )];

    let script = DemoScript {
        version: "1".to_string(),
        mode: DemoMode::Terminal,
        setup,
        scenes,
        checks,
        cleanup: Vec::new(),
        redactions: Vec::new(),
        audio: Some(AudioConfig {
            typing: true,
            music_path: None,
        }),
        branding: Some(BrandingConfig {
            title: Some("castkit demo".to_string()),
            watermark_text: Some("castkit.com".to_string()),
            ..BrandingConfig::default()
        }),
        web: None,
    };

    let body = serde_json::to_string_pretty(&script)?;
    fs::write(&args.output, body)?;

    Ok(ScaffoldResponse {
        ok: true,
        session_id: args.session,
        output: args.output,
        scenes: script.scenes.len(),
        setup_steps: script.setup.len(),
        checks_steps: script.checks.len(),
    })
}

fn make_step(
    id: &str,
    run: impl Into<String>,
    source_ref: &str,
    contains: Option<&str>,
) -> ScriptStep {
    ScriptStep {
        id: id.to_string(),
        run: run.into(),
        expect: Some(ExpectCondition {
            contains: contains.map(ToOwned::to_owned),
            regex: None,
            exit_code: Some(0),
        }),
        timeout_ms: Some(120_000),
        source_refs: vec![source_ref.to_string()],
        manual_step: false,
        manual_reason: None,
        artifacts: Vec::new(),
    }
}

fn infer_binary_name(target: &str) -> String {
    Path::new(target)
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or(target)
        .to_string()
}

fn scene_commands(commands: &[String], binary: &str) -> Vec<String> {
    let mut out = Vec::new();
    for cmd in commands {
        let trimmed = cmd.trim();
        if trimmed.is_empty() || trimmed == binary {
            continue;
        }
        if trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            out.push(trimmed.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::{infer_binary_name, scene_commands};

    #[test]
    fn infers_binary_name_from_path() {
        assert_eq!(infer_binary_name("/tmp/mycli"), "mycli");
        assert_eq!(infer_binary_name("mycli"), "mycli");
    }

    #[test]
    fn filters_scene_commands() {
        let commands = vec![
            "mycli".to_string(),
            "init".to_string(),
            "run".to_string(),
            "init".to_string(),
            "bad:cmd".to_string(),
        ];
        let out = scene_commands(&commands, "mycli");
        assert_eq!(out, vec!["init".to_string(), "run".to_string()]);
    }
}
