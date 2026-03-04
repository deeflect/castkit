# castkit Optimization & Quality Roadmap (2026-03-04)

## Goals
- Remove correctness gaps (flags that do nothing, spec/behavior mismatch).
- Improve runtime efficiency for local iteration and final renders.
- Keep agent workflow strict and low-friction.

## Phase 1 (Immediate: correctness + UX)
1. Wire currently no-op execution flags:
- `--no-zoom` should disable camera zoom/pan behavior.
- `--format` should produce `mp4|webm|gif` outputs.
- `--json` should control output style (machine vs human-readable).
- `--verbose` should print stage logs.
2. Add tests for new behavior where cheap and deterministic.
3. Update docs to match actual behavior.

## Phase 2 (Near-term: performance)
1. Replace full snapshot copies with event/delta timeline model to reduce memory growth on long demos.
2. Replace frame-file rendering with ffmpeg piping (`image2pipe`) to reduce disk I/O and speed up long renders.
3. Add renderer benchmark command and profile presets tuned by duration.

## Phase 3 (Quality features)
1. Real sample-based typing SFX packs (optional) and better audio variation.
2. Scene-level camera controls (`focus`, `zoom_strength`, `hold_ms`).
3. Optional voiceover track with automatic ducking.

## Phase 4 (Agent-native polish)
1. Script scaffold command from refs (`castkit plan scaffold`) to reduce agent planning mistakes.
2. Better dependency/order checks for setup pathways (`.env`, config files, prerequisite commands).
3. Session storage backend option (SQLite + TTL cleanup) for large runs.

## Current execution decision
Implement Phase 1 now, starting with:
- `--no-zoom`
- `--format`
- `--json` and `--verbose` behavior improvements
