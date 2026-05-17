use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use skillsmgr_core::{Result, SkillsMgrError};

use crate::{TranslationProvider, TranslationRequest};

const PROVIDER_KIND: &str = "openai-compat";
const SYSTEM_PROMPT_TEMPLATE: &str = "You are a precise technical translator. \
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
        let system_prompt = SYSTEM_PROMPT_TEMPLATE.replace("{{locale}}", &request.locale);
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
            return match response.json::<ChatResponse>().await {
                Ok(parsed) => match parsed.choices.into_iter().next() {
                    Some(choice) => AttemptOutcome::Done(Ok(choice.message.content)),
                    None => AttemptOutcome::Done(Err(SkillsMgrError::TranslateProvider {
                        kind: PROVIDER_KIND.into(),
                        status: Some(status_code),
                        message: "response had no choices".into(),
                    })),
                },
                Err(e) => AttemptOutcome::Done(Err(SkillsMgrError::TranslateProvider {
                    kind: PROVIDER_KIND.into(),
                    status: Some(status_code),
                    message: format!("decode response: {e}"),
                })),
            };
        }

        let body_text = response.text().await.unwrap_or_default();
        let err = SkillsMgrError::TranslateProvider {
            kind: PROVIDER_KIND.into(),
            status: Some(status_code),
            message: truncate_body(&body_text),
        };
        if status.is_server_error() {
            AttemptOutcome::Retryable(err)
        } else {
            AttemptOutcome::Done(Err(err))
        }
    }
}

#[async_trait]
impl TranslationProvider for OpenAICompatProvider {
    async fn translate(&self, request: &TranslationRequest) -> Result<String> {
        let body = self.build_body(request);
        let mut last_err: Option<SkillsMgrError> = None;
        for attempt in 0..=self.max_retries {
            match self.attempt(&body).await {
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

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
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
}
