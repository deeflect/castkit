# castkit Agent Contract (v1)

This file defines how an LLM agent should use castkit in non-interactive mode.

## Goal
Produce a polished terminal demo video from evidence-backed steps, with no invented commands.

## Hard Rules
- Never invent executable commands, flags, file paths, or setup steps.
- Every executable step must include non-empty `source_refs` from the active handoff session.
- Validate before execute. If validation fails, fix the script and re-validate.
- Prefer `manual_step=true` only when no runnable command exists in evidence.
- Keep output deterministic: non-interactive only (`--non-interactive`).
- Treat bootstrap contract/schema commands as planning context, not main demo scenes.

## Bootstrap First (required)
Do this before generating or validating any script:
1. Load machine contract:
```bash
castkit --json agent contract
```
2. Load machine schema:
```bash
castkit --json schema
```
3. Use returned `contract_version` and schema as runtime source of truth.

## Required Flow
1. Initialize handoff session:
```bash
castkit handoff init <target_binary_or_path> --json
```
2. Discover refs with pagination (repeat for each source):
```bash
castkit handoff list --session <session_id> --source help --page 1 --per-page 20 --json
castkit handoff list --session <session_id> --source readme --page 1 --per-page 20 --json
castkit handoff list --session <session_id> --source files --page 1 --per-page 20 --json
castkit handoff list --session <session_id> --source probes --page 1 --per-page 20 --json
```
3. Fetch exact refs you plan to cite:
```bash
castkit handoff get --session <session_id> --ref <ref_id> --json
```
4. Optional scaffold:
```bash
castkit plan scaffold --session <session_id> --output demo-script.json --max-scenes 3 --json
```
5. Write/refine `DemoScript` JSON.
6. Validate:
```bash
castkit validate --session <session_id> --script demo.json --json
```
7. Execute + render:
```bash
castkit execute --session <session_id> --script demo.json --non-interactive --preset polished --output demo.mp4 --json
```

Human-readable contract:
```bash
castkit agent contract
```

## Runtime Variables During Execute
These are injected automatically for every step:
- `SESSION`
- `CASTKIT_SESSION`

Both start as the value passed to `castkit execute --session <id>`.
If a step prints JSON containing `"session_id"`, castkit updates both variables for subsequent steps.

## Agent Output Contract (for script generation)
Return script as raw JSON only (no markdown), strictly matching the schema below.

## DemoScript Schema (strict)
```json
{
  "version": "1",
  "setup": [
    {
      "id": "setup_01",
      "run": "<command>",
      "expect": {
        "contains": null,
        "regex": null,
        "exit_code": 0
      },
      "timeout_ms": 120000,
      "source_refs": ["ref_help_0001"],
      "manual_step": false,
      "manual_reason": null
    }
  ],
  "scenes": [
    {
      "id": "scene_01",
      "title": "<human-readable scene title>",
      "steps": [
        {
          "id": "step_01",
          "run": "<command>",
          "expect": {
            "contains": null,
            "regex": null,
            "exit_code": 0
          },
          "timeout_ms": 120000,
          "source_refs": ["ref_readme_0003"],
          "manual_step": false,
          "manual_reason": null
        }
      ]
    }
  ],
  "checks": [],
  "cleanup": [],
  "redactions": [],
  "audio": {
    "typing": true,
    "music_path": null
  },
  "branding": {
    "title": "castkit demo",
    "watermark_text": "castkit.com"
  }
}
```

## Scenario Quality Rubric
- Scene progression should tell a product story, not just `--help` output.
- Include meaningful feature coverage: init/config, core workflow, result verification.
- Prefer short commands with observable output.
- Keep each scene focused (2-5 steps).
- Include at least one check that verifies final state.

## Scenario Design Playbook (always apply)
Design each demo around one clear promise:
- "By the end, the user sees `<outcome>` working in `<target_time>`."

Build scenes in this order:
1. `Setup trust`: show environment/config is correct (`.env`, config files, auth status).
2. `Happy path`: run the primary workflow end-to-end.
3. `Power move`: show one advanced/high-value feature.
4. `Proof`: verify output/state with explicit checks.
5. `Wrap`: summarize artifact/result (file created, record updated, service running).

Recommended scene count:
- 3 scenes for short demos.
- 4-5 scenes for medium demos.

## What To Show
- Real user outcome, not only command catalogs.
- Before/after state (input then transformed result).
- At least one "why this matters" moment (speed, clarity, reliability, automation).
- Concrete outputs: generated files, structured JSON, test results, status commands.

Avoid:
- Long `--help` dumps as the main content.
- Full `castkit --json agent contract` or `castkit --json schema` dumps inside showcase scenes.
- Repetitive commands with low information value.
- Setup-heavy intros with no visible payoff.

## How To Show It (command style)
- Keep commands short and readable; split complex flows into multiple steps.
- Prefer deterministic commands with stable output.
- Use `expect.contains` or `expect.regex` for each meaningful step.
- Add short setup steps before commands that depend on config/env.
- Use `checks` for final proof, not only in-scene assumptions.

Command writing tips:
- Good: one intent per step, visible stdout, easy-to-verify result.
- Bad: chained opaque shell one-liners that hide what changed.

## Script Writing Guidelines
- Scene titles should be outcome-first (`"Generate typed client from schema"`), not vague (`"Run command"`).
- Step IDs should be stable and descriptive (`setup_env`, `build_bundle`, `verify_output`).
- Use `manual_step=true` only for genuinely non-runnable actions and provide a concrete `manual_reason`.
- Keep total runnable steps lean:
  - short demo: 8-14 steps
  - medium demo: 12-24 steps

## Pacing Guidelines
- First meaningful value should appear within 20-35 seconds of video time.
- Every scene should have a visible output event.
- If a command is noisy, prefer filtered/targeted variants so key lines are visible.
- End with explicit verification (not just "command succeeded").

## Pre-Execute Quality Gate
Before `validate`, confirm all are true:
- Each runnable step has valid `source_refs`.
- No scene is pure setup without user-visible outcome.
- At least one final-state check exists in `checks`.
- No step depends on hidden state that was not established in `setup`.
- Titles and command sequence tell a coherent story from problem to proof.

## Easy Settings
Use one preset for simplicity:
- `--preset quick`: fastest iteration and lower encode cost.
- `--preset balanced`: better quality with moderate speed.
- `--preset polished`: highest default polish for showcase videos.

You can still override with explicit flags (`--speed`, `--fps`, `--theme`, `--keystroke-profile`).

## Completion Contract (machine-readable)
Treat each CLI call as complete only when both are true:
1. Process exit code is `0`.
2. JSON response has `"ok": true`.

For `validate`, completion means:
- `"ok": true`
- no validation errors.

For `execute`, completion means:
- `"ok": true`
- `"output"` points to the generated file.
- `"transcript_path"` exists.
- `"render"` exists with duration/paths.

If `execute.ok` is `false`:
- If `failures` contains step-level errors, revise script and rerun `validate`.
- If failure is infra/runtime (missing ffmpeg/node/playwright), fix environment and rerun.

## Agent Feedback Output (what to report upstream)
After `execute`, return this summary to caller:
- `status`: `success` or `failed`
- `session_id`
- `output`
- `duration_secs` (from `render.duration_secs` if present)
- `failed_step` (first failure path, if any)
- `next_action` (`done`, `fix-script`, `fix-environment`)

## Time Budget Guidance (timeouts + polling)
Use broad, safe defaults; do not assume high-end hardware.

Recommended polling interval:
- every `20s` while `execute` is running.

Recommended timeout policy:
- soft timeout: `8m` (emit warning, keep running)
- hard timeout: `20m` (mark failed and stop)

Approximate end-to-end `execute` times:
- Short demo (20-45s output):
  - `quick`: `~45-120s`
  - `balanced`: `~90-210s`
  - `polished`: `~120-300s`
- Medium demo (60-120s output):
  - `quick`: `~2-5m`
  - `balanced`: `~3-7m`
  - `polished`: `~4-10m`
- Long demo (3-5 min output):
  - `quick`: `~6-12m`
  - `balanced`: `~8-16m`
  - `polished`: `~10-20m`

If unknown, use this hard-timeout heuristic:
- `hard_timeout_minutes = max(10, ceil(video_minutes * 4))`, capped at `20`.

Example watchdog loop:
```bash
castkit execute --session "$SESSION" --script demo.json --non-interactive --preset polished --output demo.mp4 --json > execute.json &
PID=$!
START=$(date +%s)
SOFT=480   # 8m
HARD=1200  # 20m
while kill -0 "$PID" 2>/dev/null; do
  NOW=$(date +%s)
  ELAPSED=$((NOW - START))
  if [ "$ELAPSED" -ge "$HARD" ]; then
    kill -TERM "$PID" 2>/dev/null || true
    echo "hard-timeout"
    break
  fi
  if [ "$ELAPSED" -ge "$SOFT" ]; then
    echo "soft-timeout-warning"
  fi
  sleep 20
done
wait "$PID"
```
