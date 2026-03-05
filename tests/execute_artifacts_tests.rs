use std::fs;

use castkit::execute::artifacts::capture_artifacts;
use castkit::execute::transcript::StepRunRecord;
use castkit::script::{ArtifactDisplay, ImageArtifact, ScriptStep, StepArtifact};

fn ok_record() -> StepRunRecord {
    StepRunRecord {
        id: "step".to_string(),
        run: "echo ok".to_string(),
        stdout: "ok".to_string(),
        stderr: String::new(),
        exit_code: 0,
        duration_ms: 10,
        status: "ok".to_string(),
        error: None,
    }
}

#[test]
fn execute_artifacts_image_creates_overlay_event() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("qr.png"), b"png").expect("write image");
    let step = ScriptStep {
        id: "img".to_string(),
        run: "echo ok".to_string(),
        expect: None,
        timeout_ms: Some(1_000),
        source_refs: vec!["ref_help_0001".to_string()],
        manual_step: false,
        manual_reason: None,
        artifacts: vec![StepArtifact::Image(ImageArtifact {
            path: "qr.png".to_string(),
            display: ArtifactDisplay {
                title: Some("QR".to_string()),
                position: None,
                show_ms: None,
                enter: None,
            },
        })],
    };

    let events = capture_artifacts(&step, dir.path(), &ok_record(), 900).expect("capture");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].t_ms, 900);
    assert!(events[0].image_path.is_some());
}

#[test]
fn execute_artifacts_missing_image_file_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let step = ScriptStep {
        id: "img_missing".to_string(),
        run: "echo ok".to_string(),
        expect: None,
        timeout_ms: Some(1_000),
        source_refs: vec!["ref_help_0001".to_string()],
        manual_step: false,
        manual_reason: None,
        artifacts: vec![StepArtifact::Image(ImageArtifact {
            path: "nope.png".to_string(),
            display: ArtifactDisplay {
                title: None,
                position: None,
                show_ms: None,
                enter: None,
            },
        })],
    };

    let err = capture_artifacts(&step, dir.path(), &ok_record(), 0).expect_err("must fail");
    assert!(err
        .to_string()
        .contains("artifact image file missing or unreadable"));
}
