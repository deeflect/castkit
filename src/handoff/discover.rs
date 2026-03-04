use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use regex::Regex;

use crate::cli::HandoffInitArgs;
use crate::handoff::types::{RefItem, RefMetadata};

pub struct DiscoveryBundle {
    pub refs: Vec<RefItem>,
    pub discovered_commands: Vec<String>,
}

pub fn discover(args: &HandoffInitArgs) -> Result<DiscoveryBundle> {
    let mut refs = Vec::new();
    let mut commands = BTreeSet::new();

    let binary = resolve_target(&args.target)?;
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
        if let Some(readme_path) = locate_readme(args.readme.as_deref())? {
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

    let files = collect_supporting_files()?;
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
    let out = std::process::Command::new(binary)
        .args(args)
        .output()
        .with_context(|| format!("failed to run {} {:?}", binary.display(), args))?;

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

fn locate_readme(explicit: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(path) = explicit {
        return Ok(Some(path.to_path_buf()));
    }

    let cwd = std::env::current_dir()?;
    for name in ["README.md", "README", "readme.md", "readme"] {
        let p = cwd.join(name);
        if p.exists() {
            return Ok(Some(p));
        }
    }

    Ok(None)
}

fn collect_supporting_files() -> Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    let cwd = std::env::current_dir()?;

    let candidates = [
        ".env.example",
        ".env.sample",
        "config.toml",
        "settings.toml",
        "castkit.toml",
    ];

    for rel in candidates {
        let p = cwd.join(rel);
        if p.exists() && p.is_file() {
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

    let command_row = Regex::new(r"^\s{2,}([a-zA-Z][a-zA-Z0-9_-]*)\s{2,}.*$").expect("regex");
    for line in text.lines() {
        if let Some(c) = command_row.captures(line) {
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
