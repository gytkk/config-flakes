use std::fs;

use tempfile::tempdir;
use trade_engine::feedback::{
    CommandFeedbackProvider, FEEDBACK_REQUEST_SCHEMA_VERSION, FeedbackProvider,
    FeedbackQueryContext, FeedbackQueryRequest,
};

#[tokio::test]
async fn command_feedback_provider_sends_request_and_parses_stdout() {
    let tempdir = tempdir().expect("tempdir");
    let request_path = tempdir.path().join("feedback-request.json");
    let script_path = tempdir.path().join("feedback.sh");
    fs::write(
        &script_path,
        format!(
            "#!/bin/sh\ncat > {}\nprintf '%s\\n' '{{\"summary\":\"reviewed\",\"findings\":[],\"risks\":[],\"suggested_followups\":[],\"policy_candidates\":[],\"evidence\":[]}}'\n",
            request_path.display()
        ),
    )
    .expect("write feedback script");

    let provider = CommandFeedbackProvider::new_for_test(
        "sh",
        vec![script_path.to_string_lossy().to_string()],
    );
    let response = provider
        .query(&FeedbackQueryRequest {
            schema_version: FEEDBACK_REQUEST_SCHEMA_VERSION,
            prompt: "summarize".to_owned(),
            context: FeedbackQueryContext {
                review: serde_json::json!({"events": []}),
                redaction_notes: vec!["account_suffix_only".to_owned()],
            },
        })
        .await
        .expect("feedback output");

    assert_eq!(response.output.output().summary, "reviewed");
    let request_json = fs::read_to_string(request_path).expect("request json");
    let request_value: serde_json::Value =
        serde_json::from_str(&request_json).expect("request json value");
    assert_eq!(request_value["schema_version"], 1);
    assert_eq!(request_value["prompt"], "summarize");
}
