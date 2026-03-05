# castkit Overview

castkit is an agent-native CLI that converts verified terminal workflows into polished demo videos.

## End-to-End Pipeline
1. `agent contract` + `schema`: Binary exposes runtime contract + script schema for agents.
2. `handoff init`: Discovers command evidence from help/readme/files/probes.
3. `handoff list/get`: Lets agents retrieve refs in paginated chunks.
4. Script authoring: agent can start from `plan scaffold`, then refine strict `DemoScript` JSON with `source_refs`.
5. `validate`: Rejects unsupported or invented steps.
6. `execute --non-interactive`: Runs commands deterministically and records transcript.
7. Renderer: Produces screenstudio-style terminal video with branding/audio overlays.

## Automation Completion Signals
- Command considered complete only when exit code is `0` and JSON response has `ok=true`.
- `validate` success: `ok=true` with no validation errors.
- `execute` success: `ok=true` plus output path and render metadata.
- Any `ok=false` should be routed to:
  - script fix loop (validation/step failures), or
  - environment fix loop (missing runtime dependencies).

## Why it is Agent-Friendly
- Evidence-first command generation (refs are mandatory).
- Strict schema and hard validation failures.
- Non-interactive execution for reproducibility.
- Preset-driven rendering (`quick|balanced|polished`) to minimize tuning overhead.

## Runtime Requirements
- Rust 1.75+
- Node 20+
- Playwright Chromium
- ffmpeg

## Current v1 Scope
- Terminal mode with artifact overlays (image/result-card)
- Web mode with deterministic browser actions and focus zoom renderer
- MP4/WebM/GIF output with optional audio mix
- Configurable branding, avatar chip, watermark, scene tags

## Planned Next Scope
- Additional artifact types: `web_snapshot` and `chart`.
- Extended web actions and richer composited overlays/charts.
- Design and roadmap:
  - `docs/plans/2026-03-05-overlays-web-mode-design.md`
  - `docs/plans/2026-03-05-overlays-web-mode-implementation-plan.md`

## Timing and Scheduling Guidance
- Poll interval during execute: every `20s`.
- Soft timeout: `8m`.
- Hard timeout: `20m`.
- Approximate execute wall-clock:
  - 20-45s output video: ~1-5 minutes
  - 60-120s output video: ~2-10 minutes
  - 3-5 minute output video: ~6-20 minutes
