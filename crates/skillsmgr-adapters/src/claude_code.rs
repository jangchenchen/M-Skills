use std::path::PathBuf;

use skillsmgr_core::Target;

use crate::DirectoryLayout;

pub fn adapter(home: impl Into<PathBuf>) -> DirectoryLayout {
    DirectoryLayout::skill(
        "claude-code",
        |scope| Target::ClaudeCode { scope },
        home.into().join(".claude/skills"),
        ".claude/skills",
    )
}
