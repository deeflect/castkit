pub mod discover;
pub mod session_store;
pub mod types;

use anyhow::{anyhow, Result};
use chrono::Utc;

use crate::cli::{HandoffGetArgs, HandoffInitArgs, HandoffListArgs};
use crate::handoff::discover::discover;
use crate::handoff::session_store::{load_session, save_session};
use crate::handoff::types::{
    HandoffSession, InitResponse, ListResponse, RefItem, RefListItem, SourceSummary,
};

pub async fn init_session(args: HandoffInitArgs) -> Result<InitResponse> {
    let discovery = discover(&args)?;
    let session_id = format!("sess_{}", uuid::Uuid::new_v4().simple());
    let refs_index_id = format!("idx_{}", uuid::Uuid::new_v4().simple());

    let refs = assign_ref_ids(discovery.refs);

    let source_summaries = summarize_sources(&refs);

    let session = HandoffSession {
        session_id: session_id.clone(),
        target: args.target,
        created_at: Utc::now(),
        sources: source_summaries.clone(),
        refs_index_id: refs_index_id.clone(),
        refs,
        discovered_commands: discovery.discovered_commands,
    };

    save_session(&session)?;

    Ok(InitResponse {
        session_id,
        target: session.target,
        sources: source_summaries,
        refs_index_id,
    })
}

pub fn list_refs(args: HandoffListArgs) -> Result<ListResponse> {
    let session = load_session(&args.session)?;
    let source = format!("{:?}", args.source).to_lowercase();
    let mut refs: Vec<&RefItem> = session
        .refs
        .iter()
        .filter(|item| item.source.eq_ignore_ascii_case(&source))
        .collect();

    refs.sort_by(|a, b| a.ref_id.cmp(&b.ref_id));

    let per_page = args.per_page.max(1);
    let total_pages = refs.len().div_ceil(per_page).max(1);
    let page = args.page.clamp(1, total_pages);
    let start = (page - 1) * per_page;
    let end = (start + per_page).min(refs.len());

    let items = refs[start..end]
        .iter()
        .map(|item| RefListItem {
            ref_id: item.ref_id.clone(),
            kind: item.kind.clone(),
            title: item.title.clone(),
            byte_len: item.content.len(),
            preview: preview(&item.content),
        })
        .collect();

    Ok(ListResponse {
        session_id: session.session_id,
        source,
        page,
        per_page,
        total_pages,
        items,
    })
}

pub fn get_ref(args: HandoffGetArgs) -> Result<RefItem> {
    let session = load_session(&args.session)?;
    session
        .refs
        .into_iter()
        .find(|item| item.ref_id == args.r#ref)
        .ok_or_else(|| anyhow!("ref not found: {}", args.r#ref))
}

fn summarize_sources(refs: &[RefItem]) -> Vec<SourceSummary> {
    let mut by_source = std::collections::BTreeMap::<String, usize>::new();
    for item in refs {
        *by_source.entry(item.source.clone()).or_insert(0) += 1;
    }

    by_source
        .into_iter()
        .map(|(source, pages)| SourceSummary { source, pages })
        .collect()
}

fn assign_ref_ids(mut refs: Vec<RefItem>) -> Vec<RefItem> {
    let mut per_source = std::collections::BTreeMap::<String, usize>::new();

    for item in &mut refs {
        let source = item.source.clone();
        let next = per_source.entry(source.clone()).or_insert(0);
        *next += 1;
        item.ref_id = format!("ref_{}_{:04}", source, *next);
    }

    refs
}

fn preview(s: &str) -> String {
    let mut p = s.lines().next().unwrap_or_default().trim().to_string();
    if p.len() > 120 {
        p.truncate(117);
        p.push_str("...");
    }
    p
}
