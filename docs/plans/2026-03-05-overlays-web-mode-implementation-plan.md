# Overlays + Web Mode Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add deterministic artifact overlays for terminal demos and a new web demo mode with ScreenStudio-style camera focus, while preserving castkit's strict agent-native workflow.

**Architecture:** Extend `DemoScript` with optional `mode`, `artifacts`, and `web.actions`; validate these structures with strict enums and safety checks; branch execute pipeline by mode; emit richer transcripts; render overlays in terminal runtime and implement a dedicated web renderer runtime entrypoint.

**Tech Stack:** Rust (clap/serde/tokio/regex), Node.js Playwright, ffmpeg image2pipe, existing renderer-runtime.

---

### Task 1: Script types and schema for overlays + web mode

**Files:**
- Modify: `src/script/types.rs`
- Modify: `src/schema.rs`
- Modify: `tests/agent_cli_tests.rs` (schema assertions)
- Create: `examples/demo-script.terminal-overlay.json`
- Create: `examples/demo-script.web.json`

**Step 1: Write failing schema/type tests**
- Add tests that parse scripts with:
  - `mode: "terminal"` + step `artifacts`.
  - `mode: "web"` + `web.actions`.
- Add schema tests for new defs and enums.

**Step 2: Run test to verify it fails**
Run: `cargo test -q schema_json_is_available`
Expected: FAIL due missing schema fields.

**Step 3: Implement minimal types**
- Add enums/structs:
  - `DemoMode`
  - `StepArtifact` + `ArtifactType` + shared display config
  - `WebConfig`
  - `WebAction` + `WebActionType`
- Keep `#[serde(deny_unknown_fields)]` for strictness.

**Step 4: Implement schema changes**
- Update `demo_script_schema()`:
  - `mode` enum default terminal
  - `script_step.artifacts`
  - `web` block + action defs

**Step 5: Add examples**
- Add one terminal artifact script example.
- Add one web script example.

**Step 6: Run tests**
Run: `cargo test -q`
Expected: PASS for updated schema/tests.

**Step 7: Commit**
```bash
git add src/script/types.rs src/schema.rs tests/agent_cli_tests.rs examples/demo-script.terminal-overlay.json examples/demo-script.web.json
git commit -m "feat: add script/schema support for overlays and web mode"
```

### Task 2: Validation for artifacts and web actions

**Files:**
- Modify: `src/validate/mod.rs`
- Modify: `tests/validate_tests.rs`

**Step 1: Write failing validation tests**
- Invalid artifact position should fail.
- Invalid artifact duration should fail.
- `mode=web` without `web.actions` should fail.
- Web action requiring selector but missing selector should fail.

**Step 2: Run focused tests to see failure**
Run: `cargo test -q validate_`
Expected: FAIL for new cases.

**Step 3: Implement validator checks**
- Add mode branching checks.
- Add artifact checks (path, duration, enums).
- Add web action checks by action type.

**Step 4: Run all validation tests**
Run: `cargo test -q validate_`
Expected: PASS.

**Step 5: Commit**
```bash
git add src/validate/mod.rs tests/validate_tests.rs
git commit -m "feat: validate overlay artifacts and web action scripts"
```

### Task 3: Execute transcript model and artifact capture engine

**Files:**
- Modify: `src/execute/transcript.rs`
- Modify: `src/execute/mod.rs`
- Create: `src/execute/artifacts.rs`
- Modify: `src/lib.rs` (if response fields need expansion)
- Create: `tests/execute_artifacts_tests.rs`

**Step 1: Write failing tests**
- Step with image artifact creates overlay event.
- Missing artifact file yields execution failure with clear reason.

**Step 2: Run failing tests**
Run: `cargo test -q execute_artifacts`
Expected: FAIL.

**Step 3: Implement artifact capture module**
- `capture_artifacts(step, cwd, record, now_t_ms)` returns artifact events.
- Implement `image` verification and staging.
- Implement `result_card` payload event.
- Add stubbed/guarded `web_snapshot` capture hook.

**Step 4: Integrate into execute flow**
- After successful step, collect artifact events.
- Persist events into transcript.

**Step 5: Run tests**
Run: `cargo test -q`
Expected: PASS.

**Step 6: Commit**
```bash
git add src/execute/transcript.rs src/execute/mod.rs src/execute/artifacts.rs tests/execute_artifacts_tests.rs
git commit -m "feat: capture artifact overlays during terminal execution"
```

### Task 4: Terminal renderer overlay support

**Files:**
- Modify: `src/render/screenstudio.rs`
- Modify: `renderer-runtime/render.mjs`
- Create: `tests/render_overlay_tests.rs` (or unit tests in `screenstudio.rs`)

**Step 1: Write failing tests**
- Manifest includes overlay events.
- Overlay timing order is stable.

**Step 2: Run failing tests**
Run: `cargo test -q render_`
Expected: FAIL.

**Step 3: Rust manifest integration**
- Add `overlay_events` to render manifest.
- Map transcript artifact events to renderer events.

**Step 4: JS overlay rendering**
- Add overlay DOM layer.
- Implement event-driven fade/slide animation.
- Respect position + duration.

**Step 5: Validate by running sample**
Run: `cargo run -- execute ... --script examples/demo-script.terminal-overlay.json --non-interactive --output /tmp/overlay-demo.mp4`
Expected: overlay appears in output video.

**Step 6: Commit**
```bash
git add src/render/screenstudio.rs renderer-runtime/render.mjs
git commit -m "feat: render terminal overlay artifacts"
```

### Task 5: Web mode execute runner (Playwright actions)

**Files:**
- Create: `src/execute/web_runner.rs`
- Modify: `src/execute/mod.rs`
- Create: `renderer-runtime/web-runner.mjs`
- Create: `tests/execute_web_tests.rs`

**Step 1: Write failing tests**
- `mode=web` dispatches to web runner.
- Action transcript is produced with expected order.

**Step 2: Run failing tests**
Run: `cargo test -q execute_web`
Expected: FAIL.

**Step 3: Implement Rust web runner integration**
- Spawn node web runner with JSON script payload/path.
- Capture action trace JSON artifact.

**Step 4: Implement Playwright web runner script**
- Execute supported actions deterministically.
- Emit records:
  - action id/type
  - timestamp
  - target box
  - cursor position
  - optional screenshot path

**Step 5: Run tests**
Run: `cargo test -q execute_web`
Expected: PASS.

**Step 6: Commit**
```bash
git add src/execute/web_runner.rs src/execute/mod.rs renderer-runtime/web-runner.mjs tests/execute_web_tests.rs
git commit -m "feat: add deterministic web action execution runner"
```

### Task 6: Web renderer with cursor + focus zoom

**Files:**
- Create: `src/render/webstudio.rs`
- Modify: `src/render/mod.rs`
- Create: `renderer-runtime/render-web.mjs`
- Modify: `src/cli.rs` (optional mode/preset flags if needed)
- Create: `tests/render_web_tests.rs`

**Step 1: Write failing tests**
- Render dispatcher selects web renderer for `mode=web` transcript.
- Web manifest serializes action keyframes.

**Step 2: Run failing tests**
Run: `cargo test -q render_web`
Expected: FAIL.

**Step 3: Implement web manifest + renderer call**
- Build compact manifest from web trace.
- Node renderer composes camera motion from target boxes.

**Step 4: Implement visual effects**
- Cursor path interpolation.
- Click pulse.
- Zoom-to-target with easing.
- Branding/watermark consistency with terminal renderer.

**Step 5: Run end-to-end sample**
Run: `cargo run -- execute --session <id> --script examples/demo-script.web.json --non-interactive --output /tmp/web-demo.mp4 --json`
Expected: mp4 with web actions and smooth focus motion.

**Step 6: Commit**
```bash
git add src/render/webstudio.rs src/render/mod.rs renderer-runtime/render-web.mjs src/cli.rs tests/render_web_tests.rs
git commit -m "feat: add web-mode screenstudio renderer"
```

### Task 7: Docs, contract updates, and release-readiness checks

**Files:**
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `docs/OVERVIEW.md`
- Modify: `src/agent_contract.rs`
- Create: `docs/plans/2026-03-05-overlays-web-mode-design.md` (already created)

**Step 1: Update docs**
- Add terminal overlay authoring guide.
- Add web mode authoring guide.
- Add "good scenario" guidance for mixed CLI+result storytelling.

**Step 2: Update machine contract**
- Add runtime hints for `mode`, `artifacts`, `web.actions`.

**Step 3: Validate docs examples**
- Ensure commands and flags match actual CLI.

**Step 4: Run full quality gates**
Run:
- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test -q`

Expected: all pass.

**Step 5: Commit**
```bash
git add README.md AGENTS.md docs/OVERVIEW.md src/agent_contract.rs
git commit -m "docs: add overlays and web mode agent guidance"
```

## Execution Order Recommendation
1. Task 1
2. Task 2
3. Task 3
4. Task 4
5. Task 5
6. Task 6
7. Task 7

## Risk Notes
- Playwright in CI environments may need browser install step before web tests.
- Web mode can introduce flakiness without strict waits/selectors; keep action grammar constrained.
- Overlay image loading can bloat manifest if not cached/resized.

## Definition of Done
- Terminal demos can show image/result/chart/web snapshot overlays via schema-defined artifacts.
- Web mode script validates, executes, and renders with focus zoom and cursor motion.
- Existing terminal scripts continue working unchanged.
- Docs and agent contract are updated and tested.
