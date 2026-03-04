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
4. Write `DemoScript` JSON.
5. Validate:
```bash
castkit validate --session <session_id> --script demo.json --json
```
6. Execute + render:
```bash
castkit execute --session <session_id> --script demo.json --non-interactive --preset polished --output demo.mp4 --json
```

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

## Easy Settings
Use one preset for simplicity:
- `--preset quick`: fastest iteration and lower encode cost.
- `--preset balanced`: better quality with moderate speed.
- `--preset polished`: highest default polish for showcase videos.

You can still override with explicit flags (`--speed`, `--fps`, `--theme`, `--keystroke-profile`).
