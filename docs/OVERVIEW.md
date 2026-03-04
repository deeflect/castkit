# castkit Overview

castkit is an agent-native CLI that converts verified terminal workflows into polished demo videos.

## End-to-End Pipeline
1. `handoff init`: Discovers command evidence from help/readme/files/probes.
2. `handoff list/get`: Lets agents retrieve refs in paginated chunks.
3. Script authoring: Agent produces strict `DemoScript` JSON with `source_refs`.
4. `validate`: Rejects unsupported or invented steps.
5. `execute --non-interactive`: Runs commands deterministically and records transcript.
6. Renderer: Produces screenstudio-style terminal video with branding/audio overlays.

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
- Terminal-only capture style
- MP4/WebM/GIF output with optional audio mix
- Configurable branding, avatar chip, watermark, scene tags
