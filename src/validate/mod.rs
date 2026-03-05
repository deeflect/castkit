pub mod errors;

use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::handoff::session_store::load_session;
use crate::script::{DemoMode, DemoScript, ScriptStep, StepArtifact, WebAction, WebActionType};

pub use errors::{ValidationError, ValidationResult};

static DOTENV_CREATE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(cp|touch|cat|printf|echo)\b.*\b\.env\b").expect("valid regex"));
static CONFIG_CREATE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(cp|touch|cat|printf|echo)\b.*\b(config\.toml|settings\.toml|castkit\.toml)\b")
        .expect("valid regex")
});
static CONFIG_MENTION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(config\.toml|settings\.toml|castkit\.toml)\b").expect("valid regex")
});
static SECRET_LITERAL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(sk-[A-Za-z0-9]{20,}|ghp_[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}|(api[_-]?key|token|secret|password)\s*=\s*\S+)",
    )
    .expect("valid regex")
});

pub fn validate_script(session_id: &str, script: &DemoScript) -> Result<ValidationResult> {
    let session = load_session(session_id)?;
    let mut errors = Vec::new();

    for (idx, rule) in script.redactions.iter().enumerate() {
        if let Err(e) = Regex::new(&rule.pattern) {
            errors.push(err(
                "INVALID_REDACTION_REGEX",
                &format!("redactions[{idx}].pattern"),
                &format!("invalid regex '{}': {e}", rule.pattern),
                None,
            ));
        }
    }

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
        false,
    );

    if matches!(script.mode, DemoMode::Terminal) && script.web.is_some() {
        errors.push(err(
            "UNEXPECTED_WEB_CONFIG",
            "web",
            "web config is only allowed when mode='web'",
            Some("set mode to 'web' or remove web block"),
        ));
    }

    let setup_has_dotenv = script.setup.iter().any(step_creates_dotenv);
    let setup_has_config = script.setup.iter().any(step_creates_config);

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
            !setup_has_config,
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
        !setup_has_config,
    );

    validate_steps(
        &script.cleanup,
        "cleanup",
        &known_refs,
        &allowed_commands,
        script,
        &mut errors,
        !setup_has_dotenv,
        !setup_has_config,
    );

    if matches!(script.mode, DemoMode::Web) {
        validate_web_mode(script, &known_refs, &mut errors);
    }

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
    enforce_config_creation: bool,
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

        if enforce_config_creation && run_mentions_config_file(&step.run) {
            errors.push(err(
                "ORDERING_CONFIG",
                &path,
                "step references config file but setup does not create/copy one",
                Some("add a setup step like 'cp config.example.toml config.toml'"),
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

        validate_artifacts(step, &path, errors);
    }
}

fn validate_web_mode(
    script: &DemoScript,
    known_refs: &BTreeSet<&str>,
    errors: &mut Vec<ValidationError>,
) {
    let Some(web) = script.web.as_ref() else {
        errors.push(err(
            "WEB_CONFIG_REQUIRED",
            "web",
            "mode='web' requires a web config block",
            Some("set script.web with non-empty actions"),
        ));
        return;
    };

    if web.actions.is_empty() {
        errors.push(err(
            "WEB_ACTIONS_REQUIRED",
            "web.actions",
            "mode='web' requires at least one web action",
            Some("add deterministic actions like goto/click/type/assert_text"),
        ));
        return;
    }

    for (idx, action) in web.actions.iter().enumerate() {
        let path = format!("web.actions[{idx}]");
        validate_web_action(action, &path, known_refs, errors);
    }
}

fn validate_web_action(
    action: &WebAction,
    path: &str,
    known_refs: &BTreeSet<&str>,
    errors: &mut Vec<ValidationError>,
) {
    if action.source_refs.is_empty() {
        errors.push(err(
            "MISSING_SOURCE_REFS",
            path,
            "web action must include at least one source_ref",
            None,
        ));
    }

    for source_ref in &action.source_refs {
        if !known_refs.contains(source_ref.as_str()) {
            errors.push(err(
                "INVALID_SOURCE_REF",
                path,
                &format!("source_ref '{source_ref}' not found in session"),
                None,
            ));
        }
    }

    match action.action_type {
        WebActionType::Goto => require_non_empty(
            action.url.as_deref(),
            "WEB_URL_REQUIRED",
            &format!("{path}.url"),
            "goto action requires non-empty url",
            errors,
        ),
        WebActionType::Click | WebActionType::WaitForSelector | WebActionType::ScrollTo => {
            require_non_empty(
                action.selector.as_deref(),
                "WEB_SELECTOR_REQUIRED",
                &format!("{path}.selector"),
                "action requires non-empty selector",
                errors,
            );
        }
        WebActionType::Type => {
            require_non_empty(
                action.selector.as_deref(),
                "WEB_SELECTOR_REQUIRED",
                &format!("{path}.selector"),
                "type action requires non-empty selector",
                errors,
            );
            require_non_empty(
                action.text.as_deref(),
                "WEB_TEXT_REQUIRED",
                &format!("{path}.text"),
                "type action requires non-empty text",
                errors,
            );
        }
        WebActionType::Press => {
            require_non_empty(
                action.key.as_deref(),
                "WEB_KEY_REQUIRED",
                &format!("{path}.key"),
                "press action requires non-empty key",
                errors,
            );
        }
        WebActionType::WaitMs => {
            if action.wait_ms.unwrap_or(0) < 1 {
                errors.push(err(
                    "WEB_WAIT_MS_REQUIRED",
                    &format!("{path}.wait_ms"),
                    "wait_ms action requires wait_ms >= 1",
                    None,
                ));
            }
        }
        WebActionType::AssertText => {
            require_non_empty(
                action.text.as_deref(),
                "WEB_TEXT_REQUIRED",
                &format!("{path}.text"),
                "assert_text action requires non-empty text",
                errors,
            );
        }
        WebActionType::Screenshot => {
            require_non_empty(
                action.path.as_deref(),
                "WEB_PATH_REQUIRED",
                &format!("{path}.path"),
                "screenshot action requires non-empty path",
                errors,
            );
        }
    }
}

fn validate_artifacts(step: &ScriptStep, step_path: &str, errors: &mut Vec<ValidationError>) {
    for (idx, artifact) in step.artifacts.iter().enumerate() {
        let path = format!("{step_path}.artifacts[{idx}]");
        let display = match artifact {
            StepArtifact::Image(v) => {
                validate_relative_path("ARTIFACT_PATH_UNSAFE", &path, "path", &v.path, errors);
                &v.display
            }
            StepArtifact::WebSnapshot(v) => {
                if let Some(p) = &v.path {
                    validate_relative_path("ARTIFACT_PATH_UNSAFE", &path, "path", p, errors);
                }
                &v.display
            }
            StepArtifact::ResultCard(v) => &v.display,
            StepArtifact::Chart(v) => {
                validate_relative_path(
                    "ARTIFACT_PATH_UNSAFE",
                    &path,
                    "data_path",
                    &v.data_path,
                    errors,
                );
                &v.display
            }
        };

        if let Some(ms) = display.show_ms {
            if !(300..=10_000).contains(&ms) {
                errors.push(err(
                    "ARTIFACT_SHOW_MS_RANGE",
                    &format!("{path}.show_ms"),
                    "artifact show_ms must be between 300 and 10000",
                    None,
                ));
            }
        }
    }
}

fn validate_relative_path(
    code: &str,
    parent: &str,
    key: &str,
    value: &str,
    errors: &mut Vec<ValidationError>,
) {
    if !is_safe_relative_path(value) {
        errors.push(err(
            code,
            &format!("{parent}.{key}"),
            "path must be a safe relative path inside sandbox",
            Some("use a relative path without '..' segments"),
        ));
    }
}

fn is_safe_relative_path(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }
    let parsed = Path::new(trimmed);
    if parsed.is_absolute() {
        return false;
    }
    !parsed.components().any(|c| matches!(c, Component::ParentDir))
}

fn require_non_empty(
    value: Option<&str>,
    code: &str,
    path: &str,
    message: &str,
    errors: &mut Vec<ValidationError>,
) {
    if value.is_none_or(|v| v.trim().is_empty()) {
        errors.push(err(code, path, message, None));
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

    let mut expect_command = true;
    for token in tokens {
        if is_command_separator(&token) {
            expect_command = true;
            continue;
        }

        if !expect_command {
            continue;
        }

        if let Some(cmd) = command_from_assignment_token(&token) {
            return Some(cmd);
        }

        if is_env_assignment_token(&token) {
            continue;
        }

        if let Some(cmd) = normalize_command_token(&token) {
            return Some(cmd);
        }
    }

    None
}

fn is_command_separator(token: &str) -> bool {
    matches!(token, "&&" | "||" | ";" | "|")
}

fn is_env_assignment_token(token: &str) -> bool {
    let Some((name, _)) = token.split_once('=') else {
        return false;
    };
    is_env_var_name(name)
}

fn command_from_assignment_token(token: &str) -> Option<String> {
    let (name, value) = token.split_once('=')?;
    if !is_env_var_name(name) {
        return None;
    }

    if value.starts_with("$(") || value.starts_with('`') || value.starts_with('(') {
        return normalize_command_token(value);
    }

    None
}

fn is_env_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn normalize_command_token(token: &str) -> Option<String> {
    let mut cmd = token.trim();
    if cmd.is_empty() {
        return None;
    }

    while let Some(next) = cmd
        .strip_prefix("$(")
        .or_else(|| cmd.strip_prefix('('))
        .or_else(|| cmd.strip_prefix('{'))
        .or_else(|| cmd.strip_prefix('`'))
        .or_else(|| cmd.strip_prefix('"'))
        .or_else(|| cmd.strip_prefix('\''))
    {
        cmd = next.trim_start();
        if cmd.is_empty() {
            return None;
        }
    }

    if cmd.starts_with('$') && !cmd.starts_with("$(") {
        return None;
    }

    cmd = cmd.trim_end_matches(|c: char| {
        matches!(c, ')' | '}' | '`' | '"' | '\'' | ';' | '|' | ',' | ':')
    });

    if cmd.is_empty() || cmd.starts_with('-') || cmd.contains('=') {
        return None;
    }

    if cmd
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/'))
    {
        return Some(cmd.to_string());
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
    DOTENV_CREATE_RE.is_match(step.run.trim())
}

fn run_mentions_dotenv(run: &str) -> bool {
    run.contains(".env")
}

fn step_creates_config(step: &ScriptStep) -> bool {
    CONFIG_CREATE_RE.is_match(step.run.trim())
}

fn run_mentions_config_file(run: &str) -> bool {
    CONFIG_MENTION_RE.is_match(run)
}

fn has_secret_literal(run: &str) -> bool {
    SECRET_LITERAL_RE.is_match(run)
}

#[cfg(test)]
mod tests {
    use super::first_command_token;

    #[test]
    fn first_command_handles_subshell_assignment() {
        let cmd = first_command_token("SESSION=$(ls .castkit/sessions | tail -n1)");
        assert_eq!(cmd.as_deref(), Some("ls"));
    }

    #[test]
    fn first_command_skips_env_prefix_assignment() {
        let cmd =
            first_command_token("FOO=bar BAR=baz castkit handoff list --session \"$SESSION\"");
        assert_eq!(cmd.as_deref(), Some("castkit"));
    }

    #[test]
    fn first_command_handles_plain_command() {
        let cmd = first_command_token("castkit handoff init . --json");
        assert_eq!(cmd.as_deref(), Some("castkit"));
    }
}
