use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::branding::BrandingConfig;
use crate::execute::transcript::{ExecutionTranscript, OverlayEvent, StepRunRecord};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const OUTPUT_WRAP_WIDTH: usize = 110;
const OUTPUT_PAGE_LINES: usize = 30;
const OUTPUT_SNAPSHOT_STEP: usize = 2;
const MANIFEST_WARN_BYTES: usize = 20 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub enum RenderSpeedPreset {
    Fast,
    Quality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderOutputFormat {
    Mp4,
    Gif,
    Webm,
}

#[derive(Debug, Clone, Copy)]
pub enum KeystrokeProfile {
    Mechanical,
    Laptop,
    Silent,
}

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub output_path: PathBuf,
    pub format: RenderOutputFormat,
    pub fps: u32,
    pub no_zoom: bool,
    pub typing_sound: bool,
    pub music_path: Option<PathBuf>,
    pub branding: Option<BrandingConfig>,
    pub speed: RenderSpeedPreset,
    pub keystroke_profile: KeystrokeProfile,
    pub avatar_cache_dir: Option<PathBuf>,
    pub verbose: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RenderArtifacts {
    pub manifest_path: PathBuf,
    pub intermediate_video_path: PathBuf,
    pub output_path: PathBuf,
    pub duration_secs: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SnapshotPhase {
    Typing,
    Output,
    Idle,
}

#[derive(Debug, Clone, Copy)]
enum KeyKind {
    Alpha,
    Digit,
    Space,
    Punctuation,
    Symbol,
}

#[derive(Debug, Clone, Copy)]
struct KeyStroke {
    t_secs: f32,
    kind: KeyKind,
    seed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RenderSnapshot {
    t_ms: u64,
    lines: Vec<String>,
    active_row: usize,
    cursor_col: usize,
    phase: SnapshotPhase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SceneCue {
    t_ms: u64,
    title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RenderManifest {
    version: String,
    width: u32,
    height: u32,
    fps: u32,
    no_zoom: bool,
    duration_ms: u64,
    line_height: u32,
    branding: Option<BrandingConfig>,
    scene_cues: Vec<SceneCue>,
    snapshots: Vec<RenderSnapshot>,
    overlay_events: Vec<OverlayEvent>,
}

pub fn render_screenstudio(
    transcript: &ExecutionTranscript,
    opts: RenderOptions,
) -> Result<RenderArtifacts> {
    let (snapshots, scene_cues, keystrokes, duration_secs, step_anchors) =
        build_timeline(transcript);
    let overlay_events = build_overlay_events(&transcript.overlay_events, &step_anchors);
    let duration_secs = duration_secs.max(max_overlay_end_secs(&overlay_events));
    let fps = opts.fps.max(24);

    let manifest = RenderManifest {
        version: "1".to_string(),
        width: WIDTH,
        height: HEIGHT,
        fps,
        no_zoom: opts.no_zoom,
        duration_ms: ((duration_secs * 1000.0).ceil() as u64).max(1000),
        line_height: 24,
        branding: opts.branding,
        scene_cues,
        snapshots,
        overlay_events,
    };

    let manifest_path = std::env::temp_dir().join(format!(
        "castkit-render-manifest-{}.json",
        uuid::Uuid::new_v4().simple()
    ));
    let manifest_bytes = serde_json::to_vec(&manifest)?;
    if manifest_bytes.len() >= MANIFEST_WARN_BYTES {
        eprintln!(
            "[castkit] warning: large render manifest ({:.1} MB, {} snapshots). Rendering may be slower.",
            manifest_bytes.len() as f64 / (1024.0 * 1024.0),
            manifest.snapshots.len()
        );
    }
    fs::write(&manifest_path, &manifest_bytes)
        .with_context(|| format!("failed writing {}", manifest_path.display()))?;

    let intermediate_video_path = std::env::temp_dir().join(format!(
        "castkit-render-{}.mp4",
        uuid::Uuid::new_v4().simple()
    ));

    run_playwright_renderer(
        &manifest_path,
        &intermediate_video_path,
        fps,
        opts.speed,
        opts.avatar_cache_dir.as_deref(),
        opts.verbose,
    )?;

    let typing_audio_path = if opts.typing_sound {
        Some(write_typing_audio(
            &keystrokes,
            manifest.duration_ms as f32 / 1000.0,
            opts.keystroke_profile,
        )?)
    } else {
        None
    };

    let muxed_output_path = match opts.format {
        RenderOutputFormat::Mp4 => opts.output_path.clone(),
        RenderOutputFormat::Gif | RenderOutputFormat::Webm => std::env::temp_dir().join(format!(
            "castkit-muxed-{}.mp4",
            uuid::Uuid::new_v4().simple()
        )),
    };

    mux_to_final_output(
        &intermediate_video_path,
        typing_audio_path.as_deref(),
        opts.music_path.as_deref(),
        &muxed_output_path,
        opts.verbose,
    )?;

    if opts.format != RenderOutputFormat::Mp4 {
        transcode_output(
            &muxed_output_path,
            &opts.output_path,
            opts.format,
            opts.verbose,
        )?;
    }

    Ok(RenderArtifacts {
        manifest_path,
        intermediate_video_path,
        output_path: opts.output_path,
        duration_secs: manifest.duration_ms as f32 / 1000.0,
    })
}

fn build_overlay_events(
    events: &[OverlayEvent],
    step_anchors: &BTreeMap<String, u64>,
) -> Vec<OverlayEvent> {
    let mut out = events.to_vec();
    for event in &mut out {
        if let Some(anchor_ms) = step_anchors.get(&event.step_id) {
            // Render overlays when the corresponding step appears in the timeline, not when
            // command execution happened on wall clock.
            event.t_ms = *anchor_ms;
        }
    }
    out.sort_by_key(|event| event.t_ms);
    out
}

fn max_overlay_end_secs(events: &[OverlayEvent]) -> f32 {
    let max_end_ms = events
        .iter()
        .map(|event| event.t_ms.saturating_add(event.show_ms))
        .max()
        .unwrap_or(0);
    (max_end_ms as f32 / 1000.0) + 0.45
}

fn run_playwright_renderer(
    manifest_path: &Path,
    output_path: &Path,
    fps: u32,
    speed: RenderSpeedPreset,
    avatar_cache_dir: Option<&Path>,
    verbose: bool,
) -> Result<()> {
    let renderer_home = resolve_renderer_home()?;
    let renderer_script = renderer_home.join("render.mjs");
    if !renderer_script.exists() {
        anyhow::bail!(
            "renderer script not found at {} (run setup first)",
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
    if let Some(dir) = avatar_cache_dir {
        command.arg("--avatar-cache-dir").arg(dir);
    }

    if verbose {
        eprintln!(
            "[castkit] renderer: node {} --manifest {} --output {} --fps {} --speed {}",
            renderer_script.display(),
            manifest_path.display(),
            output_path.display(),
            fps,
            speed_arg
        );
    }

    let output = command.output().context("failed to run node renderer")?;

    if !output.status.success() {
        anyhow::bail!(
            "node renderer failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    if !output_path.exists() {
        anyhow::bail!(
            "renderer completed but output video missing: {}",
            output_path.display()
        );
    }

    if verbose {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.trim().is_empty() {
            eprintln!("[castkit] renderer output: {}", stdout.trim());
        }
    }

    Ok(())
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

fn render_speed_arg(speed: RenderSpeedPreset) -> &'static str {
    match speed {
        RenderSpeedPreset::Fast => "fast",
        RenderSpeedPreset::Quality => "quality",
    }
}

fn mux_to_final_output(
    video_path: &Path,
    typing_audio: Option<&Path>,
    music_audio: Option<&Path>,
    output_path: &Path,
    verbose: bool,
) -> Result<()> {
    let ffmpeg = which::which("ffmpeg").context("ffmpeg not found in PATH")?;

    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        video_path.to_string_lossy().to_string(),
    ];

    if let Some(path) = typing_audio {
        args.push("-i".to_string());
        args.push(path.to_string_lossy().to_string());
    }

    if let Some(path) = music_audio {
        args.push("-stream_loop".to_string());
        args.push("-1".to_string());
        args.push("-i".to_string());
        args.push(path.to_string_lossy().to_string());
    }

    match (typing_audio.is_some(), music_audio.is_some()) {
        (false, false) => {
            args.extend([
                "-map".to_string(),
                "0:v:0".to_string(),
                "-c:v".to_string(),
                "copy".to_string(),
                "-an".to_string(),
            ]);
        }
        (true, false) => {
            args.extend([
                "-filter_complex".to_string(),
                "[1:a]highpass=f=120,lowpass=f=4200,acompressor=threshold=-24dB:ratio=3:attack=5:release=80,volume=0.28,alimiter=limit=0.92[aout]".to_string(),
                "-map".to_string(),
                "0:v:0".to_string(),
                "-map".to_string(),
                "[aout]".to_string(),
                "-c:v".to_string(),
                "copy".to_string(),
            ]);
        }
        (false, true) => {
            args.extend([
                "-filter_complex".to_string(),
                "[1:a]volume=0.15,alimiter[aout]".to_string(),
                "-map".to_string(),
                "0:v:0".to_string(),
                "-map".to_string(),
                "[aout]".to_string(),
                "-c:v".to_string(),
                "copy".to_string(),
            ]);
        }
        (true, true) => {
            args.extend([
                "-filter_complex".to_string(),
                "[2:a]volume=0.13[music];[1:a]highpass=f=120,lowpass=f=4200,acompressor=threshold=-24dB:ratio=3:attack=5:release=80,volume=0.30[typing];[music][typing]sidechaincompress=threshold=0.02:ratio=6:attack=10:release=220[ducked];[ducked][typing]amix=inputs=2:duration=shortest,alimiter=limit=0.92[aout]".to_string(),
                "-map".to_string(),
                "0:v:0".to_string(),
                "-map".to_string(),
                "[aout]".to_string(),
                "-c:v".to_string(),
                "copy".to_string(),
            ]);
        }
    }

    if typing_audio.is_some() || music_audio.is_some() {
        args.extend([
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            "160k".to_string(),
            "-shortest".to_string(),
        ]);
    }

    args.extend(["-movflags".to_string(), "+faststart".to_string()]);
    args.push(output_path.to_string_lossy().to_string());

    if verbose {
        eprintln!("[castkit] mux: ffmpeg {}", args.join(" "));
    }

    let output = Command::new(ffmpeg)
        .args(&args)
        .output()
        .context("failed to run ffmpeg mux")?;

    if !output.status.success() {
        anyhow::bail!(
            "ffmpeg mux failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
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
                "-c:a".to_string(),
                "copy".to_string(),
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
                "-c:a".to_string(),
                "libopus".to_string(),
                "-b:a".to_string(),
                "96k".to_string(),
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
        eprintln!("[castkit] transcode: ffmpeg {}", args.join(" "));
    }

    let output = Command::new(ffmpeg)
        .args(&args)
        .output()
        .context("failed to run ffmpeg transcode")?;

    if !output.status.success() {
        anyhow::bail!(
            "ffmpeg transcode failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

fn build_timeline(
    transcript: &ExecutionTranscript,
) -> (
    Vec<RenderSnapshot>,
    Vec<SceneCue>,
    Vec<KeyStroke>,
    f32,
    BTreeMap<String, u64>,
) {
    let mut snapshots = Vec::new();
    let mut scene_cues = Vec::new();
    let mut lines = Vec::<String>::new();
    let mut t = 0.0f32;
    let mut keystrokes = Vec::new();
    let mut step_anchors = BTreeMap::new();

    push_snapshot(&mut snapshots, t, &lines, 0, 0, SnapshotPhase::Idle);

    append_group_steps(
        &mut snapshots,
        &mut lines,
        &mut keystrokes,
        &mut t,
        "Setup",
        &transcript.setup,
        &mut step_anchors,
    );

    for scene in &transcript.scenes {
        lines.push(format!("# {}", scene.title));
        t += 0.25;
        scene_cues.push(SceneCue {
            t_ms: (t * 1000.0).round() as u64,
            title: scene.title.clone(),
        });
        push_snapshot(
            &mut snapshots,
            t,
            &lines,
            lines.len().saturating_sub(1),
            0,
            SnapshotPhase::Idle,
        );

        for step in &scene.steps {
            type_command(
                &mut snapshots,
                &mut lines,
                &mut keystrokes,
                &mut t,
                &step.run,
            );
            stream_output(&mut snapshots, &mut lines, &mut t, step);
            step_anchors.insert(step.id.clone(), (t * 1000.0).round() as u64);
            t += 0.20;
            push_snapshot(
                &mut snapshots,
                t,
                &lines,
                lines.len().saturating_sub(1),
                0,
                SnapshotPhase::Idle,
            );
        }

        lines.push(String::new());
        t += 0.14;
        push_snapshot(
            &mut snapshots,
            t,
            &lines,
            lines.len().saturating_sub(1),
            0,
            SnapshotPhase::Idle,
        );
    }

    append_group_steps(
        &mut snapshots,
        &mut lines,
        &mut keystrokes,
        &mut t,
        "Checks",
        &transcript.checks,
        &mut step_anchors,
    );
    append_group_steps(
        &mut snapshots,
        &mut lines,
        &mut keystrokes,
        &mut t,
        "Cleanup",
        &transcript.cleanup,
        &mut step_anchors,
    );

    let duration = (t + 0.8).max(4.0);
    (snapshots, scene_cues, keystrokes, duration, step_anchors)
}

fn append_group_steps(
    snapshots: &mut Vec<RenderSnapshot>,
    lines: &mut Vec<String>,
    keystrokes: &mut Vec<KeyStroke>,
    t: &mut f32,
    label: &str,
    steps: &[StepRunRecord],
    step_anchors: &mut BTreeMap<String, u64>,
) {
    if steps.is_empty() {
        return;
    }

    lines.push(format!("# {}", label));
    *t += 0.20;
    push_snapshot(
        snapshots,
        *t,
        lines,
        lines.len().saturating_sub(1),
        0,
        SnapshotPhase::Idle,
    );

    for step in steps {
        type_command(snapshots, lines, keystrokes, t, &step.run);
        stream_output(snapshots, lines, t, step);
        step_anchors.insert(step.id.clone(), (*t * 1000.0).round() as u64);
        *t += 0.12;
        push_snapshot(
            snapshots,
            *t,
            lines,
            lines.len().saturating_sub(1),
            0,
            SnapshotPhase::Idle,
        );
    }

    lines.push(String::new());
    *t += 0.1;
    push_snapshot(
        snapshots,
        *t,
        lines,
        lines.len().saturating_sub(1),
        0,
        SnapshotPhase::Idle,
    );
}

fn type_command(
    snapshots: &mut Vec<RenderSnapshot>,
    lines: &mut Vec<String>,
    keystrokes: &mut Vec<KeyStroke>,
    t: &mut f32,
    run: &str,
) {
    lines.push("$ ".to_string());
    let row = lines.len().saturating_sub(1);
    *t += 0.08 + ((run.len() % 7) as f32 * 0.008);
    push_snapshot(
        snapshots,
        *t,
        lines,
        row,
        lines[row].chars().count(),
        SnapshotPhase::Idle,
    );

    for (idx, ch) in run.chars().enumerate() {
        lines[row].push(ch);
        *t += typing_delay(ch, idx);
        keystrokes.push(KeyStroke {
            t_secs: *t,
            kind: classify_key(ch),
            seed: (row as u32)
                .wrapping_mul(2654435761)
                .wrapping_add(idx as u32),
        });
        push_snapshot(
            snapshots,
            *t,
            lines,
            row,
            lines[row].chars().count(),
            SnapshotPhase::Typing,
        );
    }

    *t += 0.05;
    push_snapshot(
        snapshots,
        *t,
        lines,
        row,
        lines[row].chars().count(),
        SnapshotPhase::Idle,
    );
}

fn stream_output(
    snapshots: &mut Vec<RenderSnapshot>,
    lines: &mut Vec<String>,
    t: &mut f32,
    step: &StepRunRecord,
) {
    let out = collect_wrapped_output(step);

    if out.is_empty() {
        return;
    }

    let total_pages = out.len().div_ceil(OUTPUT_PAGE_LINES).max(1);

    for (line_idx, line) in out.iter().enumerate() {
        if total_pages > 1 && line_idx % OUTPUT_PAGE_LINES == 0 {
            let page = line_idx / OUTPUT_PAGE_LINES + 1;
            lines.push(format!("-- page {page}/{total_pages} --"));
            let marker_row = lines.len().saturating_sub(1);
            *t += 0.05;
            push_snapshot(
                snapshots,
                *t,
                lines,
                marker_row,
                lines[marker_row].chars().count(),
                SnapshotPhase::Idle,
            );
        }

        lines.push(String::new());
        let row = lines.len().saturating_sub(1);
        let char_count = line.chars().count();

        for (idx, ch) in line.chars().enumerate() {
            lines[row].push(ch);
            *t += output_char_delay(ch, idx);
            let at_boundary = idx + 1 == char_count
                || idx == 0
                || idx % OUTPUT_SNAPSHOT_STEP == 0
                || matches!(ch, ' ' | '/' | '\\' | ':' | '=' | '-');
            if at_boundary {
                push_snapshot(
                    snapshots,
                    *t,
                    lines,
                    row,
                    lines[row].chars().count(),
                    SnapshotPhase::Output,
                );
            }
        }

        *t += 0.012;
        push_snapshot(
            snapshots,
            *t,
            lines,
            row,
            lines[row].chars().count(),
            SnapshotPhase::Output,
        );

        if total_pages > 1 && (line_idx + 1) % OUTPUT_PAGE_LINES == 0 && (line_idx + 1) < out.len()
        {
            *t += 0.14;
            push_snapshot(
                snapshots,
                *t,
                lines,
                row,
                lines[row].chars().count(),
                SnapshotPhase::Idle,
            );
        }
    }
}

fn collect_wrapped_output(step: &StepRunRecord) -> Vec<String> {
    step.stdout
        .lines()
        .chain(step.stderr.lines())
        .flat_map(|line| wrap_line(line, OUTPUT_WRAP_WIDTH))
        .collect()
}

fn wrap_line(line: &str, width: usize) -> Vec<String> {
    if line.chars().count() <= width {
        return vec![line.to_string()];
    }

    let mut out = Vec::new();
    let mut current = String::new();
    for ch in line.chars() {
        current.push(ch);
        if current.chars().count() >= width {
            out.push(current.clone());
            current.clear();
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn push_snapshot(
    snapshots: &mut Vec<RenderSnapshot>,
    t: f32,
    lines: &[String],
    active_row: usize,
    cursor_col: usize,
    phase: SnapshotPhase,
) {
    snapshots.push(RenderSnapshot {
        t_ms: (t * 1000.0).round() as u64,
        lines: lines.to_vec(),
        active_row,
        cursor_col,
        phase,
    });
}

fn typing_delay(ch: char, index: usize) -> f32 {
    let base = match classify_key(ch) {
        KeyKind::Space => 0.048,
        KeyKind::Punctuation => 0.038,
        KeyKind::Symbol => 0.033,
        KeyKind::Digit => 0.030,
        KeyKind::Alpha => 0.026,
    };
    let jitter = pseudo_noise((index as u32).wrapping_mul(31).wrapping_add(ch as u32)) * 0.008;
    (base + jitter).clamp(0.014, 0.075)
}

fn output_char_delay(ch: char, index: usize) -> f32 {
    let base = if ch == ' ' { 0.0014 } else { 0.0026 };
    let jitter = pseudo_noise((index as u32).wrapping_mul(17).wrapping_add(ch as u32)) * 0.0009;
    (base + jitter).clamp(0.0008, 0.0048)
}

fn write_typing_audio(
    keystrokes: &[KeyStroke],
    duration_secs: f32,
    profile: KeystrokeProfile,
) -> Result<PathBuf> {
    let sample_rate = 44_100u32;
    let total_samples = ((duration_secs + 0.1) * sample_rate as f32).ceil() as usize;
    let mut samples = vec![0.0f32; total_samples];
    let sample_bank = build_sample_bank(profile, sample_rate);

    for (event_idx, key) in keystrokes.iter().enumerate() {
        let start = (key.t_secs * sample_rate as f32) as usize;
        let bank_len = sample_bank.len().max(1);
        let sample_idx = (key.seed as usize + event_idx + key_kind_index(key.kind)) % bank_len;
        let hit = &sample_bank[sample_idx];
        let amp = profile_gain(profile)
            * key_gain(key.kind)
            * (0.84 + 0.32 * ((pseudo_noise(key.seed.wrapping_add(91)) + 1.0) * 0.5));

        for (i, hit_sample) in hit.iter().enumerate() {
            let idx = start + i;
            if idx >= samples.len() {
                break;
            }
            samples[idx] += *hit_sample * amp;
        }

        // Mechanical profile gets occasional key bounce to avoid robotic rhythm.
        if matches!(profile, KeystrokeProfile::Mechanical)
            && (event_idx + key.seed as usize) % 3 == 0
        {
            let offset = (0.0010 * sample_rate as f32) as usize;
            let secondary_start = start.saturating_add(offset + ((key.seed as usize % 3) * 6));
            for i in 0..(hit.len() / 2).max(4) {
                let idx = secondary_start + i;
                if idx >= samples.len() {
                    break;
                }
                let env = (1.0 - (i as f32 / (hit.len() / 2).max(4) as f32)).powf(2.0);
                samples[idx] += hit[i] * env * 0.22;
            }
        }
    }

    let path = std::env::temp_dir().join(format!(
        "castkit-typing-{}.wav",
        uuid::Uuid::new_v4().simple()
    ));

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec)
        .with_context(|| format!("failed creating {}", path.display()))?;

    for s in samples {
        writer.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
    }
    writer.finalize()?;

    Ok(path)
}

fn build_sample_bank(profile: KeystrokeProfile, sample_rate: u32) -> Vec<Vec<f32>> {
    let (freqs, len_ms, noise, harmonics) = match profile {
        KeystrokeProfile::Mechanical => (
            vec![165.0, 182.0, 205.0, 228.0, 252.0, 281.0, 318.0],
            11.0,
            0.030,
            0.22,
        ),
        KeystrokeProfile::Laptop => (
            vec![280.0, 320.0, 355.0, 398.0, 442.0, 486.0],
            7.2,
            0.042,
            0.16,
        ),
        KeystrokeProfile::Silent => (vec![320.0], 4.0, 0.0, 0.0),
    };

    freqs
        .into_iter()
        .enumerate()
        .map(|(idx, f)| synth_click_sample(sample_rate, f, len_ms, noise, harmonics, idx as u32))
        .collect()
}

fn synth_click_sample(
    sample_rate: u32,
    base_freq: f32,
    len_ms: f32,
    noise_amount: f32,
    harmonics: f32,
    seed: u32,
) -> Vec<f32> {
    let click_len = ((len_ms / 1000.0) * sample_rate as f32).round().max(4.0) as usize;
    let mut out = vec![0.0f32; click_len];
    let phase = ((pseudo_noise(seed.wrapping_mul(17)) + 1.0) * 0.5) * std::f32::consts::PI;
    for (i, slot) in out.iter_mut().enumerate() {
        let t = i as f32 / sample_rate as f32;
        let env = (1.0 - (i as f32 / click_len as f32)).powf(1.95);
        let fundamental = (t * 2.0 * std::f32::consts::PI * base_freq + phase).sin() * 0.66;
        let overtone = (t * 2.0 * std::f32::consts::PI * (base_freq * 2.12)).sin() * harmonics;
        let transient = (t * 2.0 * std::f32::consts::PI * (base_freq * 4.4)).sin() * 0.05;
        let noise = pseudo_noise(seed ^ i as u32).powi(3) * noise_amount;
        *slot = (fundamental + overtone + transient + noise) * env;
    }
    out
}

fn key_kind_index(kind: KeyKind) -> usize {
    match kind {
        KeyKind::Alpha => 0,
        KeyKind::Digit => 1,
        KeyKind::Space => 2,
        KeyKind::Punctuation => 3,
        KeyKind::Symbol => 4,
    }
}

fn key_gain(kind: KeyKind) -> f32 {
    match kind {
        KeyKind::Alpha => 0.98,
        KeyKind::Digit => 1.03,
        KeyKind::Space => 0.78,
        KeyKind::Punctuation => 1.08,
        KeyKind::Symbol => 1.10,
    }
}

fn profile_gain(profile: KeystrokeProfile) -> f32 {
    match profile {
        KeystrokeProfile::Mechanical => 0.42,
        KeystrokeProfile::Laptop => 0.30,
        KeystrokeProfile::Silent => 0.0,
    }
}

fn pseudo_noise(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x45d9f3b);
    x = (x ^ (x >> 16)).wrapping_mul(0x45d9f3b);
    x ^= x >> 16;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn classify_key(ch: char) -> KeyKind {
    if ch.is_ascii_alphabetic() {
        KeyKind::Alpha
    } else if ch.is_ascii_digit() {
        KeyKind::Digit
    } else if ch == ' ' {
        KeyKind::Space
    } else if matches!(ch, '.' | ',' | ':' | ';' | '!' | '?') {
        KeyKind::Punctuation
    } else {
        KeyKind::Symbol
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_long_lines() {
        let in_line = "a".repeat(250);
        let wrapped = wrap_line(&in_line, 80);
        assert!(!wrapped.is_empty());
        assert!(wrapped.iter().all(|line| line.chars().count() <= 80));
    }

    #[test]
    fn typing_audio_writes_file() {
        let keys = vec![
            KeyStroke {
                t_secs: 0.05,
                kind: KeyKind::Alpha,
                seed: 1,
            },
            KeyStroke {
                t_secs: 0.1,
                kind: KeyKind::Space,
                seed: 2,
            },
            KeyStroke {
                t_secs: 0.2,
                kind: KeyKind::Symbol,
                seed: 3,
            },
        ];
        let path = write_typing_audio(&keys, 1.2, KeystrokeProfile::Laptop).expect("audio");
        assert!(path.exists());
        let metadata = fs::metadata(path).expect("stat");
        assert!(metadata.len() > 0);
    }

    #[test]
    fn timeline_has_snapshots() {
        let transcript = ExecutionTranscript {
            session_id: "sess_1".to_string(),
            started_at: chrono::Utc::now(),
            mode: crate::script::DemoMode::Terminal,
            setup: vec![],
            checks: vec![],
            cleanup: vec![],
            scenes: vec![],
            overlay_events: vec![],
            web_actions: vec![],
        };
        let (snapshots, _, _, duration, _) = build_timeline(&transcript);
        assert!(!snapshots.is_empty());
        assert!(duration >= 4.0);
    }

    #[test]
    fn paginates_output_lines_without_truncating_content() {
        let mut out = Vec::new();
        for i in 0..(OUTPUT_PAGE_LINES + 7) {
            out.push(format!("line {i}"));
        }
        let step = StepRunRecord {
            id: "x".to_string(),
            run: "echo x".to_string(),
            stdout: out.join("\n"),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: 1,
            status: "ok".to_string(),
            error: None,
        };
        let mut snapshots = Vec::new();
        let mut lines = Vec::new();
        let mut t = 0.0;
        stream_output(&mut snapshots, &mut lines, &mut t, &step);
        assert!(lines.iter().any(|line| line == "-- page 1/2 --"));
        assert!(lines.iter().any(|line| line == "-- page 2/2 --"));
        assert!(lines.iter().any(|line| line == "line 0"));
        assert!(lines
            .iter()
            .any(|line| line == &format!("line {}", OUTPUT_PAGE_LINES + 6)));
    }

    #[test]
    fn overlay_events_are_sorted_for_manifest() {
        let events = vec![
            OverlayEvent {
                t_ms: 800,
                step_id: "b".to_string(),
                artifact_type: crate::execute::transcript::OverlayArtifactType::ResultCard,
                title: None,
                image_path: None,
                result_items: vec![],
                position: crate::script::ArtifactPosition::TopRight,
                show_ms: 1200,
                enter: crate::script::ArtifactEnter::Fade,
            },
            OverlayEvent {
                t_ms: 300,
                step_id: "a".to_string(),
                artifact_type: crate::execute::transcript::OverlayArtifactType::Image,
                title: None,
                image_path: Some("/tmp/x.png".to_string()),
                result_items: vec![],
                position: crate::script::ArtifactPosition::TopLeft,
                show_ms: 1200,
                enter: crate::script::ArtifactEnter::Slide,
            },
        ];
        let sorted = build_overlay_events(&events, &BTreeMap::new());
        assert_eq!(sorted[0].step_id, "a");
        assert_eq!(sorted[1].step_id, "b");
    }

    #[test]
    fn overlay_events_follow_step_anchor_timing() {
        let events = vec![OverlayEvent {
            t_ms: 25,
            step_id: "step_01".to_string(),
            artifact_type: crate::execute::transcript::OverlayArtifactType::ResultCard,
            title: None,
            image_path: None,
            result_items: vec![],
            position: crate::script::ArtifactPosition::TopRight,
            show_ms: 1200,
            enter: crate::script::ArtifactEnter::Fade,
        }];
        let mut anchors = BTreeMap::new();
        anchors.insert("step_01".to_string(), 1830);
        let sorted = build_overlay_events(&events, &anchors);
        assert_eq!(sorted[0].t_ms, 1830);
    }

    #[test]
    fn overlay_end_extends_duration() {
        let events = vec![OverlayEvent {
            t_ms: 4_800,
            step_id: "a".to_string(),
            artifact_type: crate::execute::transcript::OverlayArtifactType::Image,
            title: None,
            image_path: Some("/tmp/x.png".to_string()),
            result_items: vec![],
            position: crate::script::ArtifactPosition::TopLeft,
            show_ms: 2_000,
            enter: crate::script::ArtifactEnter::Fade,
        }];
        assert!(max_overlay_end_secs(&events) >= 6.8);
    }
}
