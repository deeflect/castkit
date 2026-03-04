use assert_cmd::Command;
use predicates::str::contains;

fn castkit_cmd() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("castkit")
}

#[test]
fn agent_contract_markdown_is_available() {
    castkit_cmd()
        .args(["agent", "contract"])
        .assert()
        .success()
        .stdout(contains("# castkit Agent Contract"));
}

#[test]
fn agent_contract_json_is_available() {
    let output = castkit_cmd()
        .args(["--json", "agent", "contract"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let parsed: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
    assert_eq!(parsed["name"], "castkit-agent-contract");
    assert_eq!(parsed["contract_version"], "1.0.0");
    assert!(parsed["completion_contract"].is_object());
}

#[test]
fn schema_json_is_available() {
    let output = castkit_cmd()
        .args(["--json", "schema"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let parsed: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
    assert_eq!(parsed["title"], "DemoScript");
    assert!(parsed["$defs"]["script_step"].is_object());
}
