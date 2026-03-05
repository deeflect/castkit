use std::path::PathBuf;
use std::sync::Mutex;

use castkit::cli::{ExecuteArgs, KeystrokeProfile, OutputFormat, RenderSpeed};
use castkit::execute;
use castkit::handoff::session_store::save_session;
use castkit::handoff::types::{HandoffSession, RefItem, RefMetadata, SourceSummary};
use castkit::script::parse_script;
use chrono::Utc;
use once_cell::sync::Lazy;

static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

fn seed_session(session_id: &str) {
    let session = HandoffSession {
        session_id: session_id.to_string(),
        target: "web-demo".to_string(),
        created_at: Utc::now(),
        sources: vec![SourceSummary {
            source: "files".to_string(),
            pages: 1,
        }],
        refs_index_id: "idx_web".to_string(),
        refs: vec![RefItem {
            ref_id: "ref_help_0001".to_string(),
            source: "files".to_string(),
            kind: "file_chunk".to_string(),
            title: Some("index.html".to_string()),
            content: "<button id=\"cta\">Run</button>".to_string(),
            metadata: RefMetadata {
                path: Some("index.html".to_string()),
                line_start: Some(1),
            },
        }],
        discovered_commands: vec!["echo".to_string()],
    };
    save_session(&session).expect("save session");
}

#[tokio::test]
async fn execute_web_mode_dispatches_to_web_runner() {
    let _guard = ENV_LOCK.lock().expect("lock");
    std::env::set_var("CASTKIT_WEB_RUNNER_STUB", "1");
    std::env::set_var("CASTKIT_SKIP_RENDER", "1");

    let session_id = format!("sess_web_test_{}", uuid::Uuid::new_v4().simple());
    seed_session(&session_id);

    let script = parse_script(
        r##"{
      "version":"1",
      "mode":"web",
      "setup":[],
      "scenes":[],
      "checks":[],
      "cleanup":[],
      "redactions":[],
      "audio":null,
      "web":{
        "base_url":"https://example.com",
        "actions":[
          {"id":"open","type":"goto","url":"/","source_refs":["ref_help_0001"]},
          {"id":"click_cta","type":"click","selector":"#cta","source_refs":["ref_help_0001"]},
          {"id":"assert_done","type":"assert_text","text":"Example Domain","source_refs":["ref_help_0001"]}
        ]
      }
    }"##,
    )
    .expect("parse");

    let response = execute::execute(
        ExecuteArgs {
            session: session_id.clone(),
            script: PathBuf::from("demo-web.json"),
            non_interactive: true,
            output: PathBuf::from("demo-web.mp4"),
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
        },
        script,
    )
    .await
    .expect("execute");

    assert!(response.ok);
    assert!(response.failures.is_empty());
    assert!(response.render.is_none());
    let transcript_path = response.transcript_path.expect("transcript");
    let transcript_raw = std::fs::read_to_string(transcript_path).expect("read transcript");
    let transcript_json: serde_json::Value =
        serde_json::from_str(&transcript_raw).expect("parse transcript");
    let actions = transcript_json["web_actions"]
        .as_array()
        .expect("web_actions array");
    assert_eq!(actions.len(), 3);
    assert_eq!(actions[0]["id"], "open");
    assert_eq!(actions[1]["id"], "click_cta");

    std::env::remove_var("CASTKIT_WEB_RUNNER_STUB");
    std::env::remove_var("CASTKIT_SKIP_RENDER");
}
