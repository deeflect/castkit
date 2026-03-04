# castkit Agent-Native Handoff Design (v1)

Date: 2026-03-04
Status: proposed

## Goal

Make castkit the execution engine for agents:

- agent gets complete project context through structured retrieval
- agent writes a strict demo script
- castkit validates script against discovered evidence
- castkit executes non-interactively and renders a polished video

The system must prevent agent invention and keep outputs machine-friendly.

## Design Principles

1. No truncation of authoritative inputs (help, README, config docs).
2. Retrieval over dumping: indexed content + pagination + reference IDs.
3. Strict JSON contracts for all agent IO.
4. Validation must reject unsupported or unevidenced steps.
5. Deterministic execution and deterministic failure outputs.

## User-Facing CLI (Agent Contract)

### 1) Create Handoff Session

```bash
castkit handoff init <target> --json
```

Creates a session and discovers:

- CLI help surfaces (`--help`, `-h`, `help`)
- README sections and code blocks
- nearby config/env templates (`.env.example`, config files)
- command graph and option graph
- setup prerequisites and probe checks

Returns:

```json
{
  "session_id": "sess_01J...",
  "target": "./mycli",
  "sources": [
    {"source":"help","pages":4},
    {"source":"readme","pages":6},
    {"source":"files","pages":2}
  ],
  "refs_index_id": "idx_01J..."
}
```

### 2) Paginated Source Listing

```bash
castkit handoff list --session <id> --source help|readme|files --page N --per-page M --json
```

Returns chunk metadata without losing fidelity:

```json
{
  "session_id": "sess_01J...",
  "source": "readme",
  "page": 2,
  "per_page": 20,
  "total_pages": 6,
  "items": [
    {
      "ref_id": "ref_readme_0021",
      "kind": "code_block",
      "title": "Quick Start",
      "byte_len": 812,
      "preview": "mycli init demo..."
    }
  ]
}
```

### 3) Exact Retrieval by Ref

```bash
castkit handoff get --session <id> --ref <ref_id> --json
```

Returns exact content:

```json
{
  "ref_id": "ref_readme_0021",
  "source": "readme",
  "kind": "code_block",
  "content": "mycli init demo\nmycli deploy --dry-run\n",
  "metadata": {"path":"README.md","line_start":47}
}
```

### 4) Validate Agent Script

```bash
castkit validate --session <id> --script demo.json --json
```

### 5) Execute

```bash
castkit execute --session <id> --script demo.json --non-interactive --output demo.mp4 --json
```

## DemoScript Schema (Strict JSON)

```json
{
  "version": "1",
  "setup": [
    {
      "id": "copy_env",
      "run": "cp .env.example .env",
      "expect": {"exit_code": 0},
      "timeout_ms": 5000,
      "source_refs": ["ref_files_0004"]
    }
  ],
  "scenes": [
    {
      "id": "init",
      "title": "Initialize a new project",
      "steps": [
        {
          "id": "init_cmd",
          "run": "mycli init demo",
          "expect": {"contains": "Created project"},
          "timeout_ms": 15000,
          "source_refs": ["ref_help_0012", "ref_readme_0021"]
        }
      ]
    }
  ],
  "checks": [
    {
      "id": "env_exists",
      "run": "test -f .env",
      "expect": {"exit_code": 0},
      "timeout_ms": 3000,
      "source_refs": ["ref_files_0004"]
    }
  ],
  "cleanup": [],
  "redactions": [{"pattern": "API_KEY=.*"}],
  "audio": {"typing": true, "music_path": null}
}
```

## Hard Validation Rules (Anti-Invention)

`castkit validate` fails if any rule is violated:

1. Any `setup`, `steps`, or `checks` entry missing non-empty `source_refs`.
2. Any `source_refs` that do not exist in session index.
3. Any command token not found in discovered command graph, unless explicitly marked:
   - `"manual_step": true`
   - with `"manual_reason"` and at least one `source_ref` proving context.
4. Broken ordering dependencies:
   - references to files/env not created yet
   - scene uses command requiring setup that is absent
5. Unsafe secret handling:
   - inline secret literals in `run`
   - unsafe env writes without redaction coverage
6. Unknown schema fields (strict parser).

## Validation Output Format

```json
{
  "ok": false,
  "errors": [
    {
      "code": "MISSING_SOURCE_REFS",
      "path": "scenes[0].steps[1]",
      "message": "step must include at least one source_ref"
    },
    {
      "code": "UNKNOWN_COMMAND",
      "path": "scenes[1].steps[0].run",
      "message": "command 'mycli bootstrap' not found in discovered graph",
      "hint": "mark as manual_step with manual_reason and supporting refs"
    }
  ]
}
```

## Execution Behavior

`castkit execute`:

1. re-validates script (same strict rules)
2. runs setup/checks/scenes in sandbox by default
3. records PTY state for render
4. applies redaction
5. renders polished output with auto-zoom
6. optionally mixes typing audio/music
7. emits machine-readable events and final artifact paths

## Why This Is LLM-Friendly

1. Agent gets full fidelity context through pagination/ref fetch.
2. Agent cannot hallucinate unsupported flows without validation errors.
3. Error outputs are compact and repairable by another agent turn.
4. No giant prose blobs required; everything is structured JSON.

## Out of Scope for v1

1. Natural-language script formats.
2. Automatic planner inside castkit.
3. Browser workflows.
4. Interactive TUI planning mode.

## Next Step

Integrate this contract into `SPEC.md` as the default mode:

- keep internal discover/plan/record/render stages
- expose only `handoff + validate + execute` as primary agent interface
