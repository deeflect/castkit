use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::execute::transcript::{
    OverlayArtifactType, OverlayEvent, OverlayResultItem, StepRunRecord,
};
use crate::script::{ArtifactDisplay, ArtifactEnter, ArtifactPosition, ScriptStep, StepArtifact};

const DEFAULT_SHOW_MS: u64 = 2_200;
const STEP_ARTIFACT_STAGGER_MS: u64 = 90;

pub fn capture_artifacts(
    step: &ScriptStep,
    cwd: &Path,
    _record: &StepRunRecord,
    now_t_ms: u64,
) -> Result<Vec<OverlayEvent>> {
    let mut events = Vec::new();

    for (idx, artifact) in step.artifacts.iter().enumerate() {
        let t_ms = now_t_ms + (idx as u64 * STEP_ARTIFACT_STAGGER_MS);
        let event = match artifact {
            StepArtifact::Image(image) => {
                let staged = stage_file(cwd, &image.path).with_context(|| {
                    format!(
                        "artifact image file missing or unreadable for step '{}': {}",
                        step.id, image.path
                    )
                })?;
                OverlayEvent {
                    t_ms,
                    step_id: step.id.clone(),
                    artifact_type: OverlayArtifactType::Image,
                    title: image.display.title.clone(),
                    image_path: Some(staged.to_string_lossy().to_string()),
                    result_items: Vec::new(),
                    position: display_position(&image.display),
                    show_ms: display_show_ms(&image.display),
                    enter: display_enter(&image.display),
                }
            }
            StepArtifact::ResultCard(card) => OverlayEvent {
                t_ms,
                step_id: step.id.clone(),
                artifact_type: OverlayArtifactType::ResultCard,
                title: card.display.title.clone(),
                image_path: None,
                result_items: card
                    .items
                    .iter()
                    .map(|i| OverlayResultItem {
                        label: i.label.clone(),
                        value: i.value.clone(),
                    })
                    .collect(),
                position: display_position(&card.display),
                show_ms: display_show_ms(&card.display),
                enter: display_enter(&card.display),
            },
            StepArtifact::WebSnapshot(_) => {
                anyhow::bail!(
                    "web_snapshot artifacts are not implemented in execute yet (step '{}')",
                    step.id
                );
            }
            StepArtifact::Chart(_) => {
                anyhow::bail!(
                    "chart artifacts are not implemented in execute yet (step '{}')",
                    step.id
                );
            }
        };

        events.push(event);
    }

    Ok(events)
}

fn stage_file(cwd: &Path, rel_path: &str) -> Result<PathBuf> {
    let source_path = cwd.join(rel_path);
    let meta = fs::metadata(&source_path)
        .with_context(|| format!("unable to stat {}", source_path.display()))?;
    if !meta.is_file() {
        anyhow::bail!("artifact path is not a regular file: {}", source_path.display());
    }

    let extension = source_path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("png");
    let staged_path = std::env::temp_dir().join(format!(
        "castkit-artifact-{}.{}",
        uuid::Uuid::new_v4().simple(),
        extension
    ));

    fs::copy(&source_path, &staged_path).with_context(|| {
        format!(
            "failed to stage artifact from {} to {}",
            source_path.display(),
            staged_path.display()
        )
    })?;
    Ok(staged_path)
}

fn display_position(display: &ArtifactDisplay) -> ArtifactPosition {
    display.position.unwrap_or(ArtifactPosition::TopRight)
}

fn display_show_ms(display: &ArtifactDisplay) -> u64 {
    display.show_ms.unwrap_or(DEFAULT_SHOW_MS)
}

fn display_enter(display: &ArtifactDisplay) -> ArtifactEnter {
    display.enter.unwrap_or(ArtifactEnter::Fade)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::capture_artifacts;
    use crate::execute::transcript::StepRunRecord;
    use crate::script::{ArtifactDisplay, ImageArtifact, ScriptStep, StepArtifact};

    fn ok_record() -> StepRunRecord {
        StepRunRecord {
            id: "step".to_string(),
            run: "echo hi".to_string(),
            stdout: "ok".to_string(),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: 1,
            status: "ok".to_string(),
            error: None,
        }
    }

    #[test]
    fn capture_image_artifact_stages_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let image_path = dir.path().join("out.png");
        fs::write(&image_path, b"png").expect("write image");
        let step = ScriptStep {
            id: "step1".to_string(),
            run: "echo ok".to_string(),
            expect: None,
            timeout_ms: None,
            source_refs: vec!["ref_help_0001".to_string()],
            manual_step: false,
            manual_reason: None,
            artifacts: vec![StepArtifact::Image(ImageArtifact {
                path: "out.png".to_string(),
                display: ArtifactDisplay {
                    title: Some("QR".to_string()),
                    position: None,
                    show_ms: None,
                    enter: None,
                },
            })],
        };
        let events = capture_artifacts(&step, dir.path(), &ok_record(), 320).expect("capture");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].step_id, "step1");
        assert_eq!(events[0].t_ms, 320);
        assert!(events[0].image_path.is_some());
    }

    #[test]
    fn missing_image_artifact_file_is_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let step = ScriptStep {
            id: "step_missing".to_string(),
            run: "echo ok".to_string(),
            expect: None,
            timeout_ms: None,
            source_refs: vec!["ref_help_0001".to_string()],
            manual_step: false,
            manual_reason: None,
            artifacts: vec![StepArtifact::Image(ImageArtifact {
                path: "missing.png".to_string(),
                display: ArtifactDisplay {
                    title: None,
                    position: None,
                    show_ms: None,
                    enter: None,
                },
            })],
        };
        let err = capture_artifacts(&step, dir.path(), &ok_record(), 0).expect_err("must fail");
        assert!(err.to_string().contains("artifact image file missing or unreadable"));
    }
}
