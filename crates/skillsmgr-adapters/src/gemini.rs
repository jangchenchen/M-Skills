use std::path::PathBuf;

use skillsmgr_core::Target;

use crate::DirectoryLayout;

pub fn adapter(home: impl Into<PathBuf>) -> DirectoryLayout {
    DirectoryLayout::extension(
        "gemini",
        |scope| Target::Gemini { scope },
        home.into().join(".gemini/extensions"),
        ".gemini/extensions",
    )
}
