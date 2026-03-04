use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;

use crate::script::RedactRule;

use super::transcript::StepRunRecord;

const REDACTION_TOKEN: &str = "[REDACTED]";

static BUILTIN_SECRET_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"(?i)sk-[A-Za-z0-9]{20,}").expect("valid sk pattern"),
        Regex::new(r"(?i)ghp_[A-Za-z0-9]{20,}").expect("valid ghp pattern"),
        Regex::new(r"\bAKIA[0-9A-Z]{16}\b").expect("valid akia pattern"),
        Regex::new(r"(?i)\b(api[_-]?key|token|secret|password)\s*=\s*\S+")
            .expect("valid assignment pattern"),
    ]
});

#[derive(Debug, Clone)]
pub struct Redactor {
    patterns: Vec<Regex>,
}

impl Redactor {
    pub fn from_rules(rules: &[RedactRule]) -> Result<Self> {
        let mut patterns = BUILTIN_SECRET_PATTERNS.clone();
        for rule in rules {
            let re = Regex::new(&rule.pattern)
                .with_context(|| format!("invalid redaction regex '{}'", rule.pattern))?;
            patterns.push(re);
        }
        Ok(Self { patterns })
    }

    pub fn redact_record(&self, record: StepRunRecord) -> StepRunRecord {
        StepRunRecord {
            run: self.redact_text(&record.run),
            stdout: self.redact_text(&record.stdout),
            stderr: self.redact_text(&record.stderr),
            error: record.error.map(|e| self.redact_text(&e)),
            ..record
        }
    }

    pub fn redact_text(&self, input: &str) -> String {
        let mut out = input.to_string();
        for pattern in &self.patterns {
            out = pattern.replace_all(&out, REDACTION_TOKEN).into_owned();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::Redactor;
    use crate::script::RedactRule;

    #[test]
    fn redacts_builtin_secrets() {
        let redactor = Redactor::from_rules(&[]).expect("redactor");
        let out = redactor.redact_text("token=abc123 sk-ABCDEFGHIJKLMNOPQRSTUV");
        assert!(out.contains("[REDACTED]"));
        assert!(!out.contains("abc123"));
    }

    #[test]
    fn redacts_custom_patterns() {
        let redactor = Redactor::from_rules(&[RedactRule {
            pattern: "my_secret_[0-9]+".to_string(),
        }])
        .expect("redactor");
        let out = redactor.redact_text("value=my_secret_42");
        assert_eq!(out, "value=[REDACTED]");
    }
}
