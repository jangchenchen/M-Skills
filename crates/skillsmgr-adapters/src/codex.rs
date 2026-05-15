use std::path::PathBuf;

use skillsmgr_core::Target;

use crate::DirectoryLayout;

pub fn adapter(home: impl Into<PathBuf>) -> DirectoryLayout {
    DirectoryLayout::skill(
        "codex",
        |scope| Target::Codex { scope },
        home.into().join(".agents/skills"),
        ".agents/skills",
    )
}

pub fn shared_global_adapter(home: impl Into<PathBuf>) -> DirectoryLayout {
    DirectoryLayout::skill(
        "shared-global",
        |_scope| Target::SharedGlobal,
        home.into().join(".agents/skills"),
        ".agents/skills",
    )
}
