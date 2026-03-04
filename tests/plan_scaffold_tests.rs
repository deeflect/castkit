use castkit::cli::PlanScaffoldArgs;
use castkit::handoff::session_store::save_session;
use castkit::handoff::types::{HandoffSession, RefItem, RefMetadata, SourceSummary};
use castkit::plan;
use castkit::script::parse_script;
use chrono::Utc;

fn seed_session(session_id: &str) {
    let session = HandoffSession {
        session_id: session_id.to_string(),
        target: "mycli".to_string(),
        created_at: Utc::now(),
        sources: vec![
            SourceSummary {
                source: "help".to_string(),
                pages: 1,
            },
            SourceSummary {
                source: "files".to_string(),
                pages: 1,
            },
        ],
        refs_index_id: "idx_x".to_string(),
        refs: vec![
            RefItem {
                ref_id: "ref_help_0001".to_string(),
                source: "help".to_string(),
                kind: "help_chunk".to_string(),
                title: Some("help".to_string()),
                content: "mycli init\nmycli run".to_string(),
                metadata: RefMetadata {
                    path: None,
                    line_start: Some(1),
                },
            },
            RefItem {
                ref_id: "ref_files_0001".to_string(),
                source: "files".to_string(),
                kind: "file_snippet".to_string(),
                title: Some(".env.example".to_string()),
                content: "API_KEY=abc".to_string(),
                metadata: RefMetadata {
                    path: Some(".env.example".to_string()),
                    line_start: Some(1),
                },
            },
        ],
        discovered_commands: vec!["mycli".to_string(), "init".to_string(), "run".to_string()],
    };
    save_session(&session).expect("save session");
}

#[test]
fn scaffold_generates_valid_script_file() {
    let session_id = format!("sess_test_{}", uuid::Uuid::new_v4().simple());
    seed_session(&session_id);

    let temp = tempfile::tempdir().expect("tempdir");
    let output = temp.path().join("demo-script.json");
    let response = plan::scaffold(PlanScaffoldArgs {
        session: session_id,
        output: output.clone(),
        max_scenes: 3,
    })
    .expect("scaffold");

    assert!(response.ok);
    assert!(output.exists());
    let body = std::fs::read_to_string(output).expect("read");
    let script = parse_script(&body).expect("parse");
    assert!(!script.scenes.is_empty());
    assert!(script
        .scenes
        .iter()
        .flat_map(|s| s.steps.iter())
        .all(|step| !step.source_refs.is_empty()));
}
