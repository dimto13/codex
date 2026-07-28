use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

#[cfg(windows)]
const SIBLING_UPDATER_NAME: &str = "aren-update.cmd";
#[cfg(not(windows))]
const SIBLING_UPDATER_NAME: &str = "aren-update";
const PATH_UPDATER_NAME: &str = "aren-update";

pub(crate) fn run() -> anyhow::Result<()> {
    println!("Updating Aren via `aren-update`...");
    let current_executable = std::env::current_exe().ok();
    let updater_path = resolve_updater_path(current_executable.as_deref());
    let updater_display = updater_path.display();

    #[cfg(windows)]
    let status = {
        let parent_process_id = std::process::id().to_string();
        Command::new("cmd")
            .arg("/C")
            .arg(&updater_path)
            .args(["-ParentProcessId", parent_process_id.as_str()])
            .status()
    };
    #[cfg(not(windows))]
    let status = Command::new(&updater_path).status();

    let status =
        status.map_err(|error| anyhow::anyhow!("failed to start `{updater_display}`: {error}"))?;
    if !status.success() {
        anyhow::bail!("`{updater_display}` failed with status {status}");
    }
    println!("\nAren was updated successfully. Please restart Aren.");
    Ok(())
}

fn resolve_updater_path(current_executable: Option<&Path>) -> PathBuf {
    current_executable
        .and_then(Path::parent)
        .map(|parent| parent.join(SIBLING_UPDATER_NAME))
        .filter(|candidate| candidate.is_file())
        .unwrap_or_else(|| PathBuf::from(PATH_UPDATER_NAME))
}

#[cfg(test)]
#[path = "aren_update_tests.rs"]
mod tests;
