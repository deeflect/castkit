# Publishing Runbook

This project is prepared for open-source release and Cargo publication.

## Pre-publish checklist
- Confirm final crate metadata in `Cargo.toml`:
  - `repository`
  - `homepage`
  - `documentation`
- Confirm legal files are present and accurate:
  - `LICENSE`
  - `CONTRIBUTING.md`
  - `CHANGELOG.md`
- Confirm docs are current:
  - `README.md`
  - `AGENTS.md`
  - `docs/OVERVIEW.md`

## Local release preflight
Run:
```bash
./scripts/release-ready.sh
```

This performs:
- metadata sanity warnings (missing `repository` / `homepage`)
- formatting check
- clippy lint gate
- tests
- package verification
- cargo publish dry-run
- renderer syntax check

## CI release-readiness
Manual GitHub Action:
- Workflow: `Release Readiness`
- Trigger: `workflow_dispatch`

## Final publish command (when ready)
```bash
cargo publish
```

Do not run until metadata and ownership are finalized.
