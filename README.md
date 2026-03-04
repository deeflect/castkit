# castkit

Agent-native CLI demo video generator for terminal tools, with ScreenStudio-style rendering.

## Why castkit
- Evidence-first workflow for agents (`source_refs` are required).
- Strict validation rejects invented/unsupported execution steps.
- Non-interactive deterministic run mode for reproducible demos.
- Polished terminal video output (typed commands, streamed output, auto camera, branding, avatar, watermark, optional audio).

See agent contract: `AGENTS.md`.

## Install requirements
- Rust 1.75+
- Node 20+
- `ffmpeg` in `PATH`
- Playwright Chromium runtime

Renderer setup:
```bash
npm install --prefix renderer-runtime
npx --prefix renderer-runtime playwright install chromium
```

## Build
```bash
cargo build
```

## Release prep
- Local preflight: `./scripts/release-ready.sh`
- Publish runbook: `docs/PUBLISHING.md`
- CI gates: `.github/workflows/ci.yml` and `.github/workflows/release-readiness.yml`

## Agent flow (non-interactive)
1. `castkit handoff init <target> --json`
2. `castkit handoff list --session <id> --source <help|readme|files|probes> --page 1 --per-page 20 --json`
3. `castkit handoff get --session <id> --ref <ref_id> --json`
4. Optional scaffold: `castkit plan scaffold --session <id> --output demo-script.json --max-scenes 3 --json`
5. Write or refine strict `DemoScript` JSON (template: `examples/demo-script.template.json`)
6. `castkit validate --session <id> --script demo.json --json`
7. `castkit execute --session <id> --script demo.json --non-interactive --preset polished --output demo.mp4 --json`

## Execute presets (easy settings)
- `--preset quick`: fastest iteration (`fast`, `minimal`, `laptop`, `fps=30`)
- `--preset balanced`: good quality/speed tradeoff (`quality`, `clean`, `laptop`, `fps=45`)
- `--preset polished`: showcase defaults (`quality`, `clean`, `mechanical`, `fps=60`)

Explicit flags still override preset defaults:
- `--fps`
- `--speed fast|quality`
- `--theme clean|bold|minimal`
- `--keystroke-profile mechanical|laptop|silent`
- `--encoder auto|software|hardware`

## Branding
Branding sources are merged in this order:
1. `--theme` base palette
2. `script.branding`
3. `--branding <file.json>`
4. direct CLI overrides (`--brand-title`, `--watermark`, `--avatar-x`, `--avatar-url`, `--avatar-label`)

Branding schema (all optional):
```json
{
  "title": "string",
  "bg_primary": "#0A1020",
  "bg_secondary": "#14243B",
  "text_primary": "#EAF2FF",
  "text_muted": "#9CB2D1",
  "command_text": "#8ED0FF",
  "accent": "#69C2FF",
  "watermark_text": "castkit.com",
  "avatar_x": "fric",
  "avatar_url": "https://.../avatar.png",
  "avatar_label": "@fric"
}
```

Ready palette files: `examples/branding-clean.json`, `examples/branding-bold.json`, `examples/branding-minimal.json`.

## Output rendering behavior
- Command typing drives camera zoom/focus.
- Model/output sections stay cleaner (no typing-focused zoom).
- Long output is paginated with markers (`-- page x/y --`) instead of truncation.
- `--no-zoom` locks camera framing (no pan/zoom motion).
- Typing sound + music are optional.
- Output formats: `mp4` (default), `webm`, `gif` via `--format`.
- Encoder mode: `--encoder auto` (default, cross-platform fallback), `software`, or `hardware`.

## Renderer runtime override
Default discovery order:
1. `./renderer-runtime`
2. `./renderer`

Override:
```bash
CASTKIT_RENDERER_HOME=/abs/path/to/renderer-runtime castkit execute ...
```

## Strict validation rules
- Each executable step must have non-empty `source_refs`.
- Each `source_ref` must exist in the session.
- Unknown commands fail unless `manual_step=true` and `manual_reason` is set.
- `.env` and common config file usage should be established in setup first.
- Invalid `redactions[].pattern` regex fails validation.
- Built-in secret redaction is always applied during execution output capture.
