# Castkit Agent-Native MVP Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build an end-to-end Rust CLI that supports handoff discovery/retrieval, strict script validation, deterministic non-interactive execution, and MP4 rendering with optional audio.

**Architecture:** Keep the internal stage model (discover, plan-normalize, execute/record, render) but expose only the agent-native contract (`handoff init/list/get`, `validate`, `execute`). Persist handoff sessions as JSON on disk with stable refs; require evidence-backed script steps via strict validation.

**Tech Stack:** Rust, clap, serde/serde_json, tokio (process/timeouts), regex, uuid, ffmpeg process invocation, tempfile.

---

### Task 1: Bootstrap crate and command surface

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `src/cli.rs`

**Step 1: Write failing parse tests**
- Add CLI parser tests for `handoff init/list/get`, `validate`, `execute`.

**Step 2: Run tests and confirm failures**
- Run: `cargo test cli::tests -- --nocapture`
- Expected: unresolved modules / missing command handlers.

**Step 3: Implement clap command tree**
- Add full clap enums/args for agent contract and shared options.

**Step 4: Run tests to pass**
- Run: `cargo test cli::tests -- --nocapture`
- Expected: PASS.

### Task 2: Implement handoff sessions with pagination and refs

**Files:**
- Create: `src/handoff/mod.rs`
- Create: `src/handoff/discover.rs`
- Create: `src/handoff/session_store.rs`
- Create: `src/handoff/types.rs`
- Test: `tests/handoff_integration.rs`

**Step 1: Write failing handoff tests**
- Cover session creation, paginated list, and get-by-ref.

**Step 2: Implement discovery + indexing**
- Collect help, README, and file snippets.
- Build stable `ref_id`s and session JSON storage.

**Step 3: Verify handoff tests**
- Run: `cargo test handoff -- --nocapture`
- Expected: PASS.

### Task 3: Implement strict DemoScript schema + validator

**Files:**
- Create: `src/script/mod.rs`
- Create: `src/script/types.rs`
- Create: `src/validate/mod.rs`
- Create: `src/validate/errors.rs`
- Test: `tests/validate_tests.rs`

**Step 1: Write failing validator tests**
- Missing `source_refs`
- Invalid refs
- Unknown command without `manual_step`
- Unknown command with valid `manual_step`

**Step 2: Implement strict parsing and validation**
- `deny_unknown_fields` on schema.
- return machine-readable errors with code/path/message.

**Step 3: Verify validator tests**
- Run: `cargo test validate -- --nocapture`
- Expected: PASS.

### Task 4: Implement non-interactive executor and transcript capture

**Files:**
- Create: `src/execute/mod.rs`
- Create: `src/execute/runner.rs`
- Create: `src/execute/transcript.rs`
- Test: `tests/execute_tests.rs`

**Step 1: Write failing execution tests**
- Script steps execute in order.
- Timeout behavior.
- Re-validation is enforced.

**Step 2: Implement command runner**
- Run setup/checks/scenes/cleanup in sandbox shell.
- Capture stdout/stderr + timing in transcript.

**Step 3: Verify execution tests**
- Run: `cargo test execute -- --nocapture`
- Expected: PASS.

### Task 5: Implement MVP renderer and optional audio mix

**Files:**
- Create: `src/render/mod.rs`
- Create: `src/render/ffmpeg.rs`
- Modify: `src/execute/mod.rs`
- Test: `tests/render_tests.rs`

**Step 1: Write failing render test**
- Ensures ffmpeg args include output dimensions, fps, and optional audio inputs.

**Step 2: Implement render pipeline**
- Generate transcript text artifact.
- Spawn ffmpeg with gradient/bg + terminal-like overlay text.
- Optional typing/music mix when provided.

**Step 3: Verify render tests**
- Run: `cargo test render -- --nocapture`
- Expected: PASS.

### Task 6: Wire commands end-to-end + docs updates

**Files:**
- Modify: `src/main.rs`
- Modify: `src/lib.rs`
- Modify: `README.md` (create if missing)

**Step 1: Hook subcommands to module APIs**
- `handoff init/list/get`
- `validate`
- `execute`

**Step 2: Add JSON responses and NDJSON-style event output**
- Ensure machine-readable outputs are consistent.

**Step 3: End-to-end verification**
- Run: `cargo test -- --nocapture`
- Run: `cargo check`
- Run manual command sequence with fixture target.

**Step 4: Commit**
- Commit in logical chunks after passing checks.
