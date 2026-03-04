# castkit Final Polish + OSS Readiness Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Finalize castkit with agent-first docs, easier settings, long-output pagination, and Cargo publish readiness.

**Architecture:** Keep the existing strict handoff/validate/execute pipeline, add low-risk CLI and renderer timeline improvements, and complete repository packaging/documentation metadata.

**Tech Stack:** Rust (clap/serde/tokio), Node Playwright renderer, ffmpeg.

---

### Task 1: Add easy execute presets

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/execute/mod.rs`
- Modify: `tests/execute_tests.rs`

1. Add CLI `--preset` enum to `execute` command.
2. Map preset to effective speed/theme/keystroke/fps defaults.
3. Keep explicit CLI args as final override.
4. Add unit test for preset mapping behavior.

### Task 2: Replace output truncation with pagination

**Files:**
- Modify: `src/render/screenstudio.rs`

1. Remove hard output line truncation.
2. Add page markers for long wrapped output segments.
3. Preserve all output content while keeping smooth timeline pacing.
4. Add/adjust unit tests for pagination helper behavior.

### Task 3: Add agent contract docs

**Files:**
- Create: `AGENTS.md`
- Modify: `README.md`
- Create: `examples/demo-script.template.json`

1. Document exact non-interactive flow and output contract.
2. Add strict JSON template and constraints.
3. Link agent docs from README quick start.

### Task 4: OSS and Cargo readiness

**Files:**
- Modify: `Cargo.toml`
- Create: `LICENSE`
- Create: `CONTRIBUTING.md`
- Create: `CHANGELOG.md`

1. Add package metadata and packaging exclusions.
2. Add licensing and contributor guidance.
3. Add initial changelog baseline.

### Task 5: Validate and demo

**Files:**
- N/A

1. Run `cargo fmt`.
2. Run `cargo test`.
3. Run `cargo clippy --all-targets --all-features -- -D warnings`.
4. Run `cargo package --allow-dirty`.
5. Render one example output and capture path for handoff.
