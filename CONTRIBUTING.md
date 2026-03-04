# Contributing

## Setup
1. Install Rust 1.75+ and Node 20+.
2. Install renderer runtime dependencies:
```bash
npm install --prefix renderer-runtime
npx --prefix renderer-runtime playwright install chromium
```
3. Ensure `ffmpeg` is in `PATH`.

## Development Commands
- Format: `cargo fmt`
- Tests: `cargo test`
- Lints: `cargo clippy --all-targets --all-features -- -D warnings`
- Build: `cargo build`

## Pull Request Checklist
- Include tests for behavior changes.
- Update `README.md` and `AGENTS.md` if agent workflow or CLI flags change.
- Keep generated binaries/videos out of source changes where possible.
- Follow `CODE_OF_CONDUCT.md` and `SECURITY.md`.

## Style
- Keep CLI behavior deterministic and non-interactive-first.
- Avoid invented defaults that are not documented.
- Prefer small, composable changes.
