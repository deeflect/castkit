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
        "manual_reason": { "type": "string" }
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
        "setup": { "type": "array", "items": { "$ref": "#/$defs/script_step" }, "default": [] },
        "scenes": { "type": "array", "items": { "$ref": "#/$defs/script_scene" }, "default": [] },
        "checks": { "type": "array", "items": { "$ref": "#/$defs/script_step" }, "default": [] },
        "cleanup": { "type": "array", "items": { "$ref": "#/$defs/script_step" }, "default": [] },
        "redactions": { "type": "array", "items": { "$ref": "#/$defs/redact_rule" }, "default": [] },
        "audio": { "$ref": "#/$defs/audio_config" },
        "branding": { "$ref": "#/$defs/branding_config" }
      },
      "$defs": {
        "expect_condition": expect_schema(),
        "script_step": step_schema(),
        "script_scene": scene_schema(),
        "redact_rule": redaction_schema(),
        "audio_config": audio_schema(),
        "branding_config": branding_schema()
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
        assert!(schema["examples"].is_array());
    }
}
