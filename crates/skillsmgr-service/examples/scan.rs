use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use skillsmgr_core::AdapterPresence;
use skillsmgr_service::Service;

#[tokio::main]
async fn main() -> ExitCode {
    let Some(home) = home_dir() else {
        eprintln!("error: HOME environment variable is not set");
        return ExitCode::FAILURE;
    };

    let cwd = env::current_dir().ok();
    let service = Service::with_home(&home);
    let inventory = service.inventory(cwd.as_deref()).await;

    println!("HOME: {}", home.display());
    if let Some(cwd) = cwd.as_ref() {
        println!("CWD : {}", cwd.display());
    }
    println!();

    println!("Adapters ({}):", inventory.adapters.len());
    for adapter in &inventory.adapters {
        match &adapter.presence {
            AdapterPresence::Available => {
                println!("  + {:<14} available", adapter.adapter_id);
            }
            AdapterPresence::Missing { reason } => {
                println!("  - {:<14} missing ({reason})", adapter.adapter_id);
            }
        }
    }
    println!();

    if inventory.groups.is_empty() {
        println!("No artifacts found.");
    } else {
        println!("Artifacts ({}):", inventory.groups.len());
        for group in &inventory.groups {
            let version = group.version.as_deref().unwrap_or("-");
            println!(
                "  [{kind:?}] {name} ({version}) -- {description}",
                kind = group.kind,
                name = group.name,
                version = version,
                description = truncate(&group.description, 60),
            );
            for installation in &group.installations {
                let scope_label = match installation.installation.target.scope() {
                    Some(skillsmgr_core::Scope::Global) => "global".to_string(),
                    Some(skillsmgr_core::Scope::Project(path)) => {
                        format!("project={}", path.display())
                    }
                    None => "n/a".to_string(),
                };
                println!(
                    "      - {tool:<14} {scope:<28} {path}",
                    tool = installation.installation.target.tool_id(),
                    scope = scope_label,
                    path = installation.installation.on_disk_path.display(),
                );
            }
            if !group.also_visible_to.is_empty() {
                println!("        visible to: {}", group.also_visible_to.join(", "));
            }
            if !group.capabilities.is_empty() {
                let names: Vec<&str> = group.capabilities.iter().map(|c| c.name.as_str()).collect();
                println!("        capabilities: {}", names.join(", "));
            }
        }
    }

    if !inventory.errors.is_empty() {
        println!();
        println!("Errors ({}):", inventory.errors.len());
        for error in &inventory.errors {
            println!(
                "  ! {} ({:?}): {}",
                error.adapter_id, error.scope, error.message
            );
        }
    }

    ExitCode::SUCCESS
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}
