use std::sync::Arc;

use colored::Colorize;
use paws_api::{API, Update};
const VERSION: &str = env!("CARGO_PKG_VERSION");
use update_informer::{Check, Version, registry};

/// Package name for paws on npm.
const FORGE_NPM_PACKAGE: &str = "pawscode";

/// Runs npm update in the background, failing silently
async fn execute_update_command(api: Arc<impl API>) {
    // Spawn a new task that won't block the main application
    let output = api
        .execute_shell_command_raw(&format!("npm update -g {FORGE_NPM_PACKAGE} --force"))
        .await;

    match output {
        Err(err) => {
            tracing::error!(error = ?err, "Auto update failed");
        }
        Ok(output) => {
            if output.success() {
                let answer = paws_common::select::PawsSelect::confirm(
                    "You need to close paws to complete update. Do you want to close it now?",
                )
                .with_default(true)
                .prompt();
                if answer.unwrap_or_default().unwrap_or_default() {
                    std::process::exit(0);
                }
            } else {
                let exit_output = match output.code() {
                    Some(code) => format!("Process exited with code: {code}"),
                    None => "Process exited without code".to_string(),
                };
                tracing::error!(error = exit_output, "Auto update failed");
            }
        }
    }
}

async fn confirm_update(version: Version) -> bool {
    let answer = paws_common::select::PawsSelect::confirm(format!(
        "Confirm upgrade from {} -> {} (latest)?",
        VERSION.to_string().bold().white(),
        version.to_string().bold().white()
    ))
    .with_default(true)
    .prompt();

    match answer {
        Ok(Some(result)) => result,
        Ok(None) => false, // User canceled
        Err(_) => false,   // Error occurred
    }
}

/// Checks if there is an update available
pub async fn on_update(api: Arc<impl API>, update: Option<&Update>) {
    let update = update.cloned().unwrap_or_default();
    let frequency = update.frequency.unwrap_or_default();
    let auto_update = update.auto_update.unwrap_or_default();

    // Check if version is development version, in which case we skip the update
    // check
    if VERSION.contains("dev") || VERSION == "0.1.0" {
        // Skip update for development version 0.1.0
        return;
    }

    let informer =
        update_informer::new(registry::Npm, FORGE_NPM_PACKAGE, VERSION).interval(frequency.into());

    if let Some(version) = informer.check_version().ok().flatten()
        && (auto_update || confirm_update(version).await)
    {
        execute_update_command(api).await;
    }
}
