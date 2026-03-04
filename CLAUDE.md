# CLAUDE.md — castkit

## What is this?
castkit is an agent-native CLI that turns a strict, evidence-backed script into a polished terminal demo video.

## External contract

`handoff init/list/get` -> agent `DemoScript` JSON -> `validate` -> `execute`

## Internal flow

`discover` -> `validate` -> `execute` -> `capture transcript` -> `screenstudio render manifest` -> `playwright render` -> `ffmpeg mux`

## Quality goals

- ScreenStudio-like polished terminal frame
- smooth activity-following camera zoom
- typed command playback
- output streaming feel
- optional typing clicks and optional background music

## Hard safety/quality constraints

- strict JSON schema (`deny_unknown_fields`)
- every executable step must include valid `source_refs`
- unsupported command invention fails validation unless explicit `manual_step`
- non-interactive deterministic execution path

## Runtime dependencies

- Rust + ffmpeg
- Node + Playwright Chromium runtime in `renderer-runtime/`

Setup:

```bash
npm install --prefix renderer-runtime
npx --prefix renderer-runtime playwright install chromium
```

## Current status

- ScreenStudio renderer is the only execute path.
- Legacy renderer has been removed from CLI flow.
- Verified fixture output exists at `/tmp/deewiki_demo_v4.mp4`.
