<p align="center">
  <h1 align="center">castkit</h1>
  <p align="center">
    <strong>One command. Polished demo video. No manual recording.</strong>
  </p>
  <p align="center">
    Agent-native CLI demo video generator with ScreenStudio-style rendering.
  </p>
</p>

<p align="center">
  <a href="#install">Install</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#agent-flow">Agent Flow</a> •
  <a href="#presets">Presets</a> •
  <a href="#branding">Branding</a>
</p>

---

<p align="center">
  <img src="examples/castkit-meta-demo.gif" alt="castkit meta demo" width="640" />
</p>

## What is castkit?

Point castkit at any CLI binary. It auto-discovers help text, README, and file structure — then generates a polished terminal demo video with typed commands, streamed output, camera motion, branding, and typing sounds.

Built for AI agents, works for humans.

```bash
# Full pipeline: discover → plan → validate → render
castkit handoff init ./my-cli --json
castkit plan scaffold --session $SESSION --json
castkit validate --session $SESSION --script demo.json --json
castkit execute --session $SESSION --script demo.json --non-interactive --preset polished --output demo.mp4
```

### Key features

- 🔍 **Auto-discovery** — Extracts help text, README, file structure, and probes to build an evidence graph
- 🛡️ **Evidence-first** — Every demo step requires `source_refs` from real discovery. No invented commands
- ✅ **Strict validation** — Rejects scripts with unknown commands, missing refs, or invalid patterns
- 🎬 **ScreenStudio-quality rendering** — Auto camera zoom, cursor tracking, crossfade transitions, typing sounds
- 🎨 **Branding** — Intro/outro cards, watermark, avatar, custom color themes
- 🔒 **Auto-redaction** — Built-in secret detection (API keys, tokens, paths) with configurable patterns
- 🤖 **Agent-native** — Deterministic non-interactive mode with JSON I/O for any coding agent
- 📦 **Self-contained** — Single Rust binary + ffmpeg + Node renderer. No Docker, no browser recording

## Install

### Requirements

- Rust 1.75+
- Node 20+
- `ffmpeg` in `PATH`
- Playwright Chromium

### Build from source

```bash
git clone https://github.com/deeflect/castkit.git
cd castkit
cargo install --path .

# Set up the renderer
npm install --prefix renderer-runtime
npx --prefix renderer-runtime playwright install chromium
```

## Quick Start

```bash
# 1. Point at any CLI
castkit handoff init ./my-tool --json

# 2. Auto-generate a demo script
castkit plan scaffold --session $SESSION --max-scenes 3 --json

# 3. Validate (catches invented commands, missing refs)
castkit validate --session $SESSION --script demo-script.json --json

# 4. Render the video
castkit execute --session $SESSION --script demo-script.json \
  --non-interactive --preset polished --output demo.mp4
```

## Agent Flow

castkit is designed for AI agents to use programmatically. Full JSON I/O, deterministic execution, no human intervention needed.

```
Binary → Discover → Plan → Validate → Execute → MP4/GIF
```

### Step by step

```bash
# Bootstrap: load contract + schema
castkit --json agent contract
castkit --json schema

# Initialize handoff session (auto-discovers the target)
castkit handoff init <target> --json

# Browse discovered evidence
castkit handoff list --session <id> --source help --page 1 --per-page 20 --json
castkit handoff list --session <id> --source readme --page 1 --per-page 20 --json

# Fetch specific refs
castkit handoff get --session <id> --ref <ref_id> --json

# Generate scaffold script
castkit plan scaffold --session <id> --output demo.json --max-scenes 3 --json

# Validate → Execute
castkit validate --session <id> --script demo.json --json
castkit execute --session <id> --script demo.json --non-interactive --preset polished --output demo.mp4 --json
```

> **Rule:** A step succeeds only if exit code is `0` and JSON contains `"ok": true`.

### Session chaining

Castkit automatically captures `session_id` from step output and makes it available as `$SESSION` in subsequent steps — no manual wiring needed.

## Presets

| Preset | Speed | Theme | Keystrokes | FPS | Use case |
|--------|-------|-------|------------|-----|----------|
| `quick` | fast | minimal | laptop | 30 | Fast iteration |
| `balanced` | quality | clean | laptop | 45 | Good enough |
| `polished` | quality | clean | mechanical | 60 | Showcase / launch |

```bash
castkit execute --preset polished --output demo.mp4 ...
```

Override any preset default with explicit flags: `--fps`, `--speed`, `--theme`, `--keystroke-profile`.

## Branding

Customize intro/outro cards, colors, watermark, and avatar.

```json
{
  "title": "my-tool",
  "bg_primary": "#0A1020",
  "bg_secondary": "#14243B",
  "text_primary": "#EAF2FF",
  "text_muted": "#9CB2D1",
  "command_text": "#8ED0FF",
  "accent": "#69C2FF",
  "watermark_text": "github.com/you/tool",
  "avatar_x": "yourhandle",
  "avatar_label": "@yourhandle"
}
```

Sources merge in order: `--theme` base → `script.branding` → `--branding file.json` → CLI overrides.

Ready-made palettes in `examples/`: `branding-clean.json`, `branding-bold.json`, `branding-minimal.json`.

## Output

| Format | Flag | Notes |
|--------|------|-------|
| MP4 | `--format mp4` (default) | H.264, best quality |
| WebM | `--format webm` | VP9, smaller files |
| GIF | `--format gif` | For READMEs and tweets |

### Rendering details

- Auto camera zoom follows cursor during typing
- Crossfade transitions between scenes
- Long output paginated with `-- page x/y --` markers
- `--no-zoom` for static framing
- Typing sounds (optional, via `audio.typing` in script)

## Validation

Every script is validated before execution:

- Each step must have non-empty `source_refs` from the handoff session
- Each `source_ref` must exist in the session
- Unknown commands fail unless marked `manual_step=true` with a `manual_reason`
- Invalid redaction regex patterns are caught
- Built-in secret redaction is always applied

## Project Structure

```
castkit/
├── src/                    # Rust CLI source
│   ├── execute/            # Step runner, redaction, transcripts
│   ├── handoff/            # Session management, ref discovery
│   ├── render/             # Renderer orchestration
│   ├── validate/           # Script validation engine
│   └── plan/               # Script scaffold generation
├── renderer-runtime/       # Node.js ScreenStudio-style renderer
│   └── render.mjs          # Playwright-based frame capture + ffmpeg encode
├── examples/               # Demo scripts, branding presets, sample videos
├── AGENTS.md               # Full agent contract + scenario design playbook
└── SPEC.md                 # Technical specification
```

## License

MIT

---

<p align="center">
  Built by <a href="https://x.com/deeflectcom">@deeflectcom</a>
</p>
