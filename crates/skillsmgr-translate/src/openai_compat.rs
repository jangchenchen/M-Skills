use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use skillsmgr_core::{Result, SkillsMgrError};

use crate::{TranslationProvider, TranslationRequest};

const PROVIDER_KIND: &str = "openai-compat";
pub const PROMPT_VERSION: &str = "1";
const SYSTEM_PROMPT_TEMPLATE: &str = "Prompt version: {{prompt_version}}. \
You are a precise technical translator. \
Translate the user's text into the target locale ({{locale}}). \
Preserve every Markdown structure, code fence, inline code, list bullet, \
heading, table, link target, and frontmatter key exactly as-is. \
Do not add explanations, prefixes, or surrounding quotes — output only the translated text.";

pub struct OpenAICompatProvider {
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: String,
    max_retries: u32,
}

impl OpenAICompatProvider {
    pub fn new(
        base_url: String,
        model: String,
        api_key: String,
        timeout: Duration,
        max_retries: u32,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| SkillsMgrError::TranslateProvider {
                kind: PROVIDER_KIND.into(),
                status: None,
                message: format!("build http client: {e}"),
            })?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
            api_key,
            max_retries,
        })
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    fn build_body(&self, request: &TranslationRequest) -> ChatRequest {
        let system_prompt = SYSTEM_PROMPT_TEMPLATE
            .replace("{{prompt_version}}", PROMPT_VERSION)
            .replace("{{locale}}", &request.locale);
        ChatRequest {
            model: self.model.clone(),
            temperature: 0.2,
            messages: vec![
                ChatMessage {
                    role: "system".into(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".into(),
                    content: request.source_text.clone(),
                },
            ],
        }
    }

    async fn attempt(&self, body: &ChatRequest) -> AttemptOutcome {
        let response = match self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .json(body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let kind = if e.is_timeout() {
                    "timeout"
                } else if e.is_connect() || e.is_request() {
                    "network"
                } else {
                    "network"
                };
                return AttemptOutcome::Retryable(SkillsMgrError::TranslateProvider {
                    kind: PROVIDER_KIND.into(),
                    status: None,
                    message: format!("{kind}: {e}"),
                });
            }
        };

        let status = response.status();
        let status_code = status.as_u16();

        if status.is_success() {
            let body_text = match response.text().await {
                Ok(text) => text,
                Err(e) => {
                    return AttemptOutcome::Done(Err(SkillsMgrError::TranslateProvider {
                        kind: PROVIDER_KIND.into(),
                        status: Some(status_code),
                        message: format!("read response body: {e}"),
                    }));
                }
            };
            return match decode_chat_completion_text(&body_text) {
                Ok(text) => AttemptOutcome::Done(Ok(text)),
                Err(reason) => AttemptOutcome::Done(Err(SkillsMgrError::TranslateProvider {
                    kind: PROVIDER_KIND.into(),
                    status: Some(status_code),
                    message: format!(
                        "decode response from {}: {reason}; body: {}",
                        self.endpoint(),
                        truncate_body(&body_text)
                    ),
                })),
            };
        }

        let body_text = response.text().await.unwrap_or_default();
        let err = SkillsMgrError::TranslateProvider {
            kind: PROVIDER_KIND.into(),
            status: Some(status_code),
            message: format!(
                "{} returned: {}",
                self.endpoint(),
                truncate_body(&body_text)
            ),
        };
        if status.is_server_error() {
            AttemptOutcome::Retryable(err)
        } else {
            AttemptOutcome::Done(Err(err))
        }
    }

    async fn run_with_retries(&self, body: &ChatRequest) -> Result<String> {
        let mut last_err: Option<SkillsMgrError> = None;
        for attempt in 0..=self.max_retries {
            match self.attempt(body).await {
                AttemptOutcome::Done(result) => return result,
                AttemptOutcome::Retryable(err) => {
                    last_err = Some(err);
                    if attempt < self.max_retries {
                        let backoff_ms = 300u64 * 3u64.pow(attempt);
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    }
                }
            }
        }
        Err(
            last_err.unwrap_or_else(|| SkillsMgrError::TranslateProvider {
                kind: PROVIDER_KIND.into(),
                status: None,
                message: "unknown failure".into(),
            }),
        )
    }

    /// General-purpose chat completion. Used by callers outside the translation
    /// pipeline (e.g. import-time compatibility review) that want the same
    /// retry / HTML-detection / empty-response handling without a `TranslationRequest`.
    pub async fn chat_complete(
        &self,
        messages: Vec<(String, String)>,
        temperature: f32,
    ) -> Result<String> {
        let body = ChatRequest {
            model: self.model.clone(),
            temperature,
            messages: messages
                .into_iter()
                .map(|(role, content)| ChatMessage { role, content })
                .collect(),
        };
        self.run_with_retries(&body).await
    }
}

#[async_trait]
impl TranslationProvider for OpenAICompatProvider {
    async fn translate(&self, request: &TranslationRequest) -> Result<String> {
        let body = self.build_body(request);
        self.run_with_retries(&body).await
    }

    fn kind(&self) -> &'static str {
        PROVIDER_KIND
    }
}

enum AttemptOutcome {
    Done(Result<String>),
    Retryable(SkillsMgrError),
}

fn truncate_body(body: &str) -> String {
    const MAX: usize = 240;
    if body.len() <= MAX {
        body.to_string()
    } else {
        format!("{}…", &body[..MAX])
    }
}

fn decode_chat_completion_text(body: &str) -> std::result::Result<String, String> {
    let value: Value = serde_json::from_str(body).map_err(|e| {
        if looks_like_html(body) {
            format!(
                "expected JSON but received HTML. Check that the Base URL points to an OpenAI-compatible API root, not a web page; JSON error: {e}"
            )
        } else {
            format!("invalid JSON response: {e}")
        }
    })?;
    match extract_chat_completion_text(&value) {
        Some(text) if !text.trim().is_empty() => Ok(text),
        _ => {
            if let Some(refusal) = extract_chat_completion_refusal(&value) {
                Err(format!("provider refusal: {refusal}"))
            } else {
                Err("response had no translated text".into())
            }
        }
    }
}

fn looks_like_html(body: &str) -> bool {
    let trimmed = body.trim_start();
    trimmed.starts_with("<!DOCTYPE html")
        || trimmed.starts_with("<!doctype html")
        || trimmed.starts_with("<html")
}

fn extract_chat_completion_text(value: &Value) -> Option<String> {
    if let Some(choices) = value.get("choices").and_then(Value::as_array) {
        if let Some(choice) = choices.first() {
            if let Some(text) = extract_choice_text(choice) {
                return Some(text);
            }
        }
    }

    value
        .get("output_text")
        .and_then(extract_text_value)
        .or_else(|| value.get("text").and_then(extract_text_value))
        .or_else(|| value.get("message").and_then(extract_message_text))
}

fn extract_chat_completion_refusal(value: &Value) -> Option<String> {
    value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("refusal"))
        .and_then(extract_text_value)
}

fn extract_choice_text(choice: &Value) -> Option<String> {
    choice
        .get("message")
        .and_then(extract_message_text)
        .or_else(|| choice.get("delta").and_then(extract_message_text))
        .or_else(|| choice.get("content").and_then(extract_text_value))
        .or_else(|| choice.get("text").and_then(extract_text_value))
}

fn extract_message_text(message: &Value) -> Option<String> {
    [
        message.get("content").and_then(extract_content_text),
        message.get("text").and_then(extract_text_value),
        message.get("output_text").and_then(extract_text_value),
        message
            .get("reasoning_content")
            .and_then(extract_text_value),
    ]
    .into_iter()
    .flatten()
    .find(|text| !text.trim().is_empty())
}

fn extract_content_text(value: &Value) -> Option<String> {
    match value {
        Value::String(_) => extract_text_value(value),
        Value::Array(parts) => {
            let mut text = String::new();
            for part in parts {
                if let Some(part_text) = extract_content_part_text(part) {
                    text.push_str(&part_text);
                }
            }
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
        Value::Object(_) => value
            .get("text")
            .and_then(extract_text_value)
            .or_else(|| value.get("content").and_then(extract_text_value))
            .or_else(|| value.get("value").and_then(extract_text_value))
            .or_else(|| value.get("output_text").and_then(extract_text_value)),
        _ => None,
    }
}

fn extract_content_part_text(value: &Value) -> Option<String> {
    match value {
        Value::String(_) => extract_text_value(value),
        Value::Object(_) => value
            .get("text")
            .and_then(extract_text_value)
            .or_else(|| value.get("content").and_then(extract_text_value))
            .or_else(|| value.get("value").and_then(extract_text_value))
            .or_else(|| value.get("output_text").and_then(extract_text_value)),
        _ => None,
    }
}

fn extract_text_value(value: &Value) -> Option<String> {
    value.as_str().map(ToOwned::to_owned)
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    temperature: f32,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TranslationRequest;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    fn request(text: &str) -> TranslationRequest {
        TranslationRequest {
            artifact_name: "demo".into(),
            file_path: PathBuf::from("SKILL.md"),
            field: "body".into(),
            source_text: text.into(),
            locale: "zh".into(),
        }
    }

    fn ok_body(translated: &str) -> serde_json::Value {
        serde_json::json!({
            "choices": [{"message": {"role": "assistant", "content": translated}}]
        })
    }

    #[test]
    fn decode_response_accepts_array_content_without_role() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "content": [
                        {"type": "text", "text": "你"},
                        {"type": "text", "text": "好"}
                    ]
                }
            }]
        })
        .to_string();

        assert_eq!(decode_chat_completion_text(&body).unwrap(), "你好");
    }

    #[test]
    fn decode_response_accepts_choice_text_fallback() {
        let body = serde_json::json!({
            "choices": [{"text": "你好"}]
        })
        .to_string();

        assert_eq!(decode_chat_completion_text(&body).unwrap(), "你好");
    }

    #[test]
    fn decode_response_accepts_reasoning_content_fallback() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "",
                    "reasoning_content": "你好"
                }
            }]
        })
        .to_string();

        assert_eq!(decode_chat_completion_text(&body).unwrap(), "你好");
    }

    #[test]
    fn decode_response_accepts_nested_output_text_part() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "content": [{"type": "output_text", "output_text": "你好"}]
                }
            }]
        })
        .to_string();

        assert_eq!(decode_chat_completion_text(&body).unwrap(), "你好");
    }

    #[test]
    fn decode_response_rejects_empty_success_content() {
        let body = ok_body("").to_string();
        let err = decode_chat_completion_text(&body).unwrap_err();

        assert_eq!(err, "response had no translated text");
    }

    #[test]
    fn decode_response_rejects_whitespace_success_content() {
        let body = ok_body("  \n\t").to_string();
        let err = decode_chat_completion_text(&body).unwrap_err();

        assert_eq!(err, "response had no translated text");
    }

    #[test]
    fn decode_response_explains_html_success_body() {
        let body = "<!DOCTYPE html><html lang=\"zh-CN\"><head><title>Login</title></head></html>";
        let err = decode_chat_completion_text(body).unwrap_err();

        assert!(
            err.contains("expected JSON but received HTML"),
            "got: {err}"
        );
        assert!(
            err.contains("Base URL points to an OpenAI-compatible API root"),
            "got: {err}"
        );
    }

    #[test]
    fn request_body_preserves_prompt_shape_and_user_text() {
        let provider = OpenAICompatProvider::new(
            "https://api.example.test/v1/".into(),
            "test-model".into(),
            "test-key".into(),
            Duration::from_secs(5),
            0,
        )
        .unwrap();

        let source =
            "---\nname: demo\n---\n\n# Title\n\nUse `code` and [docs](https://example.com).";
        let body = provider.build_body(&TranslationRequest {
            artifact_name: "demo".into(),
            file_path: PathBuf::from("SKILL.md"),
            field: "body".into(),
            source_text: source.into(),
            locale: "zh-Hans".into(),
        });

        assert_eq!(
            provider.endpoint(),
            "https://api.example.test/v1/chat/completions"
        );
        assert_eq!(body.model, "test-model");
        assert_eq!(body.temperature, 0.2);
        assert_eq!(body.messages.len(), 2);
        assert_eq!(body.messages[0].role, "system");
        assert!(body.messages[0]
            .content
            .contains(&format!("Prompt version: {PROMPT_VERSION}")));
        assert!(body.messages[0].content.contains("target locale (zh-Hans)"));
        assert!(body.messages[0]
            .content
            .contains("Preserve every Markdown structure"));
        assert!(body.messages[0].content.contains("link target"));
        assert!(body.messages[0].content.contains("frontmatter key"));
        assert!(body.messages[0]
            .content
            .contains("output only the translated text"));
        assert_eq!(body.messages[1].role, "user");
        assert_eq!(body.messages[1].content, source);
    }

    struct CountingResponder {
        calls: Arc<AtomicUsize>,
        fail_first: usize,
        translated: String,
    }

    impl Respond for CountingResponder {
        fn respond(&self, _req: &Request) -> ResponseTemplate {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if n < self.fail_first {
                ResponseTemplate::new(503).set_body_string("upstream busy")
            } else {
                ResponseTemplate::new(200).set_body_json(ok_body(&self.translated))
            }
        }
    }

    #[tokio::test]
    async fn success_returns_translation() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_body("你好")))
            .mount(&server)
            .await;

        let provider = OpenAICompatProvider::new(
            server.uri(),
            "test-model".into(),
            "test-key".into(),
            Duration::from_secs(5),
            0,
        )
        .unwrap();
        let result = provider.translate(&request("Hello")).await.unwrap();
        assert_eq!(result, "你好");
    }

    #[tokio::test]
    async fn retries_on_5xx_then_succeeds() {
        let server = MockServer::start().await;
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(CountingResponder {
                calls: calls.clone(),
                fail_first: 2,
                translated: "你好".into(),
            })
            .mount(&server)
            .await;

        let provider = OpenAICompatProvider::new(
            server.uri(),
            "test-model".into(),
            "test-key".into(),
            Duration::from_secs(5),
            2,
        )
        .unwrap();
        let result = provider.translate(&request("Hello")).await.unwrap();
        assert_eq!(result, "你好");
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    struct AlwaysStatus {
        calls: Arc<AtomicUsize>,
        status: u16,
    }

    impl Respond for AlwaysStatus {
        fn respond(&self, _req: &Request) -> ResponseTemplate {
            self.calls.fetch_add(1, Ordering::SeqCst);
            ResponseTemplate::new(self.status).set_body_string("bad key")
        }
    }

    #[tokio::test]
    async fn no_retry_on_4xx() {
        let server = MockServer::start().await;
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(AlwaysStatus {
                calls: calls.clone(),
                status: 401,
            })
            .mount(&server)
            .await;

        let provider = OpenAICompatProvider::new(
            server.uri(),
            "test-model".into(),
            "test-key".into(),
            Duration::from_secs(5),
            3,
        )
        .unwrap();
        let err = provider.translate(&request("Hello")).await.unwrap_err();
        match err {
            SkillsMgrError::TranslateProvider { status, .. } => {
                assert_eq!(status, Some(401));
            }
            other => panic!("expected TranslateProvider, got {other:?}"),
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn chat_complete_returns_assistant_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(ok_body("{\"rating\":\"safe\"}")),
            )
            .mount(&server)
            .await;

        let provider = OpenAICompatProvider::new(
            server.uri(),
            "test-model".into(),
            "test-key".into(),
            Duration::from_secs(5),
            0,
        )
        .unwrap();
        let text = provider
            .chat_complete(
                vec![
                    ("system".into(), "You judge things.".into()),
                    ("user".into(), "Judge this.".into()),
                ],
                0.1,
            )
            .await
            .unwrap();
        assert_eq!(text, "{\"rating\":\"safe\"}");
    }

    #[tokio::test]
    async fn chat_complete_retries_5xx_then_succeeds() {
        let server = MockServer::start().await;
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(CountingResponder {
                calls: calls.clone(),
                fail_first: 1,
                translated: "ok".into(),
            })
            .mount(&server)
            .await;

        let provider = OpenAICompatProvider::new(
            server.uri(),
            "test-model".into(),
            "test-key".into(),
            Duration::from_secs(5),
            2,
        )
        .unwrap();
        let text = provider
            .chat_complete(vec![("user".into(), "hi".into())], 0.0)
            .await
            .unwrap();
        assert_eq!(text, "ok");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn chat_complete_propagates_4xx_without_retry() {
        let server = MockServer::start().await;
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(AlwaysStatus {
                calls: calls.clone(),
                status: 400,
            })
            .mount(&server)
            .await;

        let provider = OpenAICompatProvider::new(
            server.uri(),
            "test-model".into(),
            "test-key".into(),
            Duration::from_secs(5),
            3,
        )
        .unwrap();
        let err = provider
            .chat_complete(vec![("user".into(), "hi".into())], 0.0)
            .await
            .unwrap_err();
        match err {
            SkillsMgrError::TranslateProvider { status, .. } => {
                assert_eq!(status, Some(400));
            }
            other => panic!("expected TranslateProvider, got {other:?}"),
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
