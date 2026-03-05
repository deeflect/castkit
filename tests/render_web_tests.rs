use castkit::execute::transcript::{ExecutionTranscript, WebActionRecord};
use castkit::render::build_web_manifest_preview;

#[test]
fn render_web_manifest_serializes_action_keyframes() {
    let transcript = ExecutionTranscript {
        session_id: "sess_web_render".to_string(),
        started_at: chrono::Utc::now(),
        mode: castkit::script::DemoMode::Web,
        setup: vec![],
        checks: vec![],
        scenes: vec![],
        cleanup: vec![],
        overlay_events: vec![],
        web_actions: vec![
            WebActionRecord {
                id: "b".to_string(),
                action_type: "click".to_string(),
                status: "ok".to_string(),
                error: None,
                t_ms: 600,
                duration_ms: 90,
                selector: Some("#b".to_string()),
                cursor_x: Some(200.0),
                cursor_y: Some(120.0),
                target_x: Some(180.0),
                target_y: Some(100.0),
                target_w: Some(160.0),
                target_h: Some(40.0),
                screenshot_path: None,
            },
            WebActionRecord {
                id: "a".to_string(),
                action_type: "goto".to_string(),
                status: "ok".to_string(),
                error: None,
                t_ms: 200,
                duration_ms: 120,
                selector: None,
                cursor_x: Some(80.0),
                cursor_y: Some(64.0),
                target_x: None,
                target_y: None,
                target_w: None,
                target_h: None,
                screenshot_path: None,
            },
        ],
    };

    let manifest = build_web_manifest_preview(&transcript, 60, false);
    let actions = manifest["actions"].as_array().expect("actions array");
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0]["id"], "a");
    assert_eq!(actions[1]["id"], "b");
    assert!(manifest["duration_ms"].as_u64().unwrap_or_default() >= 3500);
}
