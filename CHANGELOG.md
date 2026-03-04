# Changelog

## [0.1.0] - 2026-03-04
### Added
- Agent-native handoff pipeline (`handoff init/list/get`) with paginated refs.
- `plan scaffold` command for generating strict evidence-backed `DemoScript` starters.
- Strict `DemoScript` validation with evidence-backed `source_refs`.
- Non-interactive execution transcript and renderer pipeline.
- ScreenStudio-style terminal render mode with typed input, streaming output, auto-zoom, branding, avatar, watermark.
- Audio options with typing profiles and optional music mix.
- Presets for easier execution tuning (`quick|balanced|polished`).
- Agent contract doc (`AGENTS.md`) and script template example.

### Changed
- Long output rendering now uses pagination markers instead of truncation.
- `execute --format` now supports `mp4|webm|gif`.
- `execute --no-zoom` now disables camera motion.
- Output redaction is now enforced at execution-time (built-in secret patterns + custom regex rules).
- Discovery commands now use timeout protection and broader target-root file discovery.
- Renderer now streams frames directly to ffmpeg (no frame file spooling).
- Improved Cargo package metadata and exclusions for publish readiness.
