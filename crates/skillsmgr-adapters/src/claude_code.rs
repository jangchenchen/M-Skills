use std::path::PathBuf;

use skillsmgr_core::Target;

use crate::DirectoryLayout;

pub fn adapter(home: impl Into<PathBuf>) -> DirectoryLayout {
    let home = home.into();
    DirectoryLayout::skill(
        "claude-code",
        |scope| Target::ClaudeCode { scope },
        home.join(".claude/skills"),
        ".claude/skills",
    )
    .with_flat_commands(home.join(".claude/commands"), ".claude/commands")
}
