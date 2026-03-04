---
title: "castkit — Full Technical Specification"
created: 2026-03-03
type: project
status: active
---

# castkit — Technical Specification v1

**One command. Polished demo video. No manual recording.**

Open-source Rust CLI that generates Screen Studio-quality demo videos of CLI tools. Point it at a binary, get a branded MP4.

```bash
castkit ./my-cli
```

---

## Table of Contents

1. [Overview](#overview)
2. [CLI Interface](#cli-interface)
3. [Pipeline](#pipeline)
4. [Module Specs](#module-specs)
   - [Discovery](#1-discovery)
   - [Planning](#2-planning)
   - [Recording](#3-recording)
   - [Redaction](#4-redaction)
   - [Rendering](#5-rendering)
   - [Encoding](#6-encoding)
5. [Data Types](#data-types)
6. [Animation System](#animation-system)
7. [Theme System](#theme-system)
8. [Branding](#branding)
9. [Configuration](#configuration)
10. [Project Structure](#project-structure)
11. [Dependencies](#dependencies)
12. [Build & Distribution](#build--distribution)
13. [v1 Scope](#v1-scope)

---

## Overview

### Core Principles

- **Zero context overhead** — one command, no video production knowledge needed
- **Alive, not static** — human typing cadence, auto-zoom, smooth easing on everything
- **Safe by default** — auto-redacts secrets before rendering
- **Agent-native** — any coding agent can call it. also works for humans
- **Self-contained** — single binary + ffmpeg. no Node, no Docker, no browser runtime

### Pipeline Summary

```
Binary/Command → Discover → Plan → Record → Redact → Render → Encode → MP4/GIF
```

Each stage is independent. You can intervene at any stage (edit script/plan, skip discovery, provide pre-recorded frames).

---

## CLI Interface

Primary mode is agent-native. The external API is a strict handoff/validate/execute contract.

```
castkit <subcommand> [options]

SUBCOMMANDS:
  castkit handoff init <target>             Build discovery session + refs index
  castkit handoff list --session <id>       List indexed refs by source, paginated
  castkit handoff get --session <id> --ref  Fetch exact content for one ref
  castkit validate --session <id> --script  Validate agent-authored DemoScript
  castkit execute --session <id> --script   Execute validated script and render output

COMPATIBILITY SUBCOMMANDS (advanced/manual):
  castkit discover <target>                 Internal stage access
  castkit record <plan_or_script>           Internal stage access
  castkit render <recording>                Internal stage access
  castkit run <target>                      Single-command convenience path

GLOBAL OPTIONS:
  -o, --output <path>           Output file (default: ./demo.mp4)
  -f, --format <fmt>            mp4 | gif | webm | png-sequence
  -q, --quality <level>         draft (720p) | standard (1080p) | high (4K)
  -s, --style <name>            dark | light | minimal | hacker | ocean
      --theme <name>            Terminal color theme (catppuccin, tokyo-night, dracula, one-dark)
      --font <name>             Monospace font (jetbrains-mono, fira-code, sf-mono, cascadia)
  -v, --verbose                 Show pipeline progress
      --json                    Machine-readable output where applicable

HANDOFF OPTIONS:
      --source <name>           help | readme | files | probes
      --page <n>                Page number for list endpoints
      --per-page <n>            Items per page for list endpoints
      --readme <path>           Explicit README path
      --no-readme               Skip README parsing

VALIDATE/EXECUTE OPTIONS:
      --non-interactive         Required for agent workflow
      --sandbox                 Run in temp dir (default: true)
      --shell <path>            Shell to use (default: $SHELL or /bin/bash)
      --cols <n>                Terminal columns (default: 100)
      --rows <n>                Terminal rows (default: 30)
      --timeout <secs>          Max recording time per scene (default: 30)
      --fps <n>                 Frame rate (default: 60)
      --no-zoom                 Disable auto-zoom
      --zoom-intensity <f>      Zoom factor 1.0-2.0 (default: 1.3)
      --no-brand                Skip intro/outro
      --redact-config <path>    Custom redaction rules
      --redact-extra <pattern>  Additional regex pattern to redact

EXAMPLES:
  castkit handoff init ./target/release/mycli --json
  castkit handoff list --session sess_123 --source readme --page 1 --per-page 25 --json
  castkit handoff get --session sess_123 --ref ref_readme_0021 --json
  castkit validate --session sess_123 --script demo.json --json
  castkit execute --session sess_123 --script demo.json --non-interactive -o showcase.mp4
```

---

## Pipeline

### External Agent Flow

```
Target CLI → handoff init/list/get → Agent writes DemoScript(JSON) → validate → execute → MP4/GIF/WebM
```

This is the default v1 user journey.

### Flow

```
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│ Discover │───▶│   Plan   │───▶│  Record  │───▶│  Redact  │───▶│  Render  │───▶│  Encode  │
└──────────┘    └──────────┘    └──────────┘    └──────────┘    └──────────┘    └──────────┘
     │               │               │               │               │               │
 help output    DemoPlan         Recording       Recording       Frame PNGs        MP4/GIF
 README text    (JSON)          (JSON+frames)   (sanitized)     (in memory)       (file)
```

### Intermediate Artifacts

Each pipeline stage produces a serializable artifact. This means:
- You can stop after any stage and inspect/edit
- You can resume from any stage
- Artifacts are JSON (plans, recordings) or binary (frames, video)

---

## Module Specs

### 1. Discovery

**Purpose:** Understand what a CLI tool does without any human input.

**Input:** Binary path or command name + optional README path

**Process:**

```
1. Locate the binary (resolve PATH if needed)
2. Run: <binary> --help, <binary> -h, <binary> help
3. Parse help output:
   - Extract: tool name, description/tagline
   - Extract: subcommands list with descriptions
   - Extract: flags/options with descriptions
   - Extract: usage examples from help text
4. If README exists (cwd, parent, or explicit):
   - Extract code blocks (```bash, ```shell, ```console)
   - Extract sections: "Usage", "Examples", "Quick Start", "Getting Started"
   - Filter to commands that reference the target binary
5. Build DiscoveryResult
```

**Output:** `DiscoveryResult`

```rust
struct DiscoveryResult {
    tool_name: String,
    binary_path: PathBuf,
    tagline: Option<String>,         // one-line description
    subcommands: Vec<SubcommandInfo>,
    global_flags: Vec<FlagInfo>,
    readme_examples: Vec<CodeExample>,
    help_text: String,               // raw --help output
    refs_index: RefsIndex,           // paginated retrieval index for agents
}

struct SubcommandInfo {
    name: String,
    description: String,
    flags: Vec<FlagInfo>,
    examples: Vec<String>,
}

struct FlagInfo {
    short: Option<String>,
    long: String,
    description: String,
    takes_value: bool,
}

struct CodeExample {
    source: String,      // "readme", "help"
    commands: Vec<String>,
    context: String,     // surrounding text
}

struct RefsIndex {
    session_id: String,
    refs: Vec<RefItem>,
}

struct RefItem {
    ref_id: String,      // stable ID used by handoff get
    source: String,      // help | readme | files | probes
    kind: String,        // section | code_block | file_snippet | probe_result
    title: Option<String>,
    content: String,     // exact source content (never truncated)
}
```

**Help parser heuristics:**
- Detect clap-style, structopt-style, hand-rolled help formats
- Handle multi-level subcommands (e.g., `mycli config set`)
- Extract examples from `EXAMPLES:` sections in help text

### 2. Planning

**Purpose:** Convert agent-authored script into an executable demo plan, with strict anti-invention validation.

**Input:** `DiscoveryResult` + `DemoScript` (agent-authored JSON)

**Process:**

```
1. Agent uses handoff APIs to gather evidence:
   - handoff list (paginated refs)
   - handoff get (exact ref content)
2. Agent returns DemoScript JSON:
   - setup, scenes, checks, cleanup
   - every executable step includes source_refs
3. Validate DemoScript:
   - hard-fail missing/invalid source_refs
   - hard-fail unknown commands not supported by discovery graph
   - hard-fail dependency/order issues (.env, config files, setup requirements)
4. Normalize validated script into internal DemoPlan
5. Pass DemoPlan to recorder
```

**External Script Type:** `DemoScript`

```rust
struct DemoScript {
    version: String,
    setup: Vec<ScriptStep>,
    scenes: Vec<ScriptScene>,
    checks: Vec<ScriptStep>,
    cleanup: Vec<ScriptStep>,
    redactions: Vec<RedactRule>,
    audio: Option<AudioConfig>,
}

struct ScriptScene {
    id: String,
    title: String,
    steps: Vec<ScriptStep>,
}

struct ScriptStep {
    id: String,
    run: String,
    expect: Option<ExpectCondition>,
    timeout_ms: Option<u64>,
    source_refs: Vec<String>,    // REQUIRED and non-empty
    manual_step: Option<bool>,   // optional escape hatch, still requires refs
    manual_reason: Option<String>,
}
```

**Internal Output:** `DemoPlan`

```rust
struct DemoPlan {
    tool_name: String,
    tagline: String,
    total_duration_hint: Duration,
    scenes: Vec<Scene>,
    setup: Option<SetupConfig>,  // global setup before all scenes
}

struct Scene {
    id: String,
    title: String,
    steps: Vec<Step>,
    transition: Transition,
    duration_hint: DurationHint,
}

struct Step {
    kind: StepKind,
    content: String,
    expect: Option<ExpectCondition>,
    delay_after: Option<Duration>,
}

enum StepKind {
    Command,        // type and execute
    Type,           // type but don't execute yet
    Enter,          // press enter
    Wait,           // wait for condition
    Pause,          // visual pause (breathing room)
    Clear,          // clear terminal
    Comment,        // # shown as dimmed comment before command
}

enum ExpectCondition {
    Contains(String),
    Regex(String),
    Timeout(Duration),
    ExitCode(i32),
}

enum Transition {
    Crossfade { duration_ms: u32 },
    Cut,
    FadeToBlack { duration_ms: u32 },
}

enum DurationHint {
    Short,    // 3-5s
    Medium,   // 5-10s
    Long,     // 10-20s
}

struct SetupConfig {
    commands: Vec<String>,     // run before recording (not shown)
    temp_files: Vec<TempFile>, // create these files in sandbox
    env_vars: Vec<(String, String)>,
}

struct TempFile {
    path: String,
    content: String,
}
```

**Validation contract (hard rules):**
- each script step must include at least one valid `source_ref`
- `source_refs` must resolve within current handoff session
- command must exist in discovered command graph unless `manual_step == true`
- `manual_step` still requires evidence refs and `manual_reason`

`DemoPlan` remains JSON-serializable, but the primary intervention point is now the validated `DemoScript`.

### 3. Recording

**Purpose:** Execute the demo plan in a controlled PTY and capture terminal state over time.

**Input:** `DemoPlan`

**Process:**

```
1. Create sandbox directory (temp dir)
2. Run global setup (create files, set env)
3. Open PTY with portable-pty:
   - Size: configured cols × rows (default 100×30)
   - Shell: configured shell
   - Env: clean env + PATH + custom vars
   - Working dir: sandbox
4. For each scene:
   a. For each step:
      - Command/Type: send keystrokes with human-like timing
      - Enter: send \r
      - Wait: poll terminal state for expect condition
      - Pause: wait configured duration
   b. Capture terminal state snapshots at regular intervals (every 16ms = 60fps)
   c. Store: timestamp + full terminal grid state (cells with char, fg, bg, attrs)
5. Between scenes: optional clear + pause
6. Close PTY, cleanup sandbox
7. Output Recording
```

**Terminal state capture:**

Use `vt100` crate (or `alacritty_terminal`) to maintain virtual terminal state:

```rust
// Feed PTY output into terminal parser
let parser = vt100::Parser::new(rows, cols, 0);
// On each PTY read:
parser.process(&output_bytes);
// Snapshot:
let screen = parser.screen();
// Access each cell:
for row in 0..rows {
    for col in 0..cols {
        let cell = screen.cell(row, col);
        // cell.contents() -> String (character)
        // cell.fgcolor() -> Color
        // cell.bgcolor() -> Color
        // cell.bold(), cell.italic(), cell.underline()
    }
}
```

**Human-like typing:**

```rust
struct TypingConfig {
    base_delay_ms: u32,        // 70ms default
    variance_ms: u32,          // ±25ms
    word_pause_ms: u32,        // 120ms after space
    punctuation_pause_ms: u32, // 80ms after . , ; :
    thinking_pause_ms: u32,    // 300-600ms before first char of command
    typo_rate: f32,            // 0.0 for v1 (future: type mistake, backspace, retype)
}

fn keystroke_delay(ch: char, prev: char, config: &TypingConfig) -> Duration {
    let base = config.base_delay_ms as f32;
    let jitter = rand::thread_rng().gen_range(
        -(config.variance_ms as f32)..=(config.variance_ms as f32)
    );
    let extra = match ch {
        ' ' => config.word_pause_ms as f32,
        '.' | ',' | ';' | ':' => config.punctuation_pause_ms as f32,
        '-' if prev == '-' => 30.0, // fast double-dash
        _ => 0.0,
    };
    Duration::from_millis((base + jitter + extra).max(20.0) as u64)
}
```

**Output:** `Recording`

```rust
struct Recording {
    plan: DemoPlan,
    terminal_size: (u16, u16),  // cols, rows
    scenes: Vec<RecordedScene>,
}

struct RecordedScene {
    scene_id: String,
    frames: Vec<Frame>,
    duration: Duration,
}

struct Frame {
    timestamp: Duration,  // from scene start
    grid: Vec<Vec<Cell>>,
    cursor: Option<CursorState>,
}

struct Cell {
    content: String,    // character (may be multi-byte)
    fg: Color,
    bg: Color,
    bold: bool,
    italic: bool,
    underline: bool,
    dim: bool,
    strikethrough: bool,
}

struct CursorState {
    row: u16,
    col: u16,
    visible: bool,
    shape: CursorShape,
}

enum Color {
    Default,
    Indexed(u8),      // 0-255
    Rgb(u8, u8, u8),
}
```

**Recording serialization:** Save as compressed JSON or MessagePack for large recordings. This allows re-rendering with different styles without re-recording.

### 4. Redaction

**Purpose:** Scan all frames and sanitize sensitive content before rendering.

**Input:** `Recording` (mutable)

**Process:**

```
1. Build redaction pattern set:
   a. Built-in patterns (always on):
      - API keys: sk-*, pk_*, ghp_*, ghs_*, AKIA*, xox[bpas]-*, etc.
      - Bearer/auth tokens in output
      - Home directory: /Users/<username>/, /home/<username>/
      - Email addresses: basic regex
      - Private IPs: 10.*, 172.16-31.*, 192.168.*
      - AWS account IDs (12-digit numbers in ARN context)
      - Common env var values: *_KEY, *_SECRET, *_TOKEN, *_PASSWORD
   b. Custom patterns from config
   c. Path replacements from config

2. For each frame in each scene:
   - Scan row text content against all patterns
   - Replace matches with redaction string:
     - Short values (≤20 chars): "••••••••"
     - Long values: "••••••••••••••••"
     - Paths: replace prefix (/Users/dee → /Users/demo)
   - Preserve cell colors/attributes on replaced characters
   - Track redaction locations for subtle visual indicator (optional dim highlight)

3. Log redactions (verbose mode): "Redacted 3 occurrences of API key pattern in scene 2"
```

**Built-in patterns:**

```rust
const REDACT_PATTERNS: &[(&str, &str)] = &[
    // Pattern name, regex
    ("openai_key",     r"sk-[a-zA-Z0-9]{20,}"),
    ("github_pat",     r"ghp_[a-zA-Z0-9]{36,}"),
    ("github_secret",  r"ghs_[a-zA-Z0-9]{36,}"),
    ("aws_key",        r"AKIA[0-9A-Z]{16}"),
    ("stripe_key",     r"[sr]k_(live|test)_[a-zA-Z0-9]{24,}"),
    ("slack_token",    r"xox[bpas]-[a-zA-Z0-9\-]+"),
    ("bearer",         r"[Bb]earer\s+[a-zA-Z0-9\-._~+/]+=*"),
    ("generic_key",    r"(?i)(api[_-]?key|secret|token|password)\s*[=:]\s*\S+"),
    ("email",          r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}"),
    ("private_ip",     r"\b(10\.\d{1,3}|172\.(1[6-9]|2\d|3[01])|192\.168)\.\d{1,3}\.\d{1,3}\b"),
    ("jwt",            r"eyJ[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]{10,}"),
];
```

### 5. Rendering

**Purpose:** Transform terminal state frames into pixel frames with animations and branding.

This is the core of what makes castkit special. Every visual element is composited onto a pixel buffer per frame.

**Input:** `Recording` (redacted) + `RenderConfig`

**Render engine: `tiny-skia`** (pure Rust, no system dependencies) for 2D rendering. Falls back to `skia-safe` if GPU acceleration needed (future).

For v1: use `tiny-skia` — it's simpler, pure Rust, no C++ build dependency. It handles everything we need: text rendering via `rusttype`/`fontdue`, shapes, gradients, alpha blending.

Actually, for **text rendering quality** (subpixel, ligatures), we should use `cosmic-text` which is a pure-Rust text layout engine with proper shaping (uses `swash` for font rasterization). This is what the Cosmic desktop uses.

**Revised render stack:**
- `cosmic-text` — text shaping, layout, rasterization (supports ligatures, emoji, fallback)
- `tiny-skia` — 2D drawing (shapes, gradients, shadows, compositing)
- Frame buffer: RGBA `Vec<u8>` at target resolution
- Pipe to ffmpeg for encoding

**Frame composition pipeline (per frame):**

```
1. Calculate animation state:
   - Current zoom level + center point (from auto-zoom system)
   - Any active transitions (crossfade alpha)
   - Cursor interpolated position

2. Draw background:
   - Solid color, linear gradient, or radial gradient
   - Full canvas size

3. Draw window shadow:
   - Render rounded rect slightly offset (+4px, +4px)
   - Apply gaussian blur (approximated with box blur × 3)
   - Alpha: 0.3-0.5

4. Draw window frame:
   - Rounded rectangle (border-radius from config)
   - Fill with terminal background color
   - Clip all terminal content to this rect

5. Draw title bar:
   - Height: 38px
   - Background: slightly lighter than terminal bg
   - Traffic lights: three circles at (20, 19), (40, 19), (60, 19), radius 6px
     - Red: #FF5F57, Yellow: #FEBC2E, Green: #28C840
     - Subtle inner shadow on each (1px, 60% darker variant)
   - Title text: tool name, centered, 13px, medium weight, 60% opacity

6. Draw terminal content:
   - For each cell in the grid:
     - Calculate pixel position: x = col * cell_width + padding, y = row * cell_height + title_bar_height + padding
     - Draw background rect if bg != default
     - Draw character with cosmic-text:
       - Apply fg color from theme mapping
       - Apply bold/italic/underline attributes
       - Handle wide characters (CJK) — double cell width
   - Draw cursor:
     - Interpolate position smoothly between actual positions
     - Block cursor with blink (500ms cycle)
     - Slight glow effect (optional)

7. Apply auto-zoom:
   - Calculate visible viewport based on zoom level + center
   - Crop the terminal content to viewport
   - Scale to output resolution with bilinear interpolation

8. Draw branding overlays:
   - Intro/outro cards (separate frames)
   - Watermark (corner, low opacity)
   - Scene title card (if transitioning between scenes)

9. Apply transition:
   - Crossfade: blend current scene's last frames with next scene's first frames

10. Write RGBA buffer to encoder
```

**Canvas dimensions:**

```
Output: 1920×1080 (standard), 3840×2160 (high), 1280×720 (draft)

Layout:
┌─────────────────────────────────────────────────┐
│                   padding (80px)                │
│   ┌─────────────────────────────────────────┐   │
│   │ 🔴 🟡 🟢  tool name                     │   │ ← title bar (38px)
│   ├─────────────────────────────────────────┤   │
│   │                                         │   │
│   │  terminal content area                  │   │ ← rows × cols of cells
│   │  (monospace grid)                       │   │
│   │                                         │   │
│   └─────────────────────────────────────────┘   │
│                                                 │
└─────────────────────────────────────────────────┘

Terminal window size adapts to content:
- cell_width = font_size * 0.6 (approximate for monospace)
- cell_height = font_size * 1.4 (line height)
- window_width = cols * cell_width + internal_padding * 2
- window_height = title_bar + rows * cell_height + internal_padding * 2
- Canvas = window + outer_padding * 2 (centered)
```

### 6. Encoding

**Purpose:** Encode rendered frames to video file.

**Method:** Pipe raw RGBA frames to ffmpeg via stdin.

```rust
fn spawn_encoder(config: &EncodeConfig) -> Child {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&[
        "-y",                           // overwrite
        "-f", "rawvideo",               // input format
        "-pix_fmt", "rgba",             // pixel format
        "-s", &format!("{}x{}", config.width, config.height),
        "-r", &config.fps.to_string(),  // framerate
        "-i", "-",                      // stdin
    ]);

    match config.format {
        Format::Mp4 => {
            cmd.args(&[
                "-c:v", "libx264",
                "-preset", &config.preset,  // slow for quality, fast for draft
                "-crf", &config.crf.to_string(),  // 18 for standard, 12 for high
                "-pix_fmt", "yuv420p",       // compatibility
                "-movflags", "+faststart",   // web streaming
            ]);
        }
        Format::Gif => {
            // two-pass for quality GIF
            cmd.args(&[
                "-vf", "fps=15,scale=960:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse",
            ]);
        }
        Format::Webm => {
            cmd.args(&[
                "-c:v", "libvpx-vp9",
                "-crf", "30",
                "-b:v", "0",
            ]);
        }
    }

    cmd.arg(&config.output_path);
    cmd.stdin(Stdio::piped())
       .stdout(Stdio::null())
       .stderr(Stdio::piped())
       .spawn()
       .expect("ffmpeg must be installed")
}
```

**Quality presets:**

| Preset | Resolution | FPS | CRF | ffmpeg preset | ~Size (30s) |
|--------|-----------|-----|-----|---------------|-------------|
| draft | 1280×720 | 30 | 23 | fast | ~2MB |
| standard | 1920×1080 | 60 | 18 | slow | ~8MB |
| high | 3840×2160 | 60 | 12 | slower | ~30MB |

---

## Data Types

### Full type reference

```rust
// === Config ===

struct CastkitConfig {
    style: Style,
    theme: TerminalTheme,
    font: FontConfig,
    quality: Quality,
    format: OutputFormat,
    animation: AnimationConfig,
    brand: BrandConfig,
    redaction: RedactionConfig,
    terminal: TerminalConfig,
}

struct FontConfig {
    family: String,           // "JetBrains Mono"
    size: f32,                // 14.0
    line_height: f32,         // 1.4
    ligatures: bool,          // true
}

struct AnimationConfig {
    typing_speed: TypingSpeed,     // natural, fast, slow
    zoom_enabled: bool,
    zoom_intensity: f32,           // 1.0-2.0 (1.3 default)
    zoom_ease_duration_ms: u32,    // 600ms
    transition_duration_ms: u32,   // 400ms
    pause_between_scenes_ms: u32,  // 1500ms
    cursor_blink_ms: u32,          // 500ms
    scroll_ease_ms: u32,           // 200ms
}

struct BrandConfig {
    enabled: bool,
    intro: Option<IntroConfig>,
    outro: Option<OutroConfig>,
    watermark: Option<WatermarkConfig>,
    captions: bool,
}

struct IntroConfig {
    duration_ms: u32,        // 1500
    title: String,           // tool name
    subtitle: Option<String>,// tagline
    bg_color: String,
    text_color: String,
}

struct OutroConfig {
    duration_ms: u32,
    text: String,            // "github.com/user/repo"
    bg_color: String,
}

struct TerminalConfig {
    cols: u16,      // 100
    rows: u16,      // 30
    shell: String,
    sandbox: bool,
}

// === Render Types ===

struct RenderContext {
    canvas_width: u32,
    canvas_height: u32,
    window_rect: Rect,        // terminal window position
    content_rect: Rect,       // terminal content area
    cell_size: (f32, f32),    // width, height
    theme: ResolvedTheme,     // colors resolved from theme
    font: LoadedFont,         // cosmic-text font system
    frame_count: usize,
    current_frame: usize,
}

struct Rect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

struct AutoZoomState {
    current_center: (f32, f32),
    current_zoom: f32,
    target_center: (f32, f32),
    target_zoom: f32,
    ease_progress: f32,      // 0.0 to 1.0
}
```

---

## Animation System

### Auto-Zoom

The signature feature. Camera follows the action.

**When to zoom in:**
- New command being typed → zoom to command line area
- Output appearing → zoom to output region
- Error output → zoom tighter on error

**When to zoom out:**
- Between scenes → ease back to full view
- After output settles → gradual pull back
- Long output that fills screen → zoom to fit

**Algorithm:**

```rust
fn update_auto_zoom(state: &mut AutoZoomState, frame: &Frame, config: &AnimationConfig) {
    // Find the "active region" — where new content appeared
    let active_region = detect_active_region(frame);

    // Calculate target zoom to frame the active region with padding
    let target_zoom = calculate_zoom_for_region(&active_region, config.zoom_intensity);
    let target_center = active_region.center();

    // Update targets (with hysteresis — don't jitter on small changes)
    if (target_center - state.target_center).length() > HYSTERESIS_THRESHOLD {
        state.target_center = target_center;
        state.target_zoom = target_zoom;
        state.ease_progress = 0.0;
    }

    // Ease toward target
    state.ease_progress = (state.ease_progress + 1.0 / ease_frames(config)).min(1.0);
    let t = ease_in_out_cubic(state.ease_progress);

    state.current_center = lerp(state.current_center, state.target_center, t);
    state.current_zoom = lerp(state.current_zoom, state.target_zoom, t);
}

fn detect_active_region(frame: &Frame) -> Rect {
    // Compare with previous frame
    // Find rows that changed
    // Return bounding rect of changed area + padding
}
```

**Active region detection:**
- Diff current frame with previous frame
- Find bounding box of changed cells
- Add padding (2 rows above, 4 rows below, 5 cols each side)
- Smooth transitions — don't snap to new region, ease over 400-600ms

### Easing Functions

All built-in, no crate needed:

```rust
fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 { 4.0 * t * t * t }
    else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

fn ease_out_quint(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(5)
}

fn ease_in_out_quad(t: f32) -> f32 {
    if t < 0.5 { 2.0 * t * t }
    else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
```

**Usage:**
- Auto-zoom movement: `ease_in_out_cubic`
- Cursor position: `ease_out_quint` (snappy, then settle)
- Opacity transitions: `ease_in_out_quad`
- Scene crossfades: linear

### Cursor Animation

```rust
struct CursorAnimator {
    current_pos: (f32, f32),    // smooth pixel position
    target_pos: (f32, f32),
    blink_timer: f32,
    visible: bool,
}

impl CursorAnimator {
    fn update(&mut self, actual_pos: (u16, u16), dt: f32) {
        let target = (
            actual_pos.1 as f32 * cell_width,
            actual_pos.0 as f32 * cell_height,
        );

        // Smooth movement (never teleport)
        let speed = 12.0; // higher = snappier
        self.current_pos.0 += (target.0 - self.current_pos.0) * speed * dt;
        self.current_pos.1 += (target.1 - self.current_pos.1) * speed * dt;

        // Blink
        self.blink_timer += dt;
        if self.blink_timer > 0.5 {
            self.blink_timer = 0.0;
            self.visible = !self.visible;
        }
    }
}
```

---

## Theme System

### Terminal Color Themes

Themes map ANSI color indices to RGB values:

```rust
struct TerminalTheme {
    name: String,
    // Base colors
    background: Rgb,
    foreground: Rgb,
    cursor: Rgb,
    selection: Rgb,
    // ANSI 16 colors
    black: Rgb,
    red: Rgb,
    green: Rgb,
    yellow: Rgb,
    blue: Rgb,
    magenta: Rgb,
    cyan: Rgb,
    white: Rgb,
    bright_black: Rgb,
    bright_red: Rgb,
    bright_green: Rgb,
    bright_yellow: Rgb,
    bright_blue: Rgb,
    bright_magenta: Rgb,
    bright_cyan: Rgb,
    bright_white: Rgb,
}
```

**Built-in themes:**
- `catppuccin-mocha` (default)
- `tokyo-night`
- `dracula`
- `one-dark`
- `github-dark`
- `solarized-dark`
- `nord`
- Custom via TOML

### Visual Styles

Styles control the non-terminal aesthetics:

```rust
struct Style {
    name: String,
    background: Background,
    window_shadow: ShadowConfig,
    window_border_radius: f32,
    title_bar: TitleBarStyle,
    padding: f32,
}

enum Background {
    Solid(Rgb),
    LinearGradient { from: Rgb, to: Rgb, angle: f32 },
    RadialGradient { center: Rgb, edge: Rgb },
    Mesh { colors: Vec<Rgb> },  // future: mesh gradient
}

struct ShadowConfig {
    offset_x: f32,
    offset_y: f32,
    blur_radius: f32,
    color: Rgba,
}
```

**Built-in styles:**

| Name | Background | Shadow | Vibe |
|------|-----------|--------|------|
| `dark` | `#0a0a0a` solid | heavy, dark | Screen Studio dark mode |
| `light` | `#f5f5f5` solid | soft, light | Clean docs style |
| `minimal` | `#000000` solid | none | Just the terminal |
| `hacker` | green→black gradient | green glow | Matrix vibes |
| `ocean` | `#0f0c29→#302b63→#24243e` | purple tint | Purple aesthetic |

---

## Branding

### Intro Card

```
┌──────────────────────────────────────────┐
│                                          │
│                                          │
│              mycli                       │  ← tool name, 48px, bold
│     A fast project scaffolder            │  ← tagline, 20px, 60% opacity
│                                          │
│                                          │
└──────────────────────────────────────────┘
Duration: 1.5s
Animation: fade in (400ms) → hold → crossfade to first scene
```

### Outro Card

```
┌──────────────────────────────────────────┐
│                                          │
│     ★ github.com/user/mycli              │  ← CTA, 24px
│                                          │
│     Made with castkit                    │  ← attribution, 14px, 40% opacity (optional)
│                                          │
└──────────────────────────────────────────┘
Duration: 2s
Animation: crossfade from last scene → hold → fade out (400ms)
```

### Scene Title Cards (between scenes)

Optional overlay during crossfade:

```
Bottom-left corner, 16px, fades in during transition:
"Creating a project" → "Running tests" → "Deploying"
```

---

## Configuration

### `castkit.toml` (project root or `~/.config/castkit/config.toml`)

```toml
[defaults]
style = "dark"
theme = "catppuccin-mocha"
font = "JetBrains Mono"
font_size = 14
quality = "standard"
format = "mp4"
fps = 60

[terminal]
cols = 100
rows = 30
shell = "/bin/zsh"
sandbox = true

[animation]
typing_speed = "natural"     # natural | fast | slow
zoom_enabled = true
zoom_intensity = 1.3
transition = "crossfade"
transition_duration_ms = 400
pause_between_scenes_ms = 1500

[brand]
enabled = true
name = "My Tool"
tagline = "Does cool things"
cta = "github.com/user/tool"
# logo = "./assets/logo.png"  # future

[brand.colors]
intro_bg = "#0a0a0a"
intro_text = "#ffffff"
outro_bg = "#0a0a0a"
outro_text = "#ffffff"

[redact]
enabled = true
extra_patterns = ["INTERNAL_.*"]
path_map = { "/Users/dee" = "/Users/demo", "/home/dee" = "/home/demo" }
# allowlist = ["safe-key-pattern"]  # don't redact these
```

---

## Project Structure

```
castkit/
├── Cargo.toml
├── README.md
├── LICENSE                      # MIT
├── src/
│   ├── main.rs                  # CLI entry (clap)
│   ├── lib.rs                   # Public API (for library usage)
│   ├── config.rs                # Config loading + merging (file + CLI args)
│   ├── pipeline.rs              # Orchestrates discover → plan → record → redact → render → encode
│   │
│   ├── discover/
│   │   ├── mod.rs               # Discovery orchestrator
│   │   ├── help.rs              # Parse --help output (clap, manual, structopt patterns)
│   │   ├── readme.rs            # Extract code examples from README
│   │   └── types.rs             # DiscoveryResult, SubcommandInfo, etc.
│   │
│   ├── plan/
│   │   ├── mod.rs               # Plan generation from DiscoveryResult
│   │   ├── scenario.rs          # Scenario selection heuristics
│   │   ├── mockdata.rs          # Generate fake project names, files, etc.
│   │   └── types.rs             # DemoPlan, Scene, Step, etc.
│   │
│   ├── record/
│   │   ├── mod.rs               # Recording orchestrator
│   │   ├── pty.rs               # PTY spawn + management (portable-pty)
│   │   ├── typing.rs            # Human-like keystroke timing
│   │   ├── capture.rs           # Terminal state snapshots (vt100)
│   │   ├── expect.rs            # Wait-for-output logic
│   │   └── types.rs             # Recording, Frame, Cell, etc.
│   │
│   ├── redact/
│   │   ├── mod.rs               # Redaction engine
│   │   ├── patterns.rs          # Built-in secret patterns
│   │   └── types.rs             # RedactionConfig, RedactionReport
│   │
│   ├── render/
│   │   ├── mod.rs               # Render orchestrator (frame loop)
│   │   ├── canvas.rs            # Frame buffer management
│   │   ├── background.rs        # Background drawing (solid, gradient)
│   │   ├── window.rs            # Window chrome (shadow, frame, title bar, traffic lights)
│   │   ├── terminal.rs          # Terminal content rendering (cells → pixels)
│   │   ├── text.rs              # Text rendering via cosmic-text
│   │   ├── cursor.rs            # Cursor rendering + animation
│   │   ├── zoom.rs              # Auto-zoom system
│   │   ├── transition.rs        # Scene transitions (crossfade, fade)
│   │   ├── brand.rs             # Intro/outro/watermark rendering
│   │   └── types.rs             # RenderContext, AutoZoomState, etc.
│   │
│   ├── encode/
│   │   ├── mod.rs               # Encoding orchestrator
│   │   ├── ffmpeg.rs            # ffmpeg pipe interface
│   │   └── gif.rs               # GIF-specific optimization
│   │
│   ├── theme/
│   │   ├── mod.rs               # Theme loading + resolution
│   │   ├── builtin.rs           # Built-in themes (catppuccin, etc.)
│   │   └── types.rs             # TerminalTheme, Style, etc.
│   │
│   └── util/
│       ├── easing.rs            # Easing functions
│       ├── color.rs             # Color conversion helpers
│       └── fs.rs                # Sandbox, temp dir helpers
│
├── themes/                      # Theme definitions (TOML)
│   ├── catppuccin-mocha.toml
│   ├── tokyo-night.toml
│   ├── dracula.toml
│   └── one-dark.toml
│
├── fonts/                       # Bundled or downloaded on first run
│   └── .gitkeep               # Fonts downloaded at build/first-run
│
└── tests/
    ├── discover_test.rs
    ├── plan_test.rs
    ├── record_test.rs
    ├── redact_test.rs
    └── render_test.rs
```

---

## Dependencies

### Cargo.toml (key deps)

```toml
[dependencies]
# CLI
clap = { version = "4", features = ["derive"] }

# Terminal emulation
vt100 = "0.15"                    # Terminal state parser

# PTY
portable-pty = "0.8"             # Cross-platform PTY

# Rendering
tiny-skia = "0.11"               # 2D rasterizer (shapes, gradients, compositing)
cosmic-text = "0.12"             # Text shaping + rendering (ligatures, emoji, fallback)
fontdue = "0.9"                  # Fast font rasterization (fallback)

# Image
png = "0.17"                     # PNG encoding (for png-sequence output)

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Utilities
rand = "0.8"                     # Typing jitter
regex = "1"                      # Redaction patterns
tempfile = "3"                   # Sandbox directories
which = "6"                      # Find binaries in PATH
indicatif = "0.17"               # Progress bars during render

[build-dependencies]
# None — we keep the build simple
```

### System Requirements

- **ffmpeg** — must be installed and in PATH. castkit checks on startup and prints install instructions if missing.
- **Rust 1.75+** — for building from source
- **macOS or Linux** — v1 target platforms (Windows future)

### Font Strategy

1. Check system fonts first (JetBrains Mono, Fira Code, SF Mono, Menlo)
2. If not found, download from Google Fonts / GitHub releases on first run
3. Cache in `~/.cache/castkit/fonts/`
4. Bundle a fallback (e.g., embedded Iosevka subset) for guaranteed rendering

---

## Build & Distribution

```bash
# From source
cargo install castkit

# Or clone + build
git clone https://github.com/deeflect/castkit
cd castkit
cargo build --release

# Homebrew (future)
brew install castkit
```

**Binary size target:** <15MB (Rust static binary)

**CI:** GitHub Actions for macOS + Linux releases. Attach binaries to GitHub Releases.

---

## v1 Scope

### In ✅

- [x] CLI interface with clap
- [x] Agent-native handoff protocol (`handoff init/list/get`)
- [x] Auto-discovery from --help + README
- [x] Paginated source retrieval with stable `ref_id`s
- [x] Strict DemoScript JSON contract
- [x] Hard validation for missing `source_refs`
- [x] Hard validation for unsupported command invention
- [x] PTY recording with human-like typing
- [x] Terminal state capture via vt100
- [x] Auto-redaction of secrets (built-in patterns)
- [x] Custom redaction config
- [x] Terminal rendering (cosmic-text + tiny-skia)
- [x] macOS window chrome (traffic lights, title bar, shadow)
- [x] Auto-zoom with easing
- [x] Crossfade scene transitions
- [x] Cursor smoothing + blink
- [x] Built-in color themes (catppuccin, tokyo-night, dracula, one-dark)
- [x] Visual styles (dark, light, minimal, ocean, hacker)
- [x] Intro/outro branding cards
- [x] MP4 output (H.264 via ffmpeg)
- [x] GIF output (optimized)
- [x] WebM output
- [x] Optional typing sound track
- [x] Optional background music track (basic mix)
- [x] Config file (castkit.toml)
- [x] Quality presets (draft/standard/high)
- [x] Non-interactive execute path for agents
- [x] Progress bar during render

### Out (v2+) 🔮

- [ ] Browser/GUI recording (Playwright integration)
- [ ] AI voiceover narration (TTS)
- [ ] Interactive web embed format (HTML5 player)
- [ ] Custom animation scripting (keyframe DSL)
- [ ] Advanced background music auto-ducking
- [ ] Multi-tool comparison demos (side by side)
- [ ] CI/CD integration (auto-generate on release tag)
- [ ] Logo/image overlay in intro
- [ ] Typo simulation (type mistake → backspace → correct)
- [ ] Camera shake on errors
- [ ] Rich sound effects pack (success chime, error cues, etc.)
- [ ] Windows support
- [ ] Plugin system for custom renderers
- [ ] Web service (API: upload binary, get video)

---

## Meta

**Repo:** `github.com/deeflect/castkit`
**License:** MIT
**First dogfood:** Generate castkit's own demo video
**Launch content:** "I built a tool that generates its own demo" — the meta-demo
