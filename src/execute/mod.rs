pub mod redact;
pub mod runner;
pub mod transcript;

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use tempfile::TempDir;

use crate::branding::BrandingConfig;
use crate::cli::{
    EncoderMode as CliEncoderMode, ExecuteArgs, ExecutePreset as CliExecutePreset,
    KeystrokeProfile as CliKeystrokeProfile, OutputFormat as CliOutputFormat,
    RenderSpeed as CliRenderSpeed, ThemePreset,
};
use crate::render::{
    render_screenstudio, KeystrokeProfile, RenderArtifacts, RenderEncoderMode, RenderOptions,
    RenderOutputFormat, RenderSpeedPreset,
};
use crate::script::{DemoScript, ExpectCondition, ScriptStep};
use crate::validate::{validate_script, ValidationError, ValidationResult};

use self::redact::Redactor;
use self::runner::run_step;
use self::transcript::{ExecutionTranscript, SceneTranscript, StepRunRecord};

#[derive(Debug, Clone, Serialize)]
pub struct ExecuteResponse {
    pub ok: bool,
    pub session_id: String,
    pub output: Option<PathBuf>,
    pub transcript_path: Option<PathBuf>,
    pub validation: Option<ValidationResult>,
    pub failures: Vec<ExecutionFailure>,
    pub render: Option<RenderArtifacts>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutionFailure {
    pub step_path: String,
    pub reason: String,
    pub record: StepRunRecord,
}

#[derive(Debug, Clone, Default)]
struct BrandingOverrides {
    title: Option<String>,
    watermark_text: Option<String>,
    avatar_x: Option<String>,
    avatar_url: Option<String>,
    avatar_label: Option<String>,
}

pub async fn execute(args: ExecuteArgs, script: DemoScript) -> Result<ExecuteResponse> {
    if !args.non_interactive {
        return Ok(ExecuteResponse {
            ok: false,
            session_id: args.session,
            output: None,
            transcript_path: None,
            validation: None,
            failures: vec![ExecutionFailure {
                step_path: "execute".to_string(),
                reason: "--non-interactive is required".to_string(),
                record: empty_record(),
            }],
            render: None,
        });
    }

    let validation = validate_script(&args.session, &script)?;
    if !validation.ok {
        return Ok(ExecuteResponse {
            ok: false,
            session_id: args.session,
            output: None,
            transcript_path: None,
            validation: Some(validation),
            failures: vec![],
            render: None,
        });
    }

    let sandbox = tempfile::tempdir()?;
    let redactor = Redactor::from_rules(&script.redactions)?;
    let mut transcript = ExecutionTranscript {
        session_id: args.session.clone(),
        started_at: Utc::now(),
        setup: Vec::new(),
        checks: Vec::new(),
        scenes: Vec::new(),
        cleanup: Vec::new(),
    };

    let mut failures = Vec::new();

    execute_group(
        sandbox.path(),
        &script.setup,
        "setup",
        &mut transcript.setup,
        &mut failures,
        &redactor,
    )
    .await;

    execute_group(
        sandbox.path(),
        &script.checks,
        "checks",
        &mut transcript.checks,
        &mut failures,
        &redactor,
    )
    .await;

    for (scene_idx, scene) in script.scenes.iter().enumerate() {
        let mut scene_transcript = SceneTranscript {
            id: scene.id.clone(),
            title: scene.title.clone(),
            steps: Vec::new(),
        };

        execute_group(
            sandbox.path(),
            &scene.steps,
            &format!("scenes[{scene_idx}].steps"),
            &mut scene_transcript.steps,
            &mut failures,
            &redactor,
        )
        .await;

        transcript.scenes.push(scene_transcript);
    }

    execute_group(
        sandbox.path(),
        &script.cleanup,
        "cleanup",
        &mut transcript.cleanup,
        &mut failures,
        &redactor,
    )
    .await;

    let transcript_path = write_transcript(&transcript, &sandbox)?;

    if !failures.is_empty() {
        return Ok(ExecuteResponse {
            ok: false,
            session_id: args.session,
            output: None,
            transcript_path: Some(transcript_path),
            validation: Some(validation),
            failures,
            render: None,
        });
    }

    let music_path = args.music.or_else(|| {
        script
            .audio
            .as_ref()
            .and_then(|a| a.music_path.as_ref())
            .map(PathBuf::from)
    });
    let tuning = resolve_render_tuning(
        args.preset,
        args.fps,
        args.theme,
        args.speed,
        args.keystroke_profile,
    );

    let typing_sound = if matches!(tuning.keystroke_profile, CliKeystrokeProfile::Silent) {
        false
    } else if let Some(audio) = &script.audio {
        args.typing_sound || audio.typing
    } else {
        args.typing_sound
    };
    let file_branding = load_branding(args.branding.as_ref())?;
    let merged_branding = merge_branding(
        tuning.theme,
        script.branding.clone(),
        file_branding,
        BrandingOverrides {
            title: args.brand_title.clone(),
            watermark_text: args.watermark.clone(),
            avatar_x: args.avatar_x.clone(),
            avatar_url: args.avatar_url.clone(),
            avatar_label: args.avatar_label.clone(),
        },
    );

    let render_opts = RenderOptions {
        output_path: args.output.clone(),
        format: map_output_format(args.format.clone()),
        fps: tuning.fps,
        no_zoom: args.no_zoom,
        typing_sound,
        music_path,
        branding: merged_branding,
        speed: map_render_speed(tuning.speed),
        encoder_mode: map_encoder_mode(args.encoder),
        keystroke_profile: map_keystroke_profile(tuning.keystroke_profile),
        avatar_cache_dir: args.avatar_cache_dir.clone(),
        verbose: is_verbose(),
    };
    let render_artifacts = render_screenstudio(&transcript, render_opts)?;

    Ok(ExecuteResponse {
        ok: true,
        session_id: args.session,
        output: Some(args.output),
        transcript_path: Some(transcript_path),
        validation: Some(validation),
        failures,
        render: Some(render_artifacts),
    })
}

async fn execute_group(
    cwd: &std::path::Path,
    steps: &[ScriptStep],
    group: &str,
    out: &mut Vec<StepRunRecord>,
    failures: &mut Vec<ExecutionFailure>,
    redactor: &Redactor,
) {
    if !failures.is_empty() {
        return;
    }

    for (idx, step) in steps.iter().enumerate() {
        let path = format!("{group}[{idx}]");
        match run_step(cwd, step).await {
            Ok(record_raw) => {
                let expectation_error = evaluate_expectation(step.expect.as_ref(), &record_raw);
                let has_expected_exit = step.expect.as_ref().and_then(|e| e.exit_code).is_some();
                let failed = expectation_error.is_some()
                    || (!has_expected_exit && record_raw.status != "ok");
                let record = redactor.redact_record(record_raw);
                out.push(record.clone());
                if failed {
                    failures.push(ExecutionFailure {
                        step_path: path,
                        reason: redactor.redact_text(
                            &expectation_error
                                .unwrap_or_else(|| "command exited non-zero".to_string()),
                        ),
                        record,
                    });
                    break;
                }
            }
            Err(err) => {
                let record = redactor.redact_record(StepRunRecord {
                    id: step.id.clone(),
                    run: step.run.clone(),
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 1,
                    duration_ms: 0,
                    status: "failed".to_string(),
                    error: Some(err.to_string()),
                });
                out.push(record.clone());
                failures.push(ExecutionFailure {
                    step_path: path,
                    reason: redactor.redact_text(&err.to_string()),
                    record,
                });
                break;
            }
        }
    }
}

fn evaluate_expectation(
    expect: Option<&ExpectCondition>,
    record: &StepRunRecord,
) -> Option<String> {
    let expect = expect?;
    if let Some(code) = expect.exit_code {
        if record.exit_code != code {
            return Some(format!(
                "expected exit_code={code}, got {}",
                record.exit_code
            ));
        }
    }

    if let Some(needle) = &expect.contains {
        if !record.stdout.contains(needle) && !record.stderr.contains(needle) {
            return Some(format!("expected output to contain '{needle}'"));
        }
    }

    if let Some(pattern) = &expect.regex {
        match regex::Regex::new(pattern) {
            Ok(re) => {
                if !re.is_match(&record.stdout) && !re.is_match(&record.stderr) {
                    return Some(format!("expected output to match regex '{pattern}'"));
                }
            }
            Err(e) => {
                return Some(format!("invalid expectation regex '{pattern}': {e}"));
            }
        }
    }

    None
}

fn write_transcript(transcript: &ExecutionTranscript, _sandbox: &TempDir) -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!(
        "castkit-exec-transcript-{}.json",
        uuid::Uuid::new_v4().simple()
    ));
    let body = serde_json::to_string_pretty(transcript)?;
    fs::write(&path, body)?;
    Ok(path)
}

fn empty_record() -> StepRunRecord {
    StepRunRecord {
        id: "n/a".to_string(),
        run: "n/a".to_string(),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 1,
        duration_ms: 0,
        status: "failed".to_string(),
        error: None,
    }
}

fn load_branding(path: Option<&PathBuf>) -> Result<Option<BrandingConfig>> {
    let Some(path) = path else {
        return Ok(None);
    };
    let body = fs::read_to_string(path)?;
    let branding = serde_json::from_str::<BrandingConfig>(&body)?;
    if branding.is_empty() {
        return Ok(None);
    }
    Ok(Some(branding))
}

fn merge_branding(
    theme: Option<ThemePreset>,
    script_branding: Option<BrandingConfig>,
    file_branding: Option<BrandingConfig>,
    overrides: BrandingOverrides,
) -> Option<BrandingConfig> {
    let mut merged = theme.map(theme_branding);
    if let Some(script) = script_branding {
        merged = Some(match merged {
            Some(base) => base.overlay(script),
            None => script,
        });
    }
    if let Some(file) = file_branding {
        merged = Some(match merged {
            Some(base) => base.overlay(file),
            None => file,
        });
    }

    if let Some(title) = overrides.title {
        let mut branding = merged.unwrap_or_default();
        branding.title = Some(title);
        merged = Some(branding);
    }
    if let Some(watermark_text) = overrides.watermark_text {
        let mut branding = merged.unwrap_or_default();
        branding.watermark_text = Some(watermark_text);
        merged = Some(branding);
    }
    if let Some(avatar_x) = overrides.avatar_x {
        let mut branding = merged.unwrap_or_default();
        branding.avatar_x = Some(avatar_x);
        merged = Some(branding);
    }
    if let Some(avatar_url) = overrides.avatar_url {
        let mut branding = merged.unwrap_or_default();
        branding.avatar_url = Some(avatar_url);
        merged = Some(branding);
    }
    if let Some(avatar_label) = overrides.avatar_label {
        let mut branding = merged.unwrap_or_default();
        branding.avatar_label = Some(avatar_label);
        merged = Some(branding);
    }

    merged.filter(|b| !b.is_empty())
}

fn theme_branding(theme: ThemePreset) -> BrandingConfig {
    match theme {
        ThemePreset::Clean => BrandingConfig {
            title: Some("castkit • screenstudio mode".to_string()),
            bg_primary: Some("#0A1020".to_string()),
            bg_secondary: Some("#14243B".to_string()),
            text_primary: Some("#EAF2FF".to_string()),
            text_muted: Some("#9CB2D1".to_string()),
            command_text: Some("#8ED0FF".to_string()),
            accent: Some("#69C2FF".to_string()),
            ..BrandingConfig::default()
        },
        ThemePreset::Bold => BrandingConfig {
            title: Some("castkit • bold mode".to_string()),
            bg_primary: Some("#111318".to_string()),
            bg_secondary: Some("#2A1A12".to_string()),
            text_primary: Some("#FFF2EA".to_string()),
            text_muted: Some("#D4BCA8".to_string()),
            command_text: Some("#FFB17A".to_string()),
            accent: Some("#FF8B47".to_string()),
            ..BrandingConfig::default()
        },
        ThemePreset::Minimal => BrandingConfig {
            title: Some("castkit".to_string()),
            bg_primary: Some("#0D0F14".to_string()),
            bg_secondary: Some("#10141B".to_string()),
            text_primary: Some("#E8ECF4".to_string()),
            text_muted: Some("#9EA8B8".to_string()),
            command_text: Some("#D2D9E8".to_string()),
            accent: Some("#BBC6DD".to_string()),
            ..BrandingConfig::default()
        },
    }
}

fn map_render_speed(speed: CliRenderSpeed) -> RenderSpeedPreset {
    match speed {
        CliRenderSpeed::Fast => RenderSpeedPreset::Fast,
        CliRenderSpeed::Quality => RenderSpeedPreset::Quality,
    }
}

fn map_encoder_mode(mode: CliEncoderMode) -> RenderEncoderMode {
    match mode {
        CliEncoderMode::Auto => RenderEncoderMode::Auto,
        CliEncoderMode::Software => RenderEncoderMode::Software,
        CliEncoderMode::Hardware => RenderEncoderMode::Hardware,
    }
}

fn map_output_format(format: CliOutputFormat) -> RenderOutputFormat {
    match format {
        CliOutputFormat::Mp4 => RenderOutputFormat::Mp4,
        CliOutputFormat::Gif => RenderOutputFormat::Gif,
        CliOutputFormat::Webm => RenderOutputFormat::Webm,
    }
}

fn map_keystroke_profile(profile: CliKeystrokeProfile) -> KeystrokeProfile {
    match profile {
        CliKeystrokeProfile::Mechanical => KeystrokeProfile::Mechanical,
        CliKeystrokeProfile::Laptop => KeystrokeProfile::Laptop,
        CliKeystrokeProfile::Silent => KeystrokeProfile::Silent,
    }
}

fn is_verbose() -> bool {
    std::env::var("CASTKIT_VERBOSE")
        .map(|v| v == "1")
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy)]
struct RenderTuning {
    fps: u32,
    theme: Option<ThemePreset>,
    speed: CliRenderSpeed,
    keystroke_profile: CliKeystrokeProfile,
}

#[derive(Debug, Clone, Copy)]
struct PresetDefaults {
    fps: u32,
    theme: ThemePreset,
    speed: CliRenderSpeed,
    keystroke_profile: CliKeystrokeProfile,
}

fn resolve_render_tuning(
    preset: Option<CliExecutePreset>,
    fps: Option<u32>,
    theme: Option<ThemePreset>,
    speed: Option<CliRenderSpeed>,
    keystroke_profile: Option<CliKeystrokeProfile>,
) -> RenderTuning {
    let defaults = preset.map(preset_defaults);

    RenderTuning {
        fps: fps
            .or_else(|| defaults.map(|d| d.fps))
            .unwrap_or(60)
            .max(24),
        theme: theme.or_else(|| defaults.map(|d| d.theme)),
        speed: speed
            .or_else(|| defaults.map(|d| d.speed))
            .unwrap_or(CliRenderSpeed::Quality),
        keystroke_profile: keystroke_profile
            .or_else(|| defaults.map(|d| d.keystroke_profile))
            .unwrap_or(CliKeystrokeProfile::Laptop),
    }
}

fn preset_defaults(preset: CliExecutePreset) -> PresetDefaults {
    match preset {
        CliExecutePreset::Quick => PresetDefaults {
            fps: 30,
            theme: ThemePreset::Minimal,
            speed: CliRenderSpeed::Fast,
            keystroke_profile: CliKeystrokeProfile::Laptop,
        },
        CliExecutePreset::Balanced => PresetDefaults {
            fps: 45,
            theme: ThemePreset::Clean,
            speed: CliRenderSpeed::Quality,
            keystroke_profile: CliKeystrokeProfile::Laptop,
        },
        CliExecutePreset::Polished => PresetDefaults {
            fps: 60,
            theme: ThemePreset::Clean,
            speed: CliRenderSpeed::Quality,
            keystroke_profile: CliKeystrokeProfile::Mechanical,
        },
    }
}

impl From<ValidationError> for ExecutionFailure {
    fn from(value: ValidationError) -> Self {
        Self {
            step_path: value.path,
            reason: value.message,
            record: empty_record(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        map_encoder_mode, map_output_format, merge_branding, resolve_render_tuning,
        BrandingOverrides,
    };
    use crate::branding::BrandingConfig;
    use crate::cli::{
        EncoderMode, ExecutePreset, KeystrokeProfile, OutputFormat, RenderSpeed, ThemePreset,
    };
    use crate::render::{RenderEncoderMode, RenderOutputFormat};

    #[test]
    fn merge_branding_prefers_file_and_cli_title() {
        let script = BrandingConfig {
            title: Some("Script".to_string()),
            accent: Some("#66b3ff".to_string()),
            ..BrandingConfig::default()
        };
        let file = BrandingConfig {
            title: Some("File".to_string()),
            command_text: Some("#aef".to_string()),
            ..BrandingConfig::default()
        };

        let merged = merge_branding(
            None,
            Some(script),
            Some(file),
            BrandingOverrides {
                title: Some("CLI".to_string()),
                watermark_text: Some("castkit.com".to_string()),
                avatar_x: Some("fric".to_string()),
                avatar_url: None,
                avatar_label: Some("@fric".to_string()),
            },
        )
        .expect("branding");
        assert_eq!(merged.title.as_deref(), Some("CLI"));
        assert_eq!(merged.accent.as_deref(), Some("#66b3ff"));
        assert_eq!(merged.command_text.as_deref(), Some("#aef"));
        assert_eq!(merged.watermark_text.as_deref(), Some("castkit.com"));
        assert_eq!(merged.avatar_x.as_deref(), Some("fric"));
        assert_eq!(merged.avatar_label.as_deref(), Some("@fric"));
    }

    #[test]
    fn resolve_render_tuning_uses_preset_defaults() {
        let tuning = resolve_render_tuning(Some(ExecutePreset::Quick), None, None, None, None);
        assert_eq!(tuning.fps, 30);
        assert!(matches!(tuning.theme, Some(ThemePreset::Minimal)));
        assert!(matches!(tuning.speed, RenderSpeed::Fast));
        assert!(matches!(tuning.keystroke_profile, KeystrokeProfile::Laptop));
    }

    #[test]
    fn resolve_render_tuning_prefers_explicit_args() {
        let tuning = resolve_render_tuning(
            Some(ExecutePreset::Quick),
            Some(72),
            Some(ThemePreset::Bold),
            Some(RenderSpeed::Quality),
            Some(KeystrokeProfile::Mechanical),
        );
        assert_eq!(tuning.fps, 72);
        assert!(matches!(tuning.theme, Some(ThemePreset::Bold)));
        assert!(matches!(tuning.speed, RenderSpeed::Quality));
        assert!(matches!(
            tuning.keystroke_profile,
            KeystrokeProfile::Mechanical
        ));
    }

    #[test]
    fn maps_output_format() {
        assert!(matches!(
            map_output_format(OutputFormat::Mp4),
            RenderOutputFormat::Mp4
        ));
        assert!(matches!(
            map_output_format(OutputFormat::Gif),
            RenderOutputFormat::Gif
        ));
        assert!(matches!(
            map_output_format(OutputFormat::Webm),
            RenderOutputFormat::Webm
        ));
    }

    #[test]
    fn maps_encoder_mode() {
        assert!(matches!(
            map_encoder_mode(EncoderMode::Auto),
            RenderEncoderMode::Auto
        ));
        assert!(matches!(
            map_encoder_mode(EncoderMode::Software),
            RenderEncoderMode::Software
        ));
        assert!(matches!(
            map_encoder_mode(EncoderMode::Hardware),
            RenderEncoderMode::Hardware
        ));
    }
}
