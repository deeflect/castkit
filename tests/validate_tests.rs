use castkit::handoff::session_store::save_session;
use castkit::handoff::types::{HandoffSession, RefItem, RefMetadata, SourceSummary};
use castkit::script::parse_script;
use castkit::validate::validate_script;
use chrono::Utc;

fn seed_session(session_id: &str) {
    let session = HandoffSession {
        session_id: session_id.to_string(),
        target: "mycli".to_string(),
        created_at: Utc::now(),
        sources: vec![SourceSummary {
            source: "help".to_string(),
            pages: 1,
        }],
        refs_index_id: "idx_x".to_string(),
        refs: vec![RefItem {
            ref_id: "ref_help_0001".to_string(),
            source: "help".to_string(),
            kind: "help_chunk".to_string(),
            title: Some("help".to_string()),
            content: "mycli init\nmycli run".to_string(),
            metadata: RefMetadata {
                path: None,
                line_start: Some(1),
            },
        }],
        discovered_commands: vec!["mycli".to_string(), "init".to_string(), "run".to_string()],
    };
    save_session(&session).expect("save session");
}

#[test]
fn validate_fails_missing_source_refs() {
    let session_id = format!("sess_test_{}", uuid::Uuid::new_v4().simple());
    seed_session(&session_id);

    let script = parse_script(
        r#"{
      "version":"1",
      "setup":[],
      "scenes":[{"id":"s1","title":"t","steps":[{"id":"a","run":"mycli init demo","expect":null,"timeout_ms":1000,"source_refs":[]}]}],
      "checks":[],
      "cleanup":[],
      "redactions":[],
      "audio":null
    }"#,
    )
    .expect("parse");

    let res = validate_script(&session_id, &script).expect("validate");
    assert!(!res.ok);
    assert!(res.errors.iter().any(|e| e.code == "MISSING_SOURCE_REFS"));
}

#[test]
fn validate_fails_unknown_command_without_manual_step() {
    let session_id = format!("sess_test_{}", uuid::Uuid::new_v4().simple());
    seed_session(&session_id);

    let script = parse_script(
        r#"{
      "version":"1",
      "setup":[],
      "scenes":[{"id":"s1","title":"t","steps":[{"id":"a","run":"unknowncmd do","expect":null,"timeout_ms":1000,"source_refs":["ref_help_0001"]}]}],
      "checks":[],
      "cleanup":[],
      "redactions":[],
      "audio":null
    }"#,
    )
    .expect("parse");

    let res = validate_script(&session_id, &script).expect("validate");
    assert!(!res.ok);
    assert!(res.errors.iter().any(|e| e.code == "UNKNOWN_COMMAND"));
}

#[test]
fn validate_allows_manual_step_with_reason() {
    let session_id = format!("sess_test_{}", uuid::Uuid::new_v4().simple());
    seed_session(&session_id);

    let script = parse_script(
        r#"{
      "version":"1",
      "setup":[],
      "scenes":[{"id":"s1","title":"t","steps":[{"id":"a","run":"unknowncmd do","expect":null,"timeout_ms":1000,"source_refs":["ref_help_0001"],"manual_step":true,"manual_reason":"external bootstrap"}]}],
      "checks":[],
      "cleanup":[],
      "redactions":[],
      "audio":null
    }"#,
    )
    .expect("parse");

    let res = validate_script(&session_id, &script).expect("validate");
    assert!(res.ok, "errors: {:?}", res.errors);
}
