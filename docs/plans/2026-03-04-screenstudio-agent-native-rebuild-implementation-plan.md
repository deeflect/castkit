# ScreenStudio-Style Agent-Native Rebuild Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rebuild castkit so agents can reliably generate ScreenStudio-quality terminal demo videos (typed feel, smooth auto-zoom, polished window, optional music/typing audio) from strict evidence-backed scripts.

**Architecture:** Keep castkit as the orchestrator and validator, but move visual fidelity to an event-driven render pipeline: terminal session capture (`asciicast` events), scene render in browser terminal engine (`xterm.js`/asciinema player surface), camera path synthesis from event deltas, and deterministic encode/mix in ffmpeg. Preserve strict `handoff -> script -> validate -> execute` anti-hallucination workflow.

**Tech Stack:** Rust (`clap`, `serde`, `tokio`, `portable-pty` fallback), asciinema v3 cast format, browser renderer (Playwright + HTML/CSS/JS terminal canvas), ffmpeg for final encode/audio mix.

---

### Task 1: Freeze Current Renderer and Add New Pipeline Flag

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/lib.rs`
- Create: `src/pipeline/mod.rs`
- Test: `tests/cli_pipeline_mode_tests.rs`

**Step 1: Write failing CLI tests for renderer mode**
- Add tests for `--renderer legacy|screenstudio` on `execute`.

**Step 2: Run targeted tests**
- Run: `cargo test cli_pipeline_mode_tests -- --nocapture`
- Expected: FAIL (missing flag / dispatch).

**Step 3: Implement mode flag + dispatch**
- Add execute option:
  - `--renderer legacy` (current)
  - `--renderer screenstudio` (new default)

**Step 4: Verify tests**
- Run: `cargo test cli_pipeline_mode_tests -- --nocapture`
- Expected: PASS.

### Task 2: Define New Render Data Contracts (Event-Driven)

**Files:**
- Create: `src/pipeline/types.rs`
- Create: `src/pipeline/schema.rs`
- Test: `tests/pipeline_schema_tests.rs`

**Step 1: Write failing schema tests**
- Validate strict structures:
  - `CapturedSession`
  - `TimelineEvent`
  - `CameraKeyframe`
  - `RenderManifest`

**Step 2: Implement strict structs**
- `#[serde(deny_unknown_fields)]`
- deterministic timestamps in milliseconds

**Step 3: Verify tests**
- Run: `cargo test pipeline_schema_tests -- --nocapture`
- Expected: PASS.

### Task 3: Implement Terminal Event Capture (Asciicast-Compatible)

**Files:**
- Create: `src/capture/mod.rs`
- Create: `src/capture/asciicast.rs`
- Create: `src/capture/runner.rs`
- Test: `tests/capture_tests.rs`

**Step 1: Write failing capture tests**
- command execution emits:
  - input event stream
  - output event stream
  - timing metadata

**Step 2: Implement capture runner**
- execute each script step in PTY context
- record:
  - typed keystrokes timing
  - stdout/stderr chunks timing
  - resize events

**Step 3: Emit `CapturedSession` artifact**
- save JSON artifact in stable temp path

**Step 4: Verify tests**
- Run: `cargo test capture_tests -- --nocapture`
- Expected: PASS.

### Task 4: Improve Scenario Quality with Probe Evidence

**Files:**
- Modify: `src/handoff/discover.rs`
- Create: `src/handoff/probes.rs`
- Test: `tests/handoff_probe_tests.rs`

**Step 1: Write failing probe tests**
- ensure probe catalog includes:
  - command-specific `--help`
  - deterministic offline-safe feature probes
  - network viability probe

**Step 2: Implement probes + refs**
- add ref kinds:
  - `subcommand_help`
  - `probe_result`
  - `network_probe`

**Step 3: Add script guidance fields in handoff**
- per command:
  - likely setup deps
  - deterministic demo variants (online/offline)

**Step 4: Verify tests**
- Run: `cargo test handoff_probe_tests -- --nocapture`
- Expected: PASS.

### Task 5: Build Browser Renderer App Skeleton

**Files:**
- Create: `renderer/package.json`
- Create: `renderer/playwright.config.ts`
- Create: `renderer/src/index.html`
- Create: `renderer/src/player.ts`
- Create: `renderer/src/camera.ts`
- Create: `renderer/src/theme.css`
- Test: `renderer/tests/smoke.spec.ts`

**Step 1: Write failing renderer smoke test**
- Render one synthetic timeline and assert screenshot produced.

**Step 2: Implement page shell**
- terminal window chrome
- titlebar + mac traffic lights
- gradient background

**Step 3: Implement terminal playback mount**
- event-driven line updates
- caret state
- scroll region support

**Step 4: Verify renderer smoke**
- Run: `npm --prefix renderer test`
- Expected: PASS.

### Task 6: Implement ScreenStudio-Like Camera Engine

**Files:**
- Modify: `renderer/src/camera.ts`
- Create: `renderer/src/activity.ts`
- Test: `renderer/tests/camera.spec.ts`

**Step 1: Write failing camera tests**
- activity region changes produce eased keyframes
- hysteresis prevents jitter
- zoom-out reset between scenes

**Step 2: Implement activity detector**
- derive active row/region from event diffs

**Step 3: Implement camera interpolation**
- eased transforms:
  - `easeInOutCubic`
  - target zoom clamped (1.0–1.35)

**Step 4: Verify tests**
- Run: `npm --prefix renderer test -- camera.spec.ts`
- Expected: PASS.

### Task 7: Implement Typing Realism and Streamed Output Playback

**Files:**
- Modify: `src/capture/runner.rs`
- Modify: `renderer/src/player.ts`
- Test: `tests/typing_timeline_tests.rs`
- Test: `renderer/tests/typing_playback.spec.ts`

**Step 1: Write failing typing tests**
- per-character delays with punctuation/word pauses
- command echo and output chunk streaming

**Step 2: Implement timing model**
- configurable typing profile:
  - natural, fast, slow
- deterministic seeded jitter

**Step 3: Verify tests**
- Run: `cargo test typing_timeline_tests -- --nocapture`
- Run: `npm --prefix renderer test -- typing_playback.spec.ts`
- Expected: PASS.

### Task 8: Implement Audio Synthesis and Mix

**Files:**
- Create: `src/audio/mod.rs`
- Create: `src/audio/click_track.rs`
- Create: `src/audio/mix.rs`
- Test: `tests/audio_tests.rs`

**Step 1: Write failing audio tests**
- click track generated from keystroke events
- ffmpeg mix graph includes ducking when music exists

**Step 2: Implement audio generation**
- deterministic click samples aligned with input timestamps

**Step 3: Implement ffmpeg mix pipeline**
- typing only
- music only
- typing + music with sidechain ducking + loudnorm

**Step 4: Verify tests**
- Run: `cargo test audio_tests -- --nocapture`
- Expected: PASS.

### Task 9: Wire End-to-End ScreenStudio Renderer Mode

**Files:**
- Modify: `src/execute/mod.rs`
- Modify: `src/render/mod.rs`
- Create: `src/render/screenstudio.rs`
- Test: `tests/execute_screenstudio_tests.rs`

**Step 1: Write failing E2E pipeline test**
- run execute with `--renderer screenstudio`
- assert output video exists and metadata fields are present

**Step 2: Implement orchestrator flow**
- validate
- capture timeline
- render browser frames/video
- mux audio

**Step 3: Verify tests**
- Run: `cargo test execute_screenstudio_tests -- --nocapture`
- Expected: PASS.

### Task 10: Performance Pass (Iteration-Speed + Render-Speed)

**Files:**
- Modify: `.cargo/config.toml`
- Create: `src/render/presets.rs`
- Modify: `src/cli.rs`
- Test: `tests/perf_preset_tests.rs`

**Step 1: Write failing preset tests**
- preset defaults:
  - `draft` (fast)
  - `standard` (default)
  - `high`

**Step 2: Implement presets**
- default to 1080p30 during iteration
- enable higher quality explicitly

**Step 3: Add frame-skip and output truncation safeguards**
- prevent runaway timeline rendering on huge outputs

**Step 4: Verify tests**
- Run: `cargo test perf_preset_tests -- --nocapture`
- Expected: PASS.

### Task 11: Real Fixture E2E on `dee-wiki`

**Files:**
- Create: `fixtures/dee-wiki-script.online.json`
- Create: `fixtures/dee-wiki-script.offline.json`
- Create: `tests/e2e_dee_wiki.rs`

**Step 1: Write failing e2e test**
- chooses offline script when network probe fails
- validates `source_refs`
- produces MP4

**Step 2: Implement fixture runner**
- deterministic script selection based on probe refs

**Step 3: Verify e2e**
- Run: `cargo test e2e_dee_wiki -- --nocapture`
- Expected: PASS.

### Task 12: Documentation + Migration Cleanup

**Files:**
- Modify: `README.md`
- Modify: `SPEC.md`
- Modify: `CLAUDE.md`
- Create: `docs/SCREENSTUDIO_MODE.md`

**Step 1: Document new default**
- `--renderer screenstudio` default behavior
- agent workflow examples

**Step 2: Document quality/performance tuning**
- presets, fps, zoom params, audio toggles

**Step 3: Add troubleshooting section**
- network probe behavior
- ffmpeg missing
- browser capture dependency setup

**Step 4: Final verification**
- Run: `cargo test -- --nocapture`
- Run: `cargo check`
- Run: end-to-end command sequence on fixture target.

**Step 5: Commit**
- Commit in logical chunks per task cluster.
