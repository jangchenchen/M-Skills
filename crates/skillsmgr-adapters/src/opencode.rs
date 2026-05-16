use std::path::PathBuf;

use skillsmgr_core::Target;

use crate::{DirectoryLayout, SourceRoot};

pub fn adapter(home: impl Into<PathBuf>) -> DirectoryLayout {
    let home = home.into();
    DirectoryLayout::skill_with_roots(
        "opencode",
        |scope| Target::Opencode { scope },
        vec![
            SourceRoot::owned(home.join(".config/opencode/skills")),
            SourceRoot::shared(home.join(".claude/skills"), "claude-code"),
            SourceRoot::shared(home.join(".agents/skills"), "shared-global"),
        ],
        ".opencode/skills",
    )
}
