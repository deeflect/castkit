use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::branding::BrandingConfig;
use crate::execute::transcript::{ExecutionTranscript, WebActionRecord};

use super::{RenderArtifacts, RenderOptions, RenderOutputFormat, RenderSpeedPreset};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebRenderManifest {
    version: String,
    width: u32,
    height: u32,
    fps: u32,
    no_zoom: bool,
    duration_ms: u64,
    branding: Option<BrandingConfig>,
    actions: Vec<WebActionRecord>,
}

pub fn render_webstudio(transcript: &ExecutionTranscript, opts: RenderOptions) -> Result<RenderArtifacts> {
    let manifest = build_web_manifest(transcript, opts.fps.max(24), opts.no_zoom, opts.branding.clone());
    let manifest_path = std::env::temp_dir().join(format!(
        "castkit-web-render-manifest-{}.json",
        uuid::Uuid::new_v4().simple()
    ));
    fs::write(&manifest_path, serde_json::to_vec(&manifest)?)
        .with_context(|| format!("failed writing {}", manifest_path.display()))?;

    let intermediate_video_path = std::env::temp_dir().join(format!(
        "castkit-web-render-{}.mp4",
        uuid::Uuid::new_v4().simple()
    ));
    run_playwright_web_renderer(
        &manifest_path,
        &intermediate_video_path,
        manifest.fps,
        opts.speed,
        opts.verbose,
    )?;

    match opts.format {
        RenderOutputFormat::Mp4 => {
            fs::copy(&intermediate_video_path, &opts.output_path).with_context(|| {
                format!(
                    "failed copying {} to {}",
                    intermediate_video_path.display(),
                    opts.output_path.display()
                )
            })?;
        }
        RenderOutputFormat::Gif | RenderOutputFormat::Webm => {
            transcode_output(
                &intermediate_video_path,
                &opts.output_path,
                opts.format,
                opts.verbose,
            )?;
        }
    }

    Ok(RenderArtifacts {
        manifest_path,
        intermediate_video_path,
        output_path: opts.output_path,
        duration_secs: manifest.duration_ms as f32 / 1000.0,
    })
}

fn build_web_manifest(
    transcript: &ExecutionTranscript,
    fps: u32,
    no_zoom: bool,
    branding: Option<BrandingConfig>,
) -> WebRenderManifest {
    let mut actions = transcript.web_actions.clone();
    actions.sort_by_key(|action| action.t_ms);
    let duration_ms = actions
        .iter()
        .map(|action| action.t_ms.saturating_add(action.duration_ms).saturating_add(900))
        .max()
        .unwrap_or(3_500)
        .max(3_500);

    WebRenderManifest {
        version: "1".to_string(),
        width: WIDTH,
        height: HEIGHT,
        fps,
        no_zoom,
        duration_ms,
        branding,
        actions,
    }
}

pub fn build_web_manifest_preview(transcript: &ExecutionTranscript, fps: u32, no_zoom: bool) -> serde_json::Value {
    serde_json::to_value(build_web_manifest(transcript, fps.max(24), no_zoom, None))
        .unwrap_or_else(|_| json!({ "actions": [] }))
}

fn run_playwright_web_renderer(
    manifest_path: &Path,
    output_path: &Path,
    fps: u32,
    speed: RenderSpeedPreset,
    verbose: bool,
) -> Result<()> {
    let renderer_home = resolve_renderer_home()?;
    let renderer_script = renderer_home.join("render-web.mjs");
    if !renderer_script.exists() {
        anyhow::bail!(
            "web renderer script not found at {} (run setup first)",
            renderer_script.display()
        );
    }

    let speed_arg = render_speed_arg(speed);
    let mut command = Command::new("node");
    command
        .arg(&renderer_script)
        .arg("--manifest")
        .arg(manifest_path)
        .arg("--output")
        .arg(output_path)
        .arg("--fps")
        .arg(fps.to_string())
        .arg("--speed")
        .arg(speed_arg);

    if verbose {
        eprintln!(
            "[castkit] web renderer: node {} --manifest {} --output {} --fps {} --speed {}",
            renderer_script.display(),
            manifest_path.display(),
            output_path.display(),
            fps,
            speed_arg
        );
    }

    let output = command.output().context("failed to run web node renderer")?;
    if !output.status.success() {
        anyhow::bail!(
            "web node renderer failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    if !output_path.exists() {
        anyhow::bail!(
            "web renderer completed but output video missing: {}",
            output_path.display()
        );
    }
    Ok(())
}

fn resolve_renderer_home() -> Result<PathBuf> {
    if let Ok(home) = std::env::var("CASTKIT_RENDERER_HOME") {
        return Ok(PathBuf::from(home));
    }
    let candidate = std::env::current_dir()?.join("renderer-runtime");
    if candidate.join("render-web.mjs").exists() {
        return Ok(candidate);
    }
    anyhow::bail!("no renderer home found; expected ./renderer-runtime with render-web.mjs")
}

fn render_speed_arg(speed: RenderSpeedPreset) -> &'static str {
    match speed {
        RenderSpeedPreset::Fast => "fast",
        RenderSpeedPreset::Quality => "quality",
    }
}

fn transcode_output(
    input_mp4: &Path,
    output_path: &Path,
    format: RenderOutputFormat,
    verbose: bool,
) -> Result<()> {
    let ffmpeg = which::which("ffmpeg").context("ffmpeg not found in PATH")?;
    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_mp4.to_string_lossy().to_string(),
    ];

    match format {
        RenderOutputFormat::Mp4 => {
            args.extend([
                "-c:v".to_string(),
                "copy".to_string(),
                "-an".to_string(),
            ]);
        }
        RenderOutputFormat::Webm => {
            args.extend([
                "-c:v".to_string(),
                "libvpx-vp9".to_string(),
                "-b:v".to_string(),
                "0".to_string(),
                "-crf".to_string(),
                "32".to_string(),
                "-row-mt".to_string(),
                "1".to_string(),
                "-deadline".to_string(),
                "good".to_string(),
                "-cpu-used".to_string(),
                "1".to_string(),
                "-pix_fmt".to_string(),
                "yuv420p".to_string(),
                "-an".to_string(),
            ]);
        }
        RenderOutputFormat::Gif => {
            args.extend([
                "-an".to_string(),
                "-vf".to_string(),
                "fps=15,scale=1280:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse".to_string(),
            ]);
        }
    }

    args.push(output_path.to_string_lossy().to_string());

    if verbose {
        eprintln!("[castkit] web transcode: ffmpeg {}", args.join(" "));
    }

    let output = Command::new(ffmpeg)
        .args(&args)
        .output()
        .context("failed to run ffmpeg transcode")?;

    if !output.status.success() {
        anyhow::bail!(
            "web ffmpeg transcode failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::build_web_manifest_preview;
    use crate::execute::transcript::ExecutionTranscript;

    #[test]
    fn web_manifest_preview_has_core_fields() {
        let transcript = ExecutionTranscript {
            session_id: "sess_web".to_string(),
            started_at: chrono::Utc::now(),
            mode: crate::script::DemoMode::Web,
            setup: vec![],
            checks: vec![],
            scenes: vec![],
            cleanup: vec![],
            overlay_events: vec![],
            web_actions: vec![],
        };
        let value = build_web_manifest_preview(&transcript, 60, false);
        assert_eq!(value["version"], "1");
        assert_eq!(value["fps"], 60);
        assert!(value["actions"].is_array());
    }
}
