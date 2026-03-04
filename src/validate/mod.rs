pub mod errors;

use std::collections::BTreeSet;

use anyhow::Result;
use regex::Regex;

use crate::handoff::session_store::load_session;
use crate::script::{DemoScript, ScriptStep};

pub use errors::{ValidationError, ValidationResult};

pub fn validate_script(session_id: &str, script: &DemoScript) -> Result<ValidationResult> {
    let session = load_session(session_id)?;
    let mut errors = Vec::new();

    let known_refs: BTreeSet<&str> = session.refs.iter().map(|r| r.ref_id.as_str()).collect();

    let mut allowed_commands: BTreeSet<String> =
        session.discovered_commands.clone().into_iter().collect();
    for item in &session.refs {
        allowed_commands.extend(extract_commands_from_text(&item.content));
    }

    validate_steps(
        &script.setup,
        "setup",
        &known_refs,
        &allowed_commands,
        script,
        &mut errors,
        false,
    );

    let setup_has_dotenv = script.setup.iter().any(step_creates_dotenv);

    for (scene_index, scene) in script.scenes.iter().enumerate() {
        let prefix = format!("scenes[{scene_index}].steps");
        validate_steps(
            &scene.steps,
            &prefix,
            &known_refs,
            &allowed_commands,
            script,
            &mut errors,
            !setup_has_dotenv,
        );
    }

    validate_steps(
        &script.checks,
        "checks",
        &known_refs,
        &allowed_commands,
        script,
        &mut errors,
        !setup_has_dotenv,
    );

    validate_steps(
        &script.cleanup,
        "cleanup",
        &known_refs,
        &allowed_commands,
        script,
        &mut errors,
        !setup_has_dotenv,
    );

    Ok(ValidationResult::from_errors(errors))
}

#[allow(clippy::too_many_arguments)]
fn validate_steps(
    steps: &[ScriptStep],
    group_path: &str,
    known_refs: &BTreeSet<&str>,
    allowed_commands: &BTreeSet<String>,
    script: &DemoScript,
    errors: &mut Vec<ValidationError>,
    enforce_dotenv_creation: bool,
) {
    for (idx, step) in steps.iter().enumerate() {
        let path = format!("{group_path}[{idx}]");

        if step.source_refs.is_empty() {
            errors.push(err(
                "MISSING_SOURCE_REFS",
                &path,
                "step must include at least one source_ref",
                None,
            ));
        }

        for r in &step.source_refs {
            if !known_refs.contains(r.as_str()) {
                errors.push(err(
                    "INVALID_SOURCE_REF",
                    &path,
                    &format!("source_ref '{r}' not found in session"),
                    None,
                ));
            }
        }

        if step.manual_step
            && step
                .manual_reason
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            errors.push(err(
                "MANUAL_REASON_REQUIRED",
                &path,
                "manual_step=true requires manual_reason",
                None,
            ));
        }

        if let Some(cmd) = first_command_token(&step.run) {
            if !step.manual_step && !allowed_commands.contains(&cmd) && !is_shell_builtin(&cmd) {
                errors.push(err(
                    "UNKNOWN_COMMAND",
                    &format!("{path}.run"),
                    &format!("command '{cmd}' not found in discovered graph"),
                    Some("mark as manual_step with manual_reason and supporting refs"),
                ));
            }
        }

        if enforce_dotenv_creation && run_mentions_dotenv(&step.run) {
            errors.push(err(
                "ORDERING_DOTENV",
                &path,
                "step references .env but setup does not create/copy .env",
                Some("add a setup step like 'cp .env.example .env'"),
            ));
        }

        if has_secret_literal(&step.run) && script.redactions.is_empty() {
            errors.push(err(
                "UNSAFE_SECRET_LITERAL",
                &format!("{path}.run"),
                "inline secret literal detected but script.redactions is empty",
                Some("add redaction patterns or avoid inline secret literals"),
            ));
        }
    }
}

fn err(code: &str, path: &str, message: &str, hint: Option<&str>) -> ValidationError {
    ValidationError {
        code: code.to_string(),
        path: path.to_string(),
        message: message.to_string(),
        hint: hint.map(ToOwned::to_owned),
    }
}

fn extract_commands_from_text(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(token) = first_command_token(trimmed) {
            if token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
                && !token.starts_with('-')
                && token.len() < 64
            {
                out.insert(token);
            }
        }
    }

    out
}

fn first_command_token(run: &str) -> Option<String> {
    let tokens = shell_words::split(run).ok().or_else(|| {
        Some(
            run.split_whitespace()
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>(),
        )
    })?;

    for token in tokens {
        if token == "&&" || token == "||" || token == ";" || token == "|" {
            continue;
        }

        if token.contains('=') && !token.starts_with("./") && !token.starts_with('/') {
            let parts: Vec<&str> = token.split('=').collect();
            if parts.len() == 2
                && !parts[0].is_empty()
                && parts[0]
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
            {
                continue;
            }
        }

        return Some(token);
    }

    None
}

fn is_shell_builtin(cmd: &str) -> bool {
    matches!(
        cmd,
        "cd" | "cp"
            | "mv"
            | "rm"
            | "mkdir"
            | "touch"
            | "cat"
            | "echo"
            | "printf"
            | "test"
            | "["
            | "pwd"
            | "ls"
            | "export"
            | "source"
            | "env"
            | "grep"
            | "awk"
            | "sed"
            | "sh"
            | "bash"
            | "zsh"
    )
}

fn step_creates_dotenv(step: &ScriptStep) -> bool {
    let run = step.run.trim();
    let copy = Regex::new(r"\b(cp|touch|cat|printf|echo)\b.*\b\.env\b").expect("regex");
    copy.is_match(run)
}

fn run_mentions_dotenv(run: &str) -> bool {
    run.contains(".env")
}

fn has_secret_literal(run: &str) -> bool {
    let secret = Regex::new(
        r"(?i)(sk-[A-Za-z0-9]{20,}|ghp_[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}|(api[_-]?key|token|secret|password)\s*=\s*\S+)",
    )
    .expect("regex");
    secret.is_match(run)
}
