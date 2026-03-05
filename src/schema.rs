use serde_json::{json, Value};

fn branding_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "title": { "type": "string" },
        "bg_primary": { "type": "string" },
        "bg_secondary": { "type": "string" },
        "text_primary": { "type": "string" },
        "text_muted": { "type": "string" },
        "command_text": { "type": "string" },
        "accent": { "type": "string" },
        "watermark_text": { "type": "string" },
        "avatar_x": { "type": "string" },
        "avatar_url": { "type": "string" },
        "avatar_label": { "type": "string" }
      }
    })
}

fn expect_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "contains": { "type": "string" },
        "regex": { "type": "string" },
        "exit_code": { "type": "integer" }
      }
    })
}

fn step_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "required": ["id", "run", "source_refs"],
      "properties": {
        "id": { "type": "string", "minLength": 1 },
        "run": { "type": "string", "minLength": 1 },
        "expect": { "$ref": "#/$defs/expect_condition" },
        "timeout_ms": { "type": "integer", "minimum": 1 },
        "source_refs": {
          "type": "array",
          "items": { "type": "string" }
        },
        "manual_step": { "type": "boolean", "default": false },
        "manual_reason": { "type": "string" },
        "artifacts": {
          "type": "array",
          "items": { "$ref": "#/$defs/step_artifact" },
          "default": []
        }
      }
    })
}

fn scene_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "required": ["id", "title", "steps"],
      "properties": {
        "id": { "type": "string", "minLength": 1 },
        "title": { "type": "string", "minLength": 1 },
        "steps": {
          "type": "array",
          "items": { "$ref": "#/$defs/script_step" }
        }
      }
    })
}

fn redaction_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "required": ["pattern"],
      "properties": {
        "pattern": { "type": "string", "minLength": 1 }
      }
    })
}

fn audio_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "typing": { "type": "boolean", "default": false },
        "music_path": { "type": "string" }
      }
    })
}

fn image_artifact_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "required": ["type", "path"],
      "properties": {
        "type": { "const": "image" },
        "path": { "type": "string", "minLength": 1 },
        "title": { "type": "string" },
        "position": {
          "type": "string",
          "enum": ["top_left", "top_right", "bottom_left", "bottom_right", "center"]
        },
        "show_ms": { "type": "integer", "minimum": 1 },
        "enter": { "type": "string", "enum": ["fade", "slide", "scale"] }
      }
    })
}

fn web_snapshot_artifact_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "required": ["type", "url"],
      "properties": {
        "type": { "const": "web_snapshot" },
        "url": { "type": "string", "minLength": 1 },
        "path": { "type": "string", "minLength": 1 },
        "wait_for_selector": { "type": "string", "minLength": 1 },
        "clip_selector": { "type": "string", "minLength": 1 },
        "title": { "type": "string" },
        "position": {
          "type": "string",
          "enum": ["top_left", "top_right", "bottom_left", "bottom_right", "center"]
        },
        "show_ms": { "type": "integer", "minimum": 1 },
        "enter": { "type": "string", "enum": ["fade", "slide", "scale"] }
      }
    })
}

fn result_card_item_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "required": ["label", "value"],
      "properties": {
        "label": { "type": "string", "minLength": 1 },
        "value": { "type": "string", "minLength": 1 }
      }
    })
}

fn result_card_artifact_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "required": ["type"],
      "properties": {
        "type": { "const": "result_card" },
        "items": {
          "type": "array",
          "items": { "$ref": "#/$defs/result_card_item" },
          "default": []
        },
        "title": { "type": "string" },
        "position": {
          "type": "string",
          "enum": ["top_left", "top_right", "bottom_left", "bottom_right", "center"]
        },
        "show_ms": { "type": "integer", "minimum": 1 },
        "enter": { "type": "string", "enum": ["fade", "slide", "scale"] }
      }
    })
}

fn chart_artifact_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "required": ["type", "chart_type", "data_path"],
      "properties": {
        "type": { "const": "chart" },
        "chart_type": { "type": "string", "enum": ["line", "bar"] },
        "data_path": { "type": "string", "minLength": 1 },
        "title": { "type": "string" },
        "position": {
          "type": "string",
          "enum": ["top_left", "top_right", "bottom_left", "bottom_right", "center"]
        },
        "show_ms": { "type": "integer", "minimum": 1 },
        "enter": { "type": "string", "enum": ["fade", "slide", "scale"] }
      }
    })
}

fn step_artifact_schema() -> Value {
    json!({
      "oneOf": [
        { "$ref": "#/$defs/image_artifact" },
        { "$ref": "#/$defs/web_snapshot_artifact" },
        { "$ref": "#/$defs/result_card_artifact" },
        { "$ref": "#/$defs/chart_artifact" }
      ]
    })
}

fn web_viewport_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "required": ["width", "height"],
      "properties": {
        "width": { "type": "integer", "minimum": 320 },
        "height": { "type": "integer", "minimum": 240 }
      }
    })
}

fn web_action_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "required": ["id", "type", "source_refs"],
      "properties": {
        "id": { "type": "string", "minLength": 1 },
        "type": {
          "type": "string",
          "enum": [
            "goto",
            "click",
            "type",
            "press",
            "wait_for_selector",
            "wait_ms",
            "assert_text",
            "screenshot",
            "scroll_to"
          ]
        },
        "source_refs": { "type": "array", "items": { "type": "string" } },
        "url": { "type": "string" },
        "selector": { "type": "string" },
        "text": { "type": "string" },
        "key": { "type": "string" },
        "wait_ms": { "type": "integer", "minimum": 1 },
        "path": { "type": "string" }
      }
    })
}

fn web_config_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "base_url": { "type": "string" },
        "viewport": { "$ref": "#/$defs/web_viewport" },
        "actions": {
          "type": "array",
          "items": { "$ref": "#/$defs/web_action" },
          "default": []
        }
      }
    })
}

pub fn demo_script_schema() -> Value {
    let example: Value =
        serde_json::from_str(include_str!("../examples/demo-script.template.json"))
            .expect("demo-script template must be valid JSON");
    json!({
      "$schema": "https://json-schema.org/draft/2020-12/schema",
      "title": "DemoScript",
      "type": "object",
      "additionalProperties": false,
      "required": ["version"],
      "properties": {
        "version": { "type": "string", "minLength": 1, "const": "1" },
        "mode": { "type": "string", "enum": ["terminal", "web"], "default": "terminal" },
        "setup": { "type": "array", "items": { "$ref": "#/$defs/script_step" }, "default": [] },
        "scenes": { "type": "array", "items": { "$ref": "#/$defs/script_scene" }, "default": [] },
        "checks": { "type": "array", "items": { "$ref": "#/$defs/script_step" }, "default": [] },
        "cleanup": { "type": "array", "items": { "$ref": "#/$defs/script_step" }, "default": [] },
        "redactions": { "type": "array", "items": { "$ref": "#/$defs/redact_rule" }, "default": [] },
        "audio": { "$ref": "#/$defs/audio_config" },
        "branding": { "$ref": "#/$defs/branding_config" },
        "web": { "$ref": "#/$defs/web_config" }
      },
      "allOf": [
        {
          "if": {
            "properties": { "mode": { "const": "web" } },
            "required": ["mode"]
          },
          "then": { "required": ["web"] }
        }
      ],
      "$defs": {
        "expect_condition": expect_schema(),
        "script_step": step_schema(),
        "script_scene": scene_schema(),
        "redact_rule": redaction_schema(),
        "audio_config": audio_schema(),
        "branding_config": branding_schema(),
        "result_card_item": result_card_item_schema(),
        "image_artifact": image_artifact_schema(),
        "web_snapshot_artifact": web_snapshot_artifact_schema(),
        "result_card_artifact": result_card_artifact_schema(),
        "chart_artifact": chart_artifact_schema(),
        "step_artifact": step_artifact_schema(),
        "web_viewport": web_viewport_schema(),
        "web_action": web_action_schema(),
        "web_config": web_config_schema()
      },
      "examples": [example]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_has_core_structure() {
        let schema = demo_script_schema();
        assert_eq!(schema["title"], "DemoScript");
        assert_eq!(schema["type"], "object");
        assert!(schema["$defs"]["script_step"].is_object());
        assert!(schema["$defs"]["step_artifact"].is_object());
        assert!(schema["$defs"]["web_action"].is_object());
        assert!(schema["examples"].is_array());
    }
}
