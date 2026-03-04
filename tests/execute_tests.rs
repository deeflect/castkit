use std::path::PathBuf;

use castkit::cli::{EncoderMode, ExecuteArgs, KeystrokeProfile, OutputFormat, RenderSpeed};
use castkit::execute;
use castkit::handoff::session_store::save_session;
use castkit::handoff::types::{HandoffSession, RefItem, RefMetadata, SourceSummary};
use castkit::script::parse_script;
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
            content: "echo hello".to_string(),
            metadata: RefMetadata {
                path: None,
                line_start: Some(1),
            },
        }],
        discovered_commands: vec!["echo".to_string()],
    };
    save_session(&session).expect("save session");
}

#[tokio::test]
async fn execute_requires_non_interactive() {
    let session_id = format!("sess_test_{}", uuid::Uuid::new_v4().simple());
    seed_session(&session_id);

    let script = parse_script(
        r#"{
      "version":"1",
      "setup":[],
      "scenes":[{"id":"s1","title":"t","steps":[{"id":"a","run":"echo hi","expect":{"contains":"hi"},"timeout_ms":1000,"source_refs":["ref_help_0001"]}]}],
      "checks":[],
      "cleanup":[],
      "redactions":[],
      "audio":null
    }"#,
    )
    .expect("parse");

    let res = execute::execute(
        ExecuteArgs {
            session: session_id,
            script: PathBuf::from("demo.json"),
            non_interactive: false,
            output: PathBuf::from("demo.mp4"),
            format: OutputFormat::Mp4,
            fps: Some(60),
            no_zoom: false,
            music: None,
            typing_sound: false,
            branding: None,
            brand_title: None,
            watermark: None,
            avatar_x: None,
            avatar_url: None,
            avatar_label: None,
            avatar_cache_dir: None,
            preset: None,
            theme: None,
            speed: Some(RenderSpeed::Quality),
            keystroke_profile: Some(KeystrokeProfile::Laptop),
            encoder: EncoderMode::Auto,
        },
        script,
    )
    .await
    .expect("execute");

    assert!(!res.ok);
    assert!(res
        .failures
        .iter()
        .any(|f| f.reason.contains("--non-interactive")));
}
