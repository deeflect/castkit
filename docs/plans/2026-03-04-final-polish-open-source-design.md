# castkit Final Polish + OSS Readiness Design

## Goal
Ship a final polished, agent-friendly, fast demo pipeline with minimal friction for script generation and execution, and make the repository ready for open-source publication and Cargo packaging.

## Scope
- Add explicit agent contract documentation (`AGENTS.md`) for non-interactive usage.
- Improve output handling so long command output is paginated instead of truncated.
- Add easy execution presets to reduce agent decision overhead.
- Harden open-source surface: package metadata, licensing, contribution docs, release notes, and crate packaging exclusions.

## Non-Goals
- Re-architect renderer pipeline beyond v1 screenshot-based approach.
- Build hosted infrastructure or GUI.

## Design
1. Agent contract
- Provide one canonical workflow (handoff -> list/get -> script -> validate -> execute).
- Provide strict `DemoScript` JSON skeleton and hard constraints.
- Define response contract for agents (JSON only, evidence-backed steps).

2. Runtime UX and performance
- Replace hard line truncation with pagination markers in timeline generation.
- Keep rendering deterministic while preserving full output for large contexts.
- Introduce presets that set sane defaults for speed/theme/audio quality.

3. OSS/Cargo readiness
- Add Cargo metadata (`repository`, `homepage`, `documentation`, `keywords`, `categories`, `rust-version`).
- Exclude heavy generated assets from crate package (videos, caches, target).
- Add `LICENSE`, `CONTRIBUTING.md`, and `CHANGELOG.md`.

## Risks and Mitigations
- Risk: very long outputs increase render time.
- Mitigation: page markers + existing speed profiles (`fast|quality`) and docs on choosing fast mode.

## Validation
- `cargo fmt`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo package --allow-dirty`
