use std::path::PathBuf;

use skillsmgr_core::Target;

use crate::DirectoryLayout;

pub fn adapter(home: impl Into<PathBuf>) -> DirectoryLayout {
    DirectoryLayout::skill(
        "opencode",
        |scope| Target::Opencode { scope },
        home.into().join(".config/opencode/skills"),
        ".opencode/skills",
    )
}
