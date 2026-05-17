//! Lightweight check that a translation preserved markdown structure.
//!
//! The translator is instructed not to touch code blocks, frontmatter, links,
//! headings, or list bullets. This module verifies that the output respects
//! those constraints by comparing simple structural counts and the bytes of
//! anything that should be byte-equal.
//!
//! Only emits warnings — never blocks display. The UI offers a "retranslate"
//! action when warnings are present.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MarkdownWarning {
    FencedCodeBlockCount { source: usize, translated: usize },
    LinkCount { source: usize, translated: usize },
    HeadingCount { source: usize, translated: usize },
    ListItemCount { source: usize, translated: usize },
    CodeBlockContentChanged { index: usize },
    FrontmatterChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationValidation {
    pub ok: bool,
    pub warnings: Vec<MarkdownWarning>,
}

impl TranslationValidation {
    pub fn ok() -> Self {
        TranslationValidation {
            ok: true,
            warnings: Vec::new(),
        }
    }
}

pub fn validate_markdown_fidelity(source: &str, translated: &str) -> TranslationValidation {
    let mut warnings = Vec::new();

    let src_blocks = extract_code_blocks(source);
    let dst_blocks = extract_code_blocks(translated);
    if src_blocks.len() != dst_blocks.len() {
        warnings.push(MarkdownWarning::FencedCodeBlockCount {
            source: src_blocks.len(),
            translated: dst_blocks.len(),
        });
    } else {
        for (index, (a, b)) in src_blocks.iter().zip(dst_blocks.iter()).enumerate() {
            if a != b {
                warnings.push(MarkdownWarning::CodeBlockContentChanged { index });
            }
        }
    }

    let src_links = count_links(source);
    let dst_links = count_links(translated);
    if src_links != dst_links {
        warnings.push(MarkdownWarning::LinkCount {
            source: src_links,
            translated: dst_links,
        });
    }

    let src_headings = count_headings(source);
    let dst_headings = count_headings(translated);
    if src_headings != dst_headings {
        warnings.push(MarkdownWarning::HeadingCount {
            source: src_headings,
            translated: dst_headings,
        });
    }

    let src_list = count_list_items(source);
    let dst_list = count_list_items(translated);
    if src_list != dst_list {
        warnings.push(MarkdownWarning::ListItemCount {
            source: src_list,
            translated: dst_list,
        });
    }

    let src_fm = extract_frontmatter(source);
    if let Some(src_fm) = src_fm {
        let dst_fm = extract_frontmatter(translated);
        if dst_fm.as_deref() != Some(&src_fm) {
            warnings.push(MarkdownWarning::FrontmatterChanged);
        }
    }

    TranslationValidation {
        ok: warnings.is_empty(),
        warnings,
    }
}

/// Extract every fenced code block body. Recognises ``` and ~~~ fences and
/// matches them only with the same fence character. Bodies are returned as
/// joined lines (newline-separated). Unclosed blocks still produce a body
/// entry so a closing-fence regression shows up as a count mismatch.
fn extract_code_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<(String, &'static str)> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        match &mut current {
            None => {
                if trimmed.starts_with("```") {
                    current = Some((String::new(), "```"));
                } else if trimmed.starts_with("~~~") {
                    current = Some((String::new(), "~~~"));
                }
            }
            Some((body, fence)) => {
                if trimmed.starts_with(*fence) {
                    blocks.push(std::mem::take(body));
                    current = None;
                } else {
                    if !body.is_empty() {
                        body.push('\n');
                    }
                    body.push_str(line);
                }
            }
        }
    }
    if let Some((body, _)) = current {
        blocks.push(body);
    }
    blocks
}

/// Count occurrences of `](`. Approximate but consistent for both source and
/// translated, which is what we care about. Both texts overcount in identical
/// ways (e.g. inside code blocks the translator preserves verbatim).
fn count_links(text: &str) -> usize {
    text.matches("](").count()
}

/// Count ATX-style headings: 1–6 leading `#` followed by a space or end of line.
fn count_headings(text: &str) -> usize {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
            if hash_count == 0 || hash_count > 6 {
                return false;
            }
            match trimmed.as_bytes().get(hash_count) {
                None => true,
                Some(b' ') => true,
                _ => false,
            }
        })
        .count()
}

/// Count list item lines: `-`/`*`/`+` or `<digits>.` followed by a space, after
/// optional indentation.
fn count_list_items(text: &str) -> usize {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            let bytes = trimmed.as_bytes();
            if bytes.len() < 2 {
                return false;
            }
            if matches!(bytes[0], b'-' | b'*' | b'+') {
                return bytes[1] == b' ';
            }
            let digit_count = bytes.iter().take_while(|&&b| b.is_ascii_digit()).count();
            if digit_count == 0 {
                return false;
            }
            bytes.get(digit_count) == Some(&b'.') && bytes.get(digit_count + 1) == Some(&b' ')
        })
        .count()
}

/// If the text begins with a `---` fence on its first line, return the bytes
/// between that fence and the next `---` fence. Returns None if there is no
/// opening fence or the block is unclosed.
fn extract_frontmatter(text: &str) -> Option<String> {
    let mut lines = text.lines();
    let first = lines.next()?;
    if first.trim_end() != "---" {
        return None;
    }
    let mut fm = String::new();
    for line in lines {
        if line.trim_end() == "---" {
            return Some(fm);
        }
        if !fm.is_empty() {
            fm.push('\n');
        }
        fm.push_str(line);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_text_validates_clean() {
        let body = "# Hello\n\nSome [link](https://example.com).\n\n```rs\nfn main() {}\n```\n";
        let v = validate_markdown_fidelity(body, body);
        assert!(v.ok);
        assert!(v.warnings.is_empty());
    }

    #[test]
    fn plain_text_with_no_markdown_validates_clean() {
        let v = validate_markdown_fidelity("Hello", "你好");
        assert!(v.ok);
        assert!(v.warnings.is_empty());
    }

    #[test]
    fn dropped_code_fence_emits_count_warning() {
        let src = "Before\n```rs\nfn main() {}\n```\nAfter";
        let dst = "之前\nfn main() {}\n之后";
        let v = validate_markdown_fidelity(src, dst);
        assert!(!v.ok);
        assert!(v.warnings.iter().any(|w| matches!(
            w,
            MarkdownWarning::FencedCodeBlockCount {
                source: 1,
                translated: 0
            }
        )));
    }

    #[test]
    fn changed_code_block_body_emits_index_warning() {
        let src = "```rs\nfn main() {}\n```";
        let dst = "```rs\nfn 主函数() {}\n```";
        let v = validate_markdown_fidelity(src, dst);
        assert!(!v.ok);
        assert_eq!(
            v.warnings,
            vec![MarkdownWarning::CodeBlockContentChanged { index: 0 }]
        );
    }

    #[test]
    fn missing_link_emits_link_count_warning() {
        let src = "See [docs](https://example.com).";
        let dst = "请参阅文档。";
        let v = validate_markdown_fidelity(src, dst);
        assert!(v.warnings.iter().any(|w| matches!(
            w,
            MarkdownWarning::LinkCount {
                source: 1,
                translated: 0
            }
        )));
    }

    #[test]
    fn changed_heading_count_emits_warning() {
        let src = "# A\n\n## B\n\n## C";
        let dst = "# A\n\n## B";
        let v = validate_markdown_fidelity(src, dst);
        assert!(v.warnings.iter().any(|w| matches!(
            w,
            MarkdownWarning::HeadingCount {
                source: 3,
                translated: 2
            }
        )));
    }

    #[test]
    fn list_item_count_is_compared() {
        let src = "- one\n- two\n- three";
        let dst = "- 一\n- 二";
        let v = validate_markdown_fidelity(src, dst);
        assert!(v.warnings.iter().any(|w| matches!(
            w,
            MarkdownWarning::ListItemCount {
                source: 3,
                translated: 2
            }
        )));
    }

    #[test]
    fn ordered_lists_also_counted() {
        let src = "1. first\n2. second\n3. third";
        let dst = "1. 第一\n2. 第二\n3. 第三";
        let v = validate_markdown_fidelity(src, dst);
        assert!(v.ok, "ordered lists with same count must not warn");
    }

    #[test]
    fn hashes_inside_word_are_not_headings() {
        let src = "Use the #channel in slack";
        let dst = "在 slack 中使用 #channel";
        let v = validate_markdown_fidelity(src, dst);
        assert!(
            v.ok,
            "non-heading hashes should not be counted, got warnings: {:?}",
            v.warnings
        );
    }

    #[test]
    fn frontmatter_byte_change_emits_warning() {
        let src = "---\nname: foo\ndescription: A skill\n---\n\nBody";
        let dst = "---\nname: 富\ndescription: 一个技能\n---\n\n正文";
        let v = validate_markdown_fidelity(src, dst);
        assert!(v
            .warnings
            .iter()
            .any(|w| matches!(w, MarkdownWarning::FrontmatterChanged)));
    }

    #[test]
    fn frontmatter_preserved_does_not_warn() {
        let src = "---\nname: foo\ndescription: A skill\n---\n\nBody";
        let dst = "---\nname: foo\ndescription: A skill\n---\n\n正文";
        let v = validate_markdown_fidelity(src, dst);
        assert!(
            v.warnings
                .iter()
                .all(|w| !matches!(w, MarkdownWarning::FrontmatterChanged)),
            "frontmatter byte-equal should not warn, got: {:?}",
            v.warnings
        );
    }

    #[test]
    fn no_frontmatter_in_source_means_no_frontmatter_warning_even_if_dst_adds_one() {
        // If source has no frontmatter, we don't try to compare. This keeps
        // the check focused on "translator didn't corrupt existing frontmatter".
        let src = "Hello";
        let dst = "---\nadded: true\n---\n你好";
        let v = validate_markdown_fidelity(src, dst);
        assert!(
            v.warnings
                .iter()
                .all(|w| !matches!(w, MarkdownWarning::FrontmatterChanged)),
            "no source frontmatter → no FrontmatterChanged warning"
        );
    }

    #[test]
    fn tilde_fences_recognised() {
        let src = "~~~rs\nfn main() {}\n~~~";
        let dst = "~~~rs\nfn main() {}\n~~~";
        let v = validate_markdown_fidelity(src, dst);
        assert!(v.ok);
    }

    #[test]
    fn warning_serializes_as_camel_case_tagged() {
        let w = MarkdownWarning::FencedCodeBlockCount {
            source: 2,
            translated: 1,
        };
        let json = serde_json::to_string(&w).unwrap();
        assert!(
            json.contains("\"kind\":\"fencedCodeBlockCount\""),
            "got: {json}"
        );
        assert!(json.contains("\"source\":2"), "got: {json}");
        assert!(json.contains("\"translated\":1"), "got: {json}");

        let w2 = MarkdownWarning::FrontmatterChanged;
        let json2 = serde_json::to_string(&w2).unwrap();
        assert_eq!(json2, "{\"kind\":\"frontmatterChanged\"}");
    }
}
