use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;

use crate::cli::HandoffInitArgs;
use crate::handoff::types::{RefItem, RefMetadata};

pub struct DiscoveryBundle {
    pub refs: Vec<RefItem>,
    pub discovered_commands: Vec<String>,
}

static COMMAND_ROW_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s{2,}([a-zA-Z][a-zA-Z0-9_-]*)\s{2,}.*$").expect("valid regex"));

const CAPTURE_TIMEOUT: Duration = Duration::from_secs(3);

pub fn discover(args: &HandoffInitArgs) -> Result<DiscoveryBundle> {
    let mut refs = Vec::new();
    let mut commands = BTreeSet::new();

    let binary = resolve_target(&args.target)?;
    let roots = candidate_roots(&args.target, &binary)?;
    let help_blob = collect_help(&binary);

    refs.extend(chunks_to_refs(
        "help",
        "help_chunk",
        &help_blob,
        60,
        "help",
        None,
    ));
    commands.extend(extract_commands(
        &help_blob,
        binary.file_name().and_then(|v| v.to_str()),
    ));

    if !args.no_readme {
        if let Some(readme_path) = locate_readme(args.readme.as_deref(), &roots)? {
            let readme = fs::read_to_string(&readme_path)
                .with_context(|| format!("failed to read README {}", readme_path.display()))?;
            refs.extend(chunks_to_refs(
                "readme",
                "readme_chunk",
                &readme,
                80,
                "readme",
                Some(readme_path.to_string_lossy().to_string()),
            ));
            commands.extend(extract_commands(
                &readme,
                binary.file_name().and_then(|v| v.to_str()),
            ));
        }
    }

    let files = collect_supporting_files(&roots)?;
    for (path, content) in files {
        refs.extend(chunks_to_refs(
            "files",
            "file_snippet",
            &content,
            80,
            path.file_name().and_then(|f| f.to_str()).unwrap_or("file"),
            Some(path.to_string_lossy().to_string()),
        ));
        commands.extend(extract_commands(
            &content,
            binary.file_name().and_then(|v| v.to_str()),
        ));
    }

    let probes = collect_probe_outputs(&binary);
    refs.extend(chunks_to_refs(
        "probes",
        "probe_result",
        &probes,
        60,
        "probes",
        None,
    ));

    Ok(DiscoveryBundle {
        refs,
        discovered_commands: commands.into_iter().collect(),
    })
}

fn resolve_target(target: &str) -> Result<PathBuf> {
    let candidate = PathBuf::from(target);
    if candidate.exists() {
        return Ok(candidate);
    }
    which::which(target).with_context(|| format!("failed to resolve target: {target}"))
}

fn run_capture(binary: &Path, args: &[&str]) -> Result<String> {
    let mut child = std::process::Command::new(binary)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to run {} {:?}", binary.display(), args))?;

    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if started.elapsed() >= CAPTURE_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!(
                "command timed out after {}ms: {} {:?}",
                CAPTURE_TIMEOUT.as_millis(),
                binary.display(),
                args
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    let out = child.wait_with_output().with_context(|| {
        format!(
            "failed to collect output for {} {:?}",
            binary.display(),
            args
        )
    })?;

    let mut s = String::new();
    s.push_str(&String::from_utf8_lossy(&out.stdout));
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    Ok(s)
}

fn collect_help(binary: &Path) -> String {
    let mut out = String::new();
    for args in [["--help"], ["-h"], ["help"]] {
        if let Ok(chunk) = run_capture(binary, &args) {
            if !chunk.trim().is_empty() {
                out.push_str(&format!("\n# {} {}\n", binary.display(), args.join(" ")));
                out.push_str(&chunk);
                out.push('\n');
            }
        }
    }
    out
}

fn collect_probe_outputs(binary: &Path) -> String {
    let mut out = String::new();
    for args in [["--version"], ["version"]] {
        if let Ok(chunk) = run_capture(binary, &args) {
            if !chunk.trim().is_empty() {
                out.push_str(&format!("\n# {} {}\n", binary.display(), args.join(" ")));
                out.push_str(&chunk);
                out.push('\n');
            }
        }
    }
    out
}

fn locate_readme(explicit: Option<&Path>, roots: &[PathBuf]) -> Result<Option<PathBuf>> {
    if let Some(path) = explicit {
        return Ok(Some(path.to_path_buf()));
    }

    for root in roots {
        for name in ["README.md", "README", "readme.md", "readme"] {
            let p = root.join(name);
            if p.exists() {
                return Ok(Some(p));
            }
        }
    }

    Ok(None)
}

fn collect_supporting_files(roots: &[PathBuf]) -> Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();

    let candidates = [
        ".env.example",
        ".env.sample",
        "config.toml",
        "settings.toml",
        "castkit.toml",
    ];

    for root in roots {
        for rel in candidates {
            let p = root.join(rel);
            if !p.exists() || !p.is_file() {
                continue;
            }
            let normalized = p.to_string_lossy().to_string();
            if !seen.insert(normalized) {
                continue;
            }
            let content = fs::read_to_string(&p)
                .with_context(|| format!("failed to read supporting file {}", p.display()))?;
            out.push((p, content));
        }
    }

    Ok(out)
}

fn chunks_to_refs(
    source: &str,
    kind: &str,
    text: &str,
    lines_per_chunk: usize,
    title_prefix: &str,
    path: Option<String>,
) -> Vec<RefItem> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    for (idx, chunk) in lines.chunks(lines_per_chunk).enumerate() {
        let content = chunk.join("\n");
        out.push(RefItem {
            ref_id: String::new(),
            source: source.to_string(),
            kind: kind.to_string(),
            title: Some(format!("{title_prefix} {}", idx + 1)),
            content,
            metadata: RefMetadata {
                path: path.clone(),
                line_start: Some(idx * lines_per_chunk + 1),
            },
        });
    }

    out
}

fn extract_commands(text: &str, fallback_binary: Option<&str>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();

    if let Some(bin) = fallback_binary {
        out.insert(bin.to_string());
    }

    for line in text.lines() {
        if let Some(c) = COMMAND_ROW_RE.captures(line) {
            if let Some(token) = c.get(1) {
                out.insert(token.as_str().to_string());
            }
        }

        let trimmed = line.trim();
        if trimmed.starts_with('$') || trimmed.starts_with("# ") {
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }

        let candidate = trimmed.split_whitespace().next().unwrap_or_default();
        if candidate
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
            && !candidate.starts_with('-')
            && candidate.len() < 64
            && !candidate.contains(':')
        {
            out.insert(candidate.to_string());
        }
    }

    out
}

fn candidate_roots(target: &str, binary: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();

    let cwd = std::env::current_dir()?;
    push_root(&mut out, &mut seen, cwd);

    let raw_target = PathBuf::from(target);
    if raw_target.exists() {
        if raw_target.is_dir() {
            push_root(&mut out, &mut seen, raw_target);
        } else if let Some(parent) = raw_target.parent() {
            push_root(&mut out, &mut seen, parent.to_path_buf());
        }
    }

    if let Some(parent) = binary.parent() {
        push_root(&mut out, &mut seen, parent.to_path_buf());
        let mut cursor = parent.to_path_buf();
        for _ in 0..4 {
            if let Some(next) = cursor.parent() {
                push_root(&mut out, &mut seen, next.to_path_buf());
                cursor = next.to_path_buf();
            } else {
                break;
            }
        }
    }

    Ok(out)
}

fn push_root(out: &mut Vec<PathBuf>, seen: &mut BTreeSet<String>, root: PathBuf) {
    let normalized = root.to_string_lossy().to_string();
    if seen.insert(normalized) {
        out.push(root);
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_commands, locate_readme};

    #[test]
    fn locate_readme_uses_provided_roots() {
        let dir = tempfile::tempdir().expect("tempdir");
        let readme = dir.path().join("README.md");
        std::fs::write(&readme, "hello").expect("write");
        let found = locate_readme(None, &[dir.path().to_path_buf()]).expect("locate");
        assert_eq!(found.as_deref(), Some(readme.as_path()));
    }

    #[test]
    fn extract_commands_parses_clap_like_rows() {
        let text = "Commands:\n  init   initialize project\n  run    execute workflow\n";
        let commands = extract_commands(text, Some("mycli"));
        assert!(commands.contains("mycli"));
        assert!(commands.contains("init"));
        assert!(commands.contains("run"));
    }
}
