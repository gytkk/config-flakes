use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Map, Value};

pub(crate) const COMMAND_USAGE_EVENTS_PATH_ENV: &str = "TRADE_ENGINE_COMMAND_USAGE_EVENTS_PATH";

static CAPTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderTokenUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
}

impl ProviderTokenUsage {
    fn is_empty(&self) -> bool {
        self.input_tokens.is_none()
            && self.cached_input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.reasoning_output_tokens.is_none()
            && self.total_tokens.is_none()
    }

    fn score(&self) -> u64 {
        self.total_tokens
            .or_else(|| Some(self.input_tokens.unwrap_or(0) + self.output_tokens.unwrap_or(0)))
            .unwrap_or(0)
    }
}

pub(crate) struct CommandUsageCapture {
    path: PathBuf,
}

impl CommandUsageCapture {
    pub(crate) fn new(label: &str) -> Self {
        let sequence = CAPTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let filename = format!(
            "trade-engine-{label}-codex-events-{}-{sequence}-{nanos}.jsonl",
            std::process::id()
        );
        Self {
            path: std::env::temp_dir().join(filename),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn read_token_usage(&self) -> Option<ProviderTokenUsage> {
        let content = fs::read_to_string(&self.path).ok()?;
        let _ = fs::remove_file(&self.path);
        parse_token_usage_jsonl(&content)
    }
}

impl Drop for CommandUsageCapture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn parse_token_usage_jsonl(content: &str) -> Option<ProviderTokenUsage> {
    let mut best = None;
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        collect_best_usage(&value, &mut best);
    }
    best
}

fn collect_best_usage(value: &Value, best: &mut Option<ProviderTokenUsage>) {
    match value {
        Value::Object(object) => {
            if let Some(usage) = token_usage_from_object(object)
                && best
                    .as_ref()
                    .is_none_or(|current| usage.score() >= current.score())
            {
                *best = Some(usage);
            }
            for nested in object.values() {
                collect_best_usage(nested, best);
            }
        }
        Value::Array(values) => {
            for nested in values {
                collect_best_usage(nested, best);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn token_usage_from_object(object: &Map<String, Value>) -> Option<ProviderTokenUsage> {
    let mut usage = ProviderTokenUsage {
        input_tokens: u64_field(object, &["input_tokens", "prompt_tokens"]),
        cached_input_tokens: u64_field(object, &["cached_input_tokens"]),
        output_tokens: u64_field(object, &["output_tokens", "completion_tokens"]),
        reasoning_output_tokens: u64_field(object, &["reasoning_output_tokens"]),
        total_tokens: u64_field(object, &["total_tokens"]),
    };

    if let Some(details) = object
        .get("input_tokens_details")
        .and_then(Value::as_object)
    {
        usage.cached_input_tokens = usage
            .cached_input_tokens
            .or_else(|| u64_field(details, &["cached_tokens"]));
    }
    if let Some(details) = object
        .get("output_tokens_details")
        .and_then(Value::as_object)
    {
        usage.reasoning_output_tokens = usage
            .reasoning_output_tokens
            .or_else(|| u64_field(details, &["reasoning_tokens"]));
    }
    if usage.total_tokens.is_none()
        && let (Some(input_tokens), Some(output_tokens)) = (usage.input_tokens, usage.output_tokens)
    {
        usage.total_tokens = Some(input_tokens + output_tokens);
    }

    (!usage.is_empty()).then_some(usage)
}

fn u64_field(object: &Map<String, Value>, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .filter_map(|key| object.get(*key))
        .find_map(value_as_u64)
}

fn value_as_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(string) => string.parse::<u64>().ok(),
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_token_usage_jsonl_prefers_largest_cumulative_usage() {
        let content = r#"
{"type":"event","usage":{"input_tokens":10,"output_tokens":5,"total_tokens":15}}
{"type":"token_count","info":{"last_token_usage":{"input_tokens":3,"output_tokens":2,"total_tokens":5},"total_token_usage":{"input_tokens":20,"output_tokens":9,"total_tokens":29,"input_tokens_details":{"cached_tokens":4},"output_tokens_details":{"reasoning_tokens":7}}}}
"#;

        let usage = parse_token_usage_jsonl(content).expect("token usage");

        assert_eq!(usage.input_tokens, Some(20));
        assert_eq!(usage.cached_input_tokens, Some(4));
        assert_eq!(usage.output_tokens, Some(9));
        assert_eq!(usage.reasoning_output_tokens, Some(7));
        assert_eq!(usage.total_tokens, Some(29));
    }

    #[test]
    fn parse_token_usage_jsonl_ignores_unrelated_events() {
        let content = r#"
{"type":"message","text":"ok"}
not-json
"#;

        assert_eq!(parse_token_usage_jsonl(content), None);
    }
}
