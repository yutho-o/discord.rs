//! The flattened errors of `docs/design/rest.md` section 10, and no token in any message.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dr_rest::error::{flatten_errors, from_response, mask_route};
use dr_rest::{Method, RequestSummary, RestError};
use serde_json::json;

fn field(errors: serde_json::Value) -> (String, String, String) {
    let found = flatten_errors(&errors);
    assert_eq!(found.len(), 1, "{found:?}");
    let first = found.into_iter().next().unwrap();
    (first.path, first.code, first.message)
}

#[test]
fn a_top_level_field() {
    let errors = json!({"username": {"_errors": [{"code": "BASE_TYPE_BAD_LENGTH", "message": "Must be between 2 and 32 in length."}]}});
    let (path, code, message) = field(errors);
    assert_eq!(path, "username");
    assert_eq!(code, "BASE_TYPE_BAD_LENGTH");
    assert_eq!(message, "Must be between 2 and 32 in length.");
}

#[test]
fn an_array_inside_an_object() {
    let errors = json!({"embed": {"fields": {"0": {"value": {"_errors": [{"code": "BASE_TYPE_REQUIRED", "message": "This field is required"}]}}}}});
    assert_eq!(field(errors).0, "embed.fields[0].value");
}

#[test]
fn a_trailing_index() {
    let errors = json!({"form_fields": {"label": {"0": {"_errors": [{"code": "BASE_TYPE_REQUIRED", "message": "This field is required"}]}}}});
    assert_eq!(field(errors).0, "form_fields.label[0]");
}

fn response(status: u16, body: &str) -> RestError {
    let request = RequestSummary::new(Method::Post, "/channels/:id/messages", Some(status), 1);
    from_response(status, &[], body.as_bytes(), request)
}

#[test]
fn an_api_error_keeps_its_code_and_fields() {
    let body = r#"{"code":50035,"message":"Invalid Form Body","errors":{"content":{"_errors":[{"code":"X","message":"bad"}]}}}"#;
    let error = response(400, body);
    assert_eq!(error.code().map(|c| c.0), Some(50035));
    let text = error.to_string();
    assert!(text.contains("Invalid Form Body"), "{text}");
    assert!(!error.is_retryable());
}

#[test]
fn a_non_json_body_is_still_an_error() {
    let error = response(502, "<html>bad gateway</html>");
    assert!(error.code().is_none());
    assert!(error.is_retryable(), "{error:?}");
}

#[test]
fn the_webhook_token_never_reaches_a_message() {
    let masked = mask_route("/webhooks/1234567890123456789/secret-token/messages/%40original");
    assert!(!masked.contains("secret-token"), "{masked}");
    let masked = mask_route("/interactions/1234567890123456789/secret-token/callback");
    assert!(!masked.contains("secret-token"), "{masked}");
}
