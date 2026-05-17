//! Live smoke test for OpenAICompatProvider. Reads credentials from env to avoid persisting them.
//!
//!     M_SKILLS_TRANSLATE_BASE_URL=https://api.deepseek.com/v1 \
//!     M_SKILLS_TRANSLATE_MODEL=deepseek-chat \
//!     M_SKILLS_TRANSLATE_API_KEY=sk-... \
//!     cargo run --example translate_smoke -p skillsmgr-translate

use std::path::PathBuf;
use std::time::Duration;

use skillsmgr_translate::{OpenAICompatProvider, TranslationProvider, TranslationRequest};

#[tokio::main]
async fn main() {
    let base_url =
        std::env::var("M_SKILLS_TRANSLATE_BASE_URL").expect("set M_SKILLS_TRANSLATE_BASE_URL");
    let model = std::env::var("M_SKILLS_TRANSLATE_MODEL").expect("set M_SKILLS_TRANSLATE_MODEL");
    let api_key =
        std::env::var("M_SKILLS_TRANSLATE_API_KEY").expect("set M_SKILLS_TRANSLATE_API_KEY");
    let source = std::env::var("M_SKILLS_TRANSLATE_TEXT").unwrap_or_else(|_| {
        "# Hello\n\nThis is a **short** test of the Skill translation pipeline.".into()
    });
    let locale = std::env::var("M_SKILLS_TRANSLATE_LOCALE").unwrap_or_else(|_| "zh".into());

    let provider = OpenAICompatProvider::new(base_url, model, api_key, Duration::from_secs(30), 2)
        .expect("build provider");

    let request = TranslationRequest {
        artifact_name: "smoke".into(),
        file_path: PathBuf::from("SKILL.md"),
        field: "body".into(),
        source_text: source,
        locale,
    };

    match provider.translate(&request).await {
        Ok(text) => {
            println!("--- source ---");
            println!("{}", request.source_text);
            println!("--- translated ---");
            println!("{text}");
        }
        Err(e) => {
            eprintln!("translation failed: {e}");
            std::process::exit(1);
        }
    }
}
