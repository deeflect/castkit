use serde_json::{json, Value};

pub const CONTRACT_VERSION: &str = "1.0.0";

pub fn contract_markdown() -> &'static str {
    include_str!("../AGENTS.md")
}

pub fn contract_json() -> Value {
    json!({
      "name": "castkit-agent-contract",
      "contract_version": CONTRACT_VERSION,
      "script_schema": {
        "name": "DemoScript",
        "version": "1",
        "command": "castkit schema --json"
      },
      "script_mode_support": {
        "default_mode": "terminal",
        "modes": ["terminal", "web"],
        "terminal": {
          "step_artifacts": ["image", "result_card", "web_snapshot", "chart"],
          "note": "image and result_card are executable in current release; others may be validator-accepted but execution-gated."
        },
        "web": {
          "required_block": "web",
          "actions": [
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
        }
      },
      "bootstrap": [
        {
          "step": "load_contract",
          "command": "castkit --json agent contract"
        },
        {
          "step": "load_schema",
          "command": "castkit --json schema"
        }
      ],
      "required_flow": [
        "castkit handoff init <target> --json",
        "castkit handoff list --session <id> --source <help|readme|files|probes> --page <n> --per-page <m> --json",
        "castkit handoff get --session <id> --ref <ref_id> --json",
        "castkit validate --session <id> --script demo.json --json",
        "castkit execute --session <id> --script demo.json --non-interactive --output demo.mp4 --json"
      ],
      "hard_rules": [
        "Never invent executable commands/flags/paths/setup steps.",
        "Every executable step must include non-empty source_refs from active session.",
        "Run validate before execute.",
        "Use non-interactive execution for deterministic runs.",
        "Bootstrap commands (agent contract/schema) are for planning context, not showcase scene output.",
        "For mode=web, every action must carry non-empty source_refs."
      ],
      "runtime_env": {
        "always_set": ["SESSION", "CASTKIT_SESSION"],
        "notes": [
          "Both vars start as execute --session value.",
          "If a step outputs JSON with session_id, runtime vars update for following steps."
        ]
      },
      "completion_contract": {
        "success_conditions": {
          "exit_code": 0,
          "response_ok": true
        },
        "validate_success": {
          "ok": true,
          "errors_len": 0
        },
        "execute_success": {
          "ok": true,
          "required_fields": ["output", "transcript_path", "render"]
        },
        "failure_routes": {
          "step_or_validation_failures": "fix-script",
          "environment_failures": "fix-environment"
        }
      },
      "agent_feedback_payload": {
        "required_fields": [
          "status",
          "session_id",
          "output",
          "duration_secs",
          "failed_step",
          "next_action"
        ],
        "next_action_enum": ["done", "fix-script", "fix-environment"]
      },
      "timeouts": {
        "poll_interval_secs": 20,
        "cron_watchdog_interval_secs": 60,
        "soft_timeout_secs": 480,
        "hard_timeout_secs": 1200,
        "hard_timeout_heuristic": "max(10, ceil(video_minutes * 4)), capped at 20 minutes"
      },
      "scenario_playbook": {
        "narrative_order": [
          "setup_trust",
          "happy_path",
          "power_move",
          "proof",
          "wrap"
        ],
        "quality_gates": [
          "No scene should be pure setup without visible user outcome.",
          "At least one explicit final-state check in checks.",
          "Titles and command order must tell a coherent story from problem to proof.",
          "Use deterministic commands with observable output."
        ]
      },
      "notes": {
        "human_contract_command": "castkit agent contract",
        "contract_markdown_source": "embedded AGENTS.md"
      }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_has_required_top_level_keys() {
        let contract = contract_json();
        assert_eq!(contract["contract_version"], CONTRACT_VERSION);
        assert!(contract["required_flow"].is_array());
        assert!(contract["completion_contract"].is_object());
        assert!(contract["scenario_playbook"].is_object());
        assert!(contract["script_mode_support"].is_object());
    }

    #[test]
    fn markdown_contract_is_embedded() {
        let body = contract_markdown();
        assert!(body.contains("# castkit Agent Contract"));
        assert!(body.contains("## Completion Contract (machine-readable)"));
    }
}
