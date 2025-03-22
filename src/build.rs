use std::process::Command;
use std::result::Result;
pub async fn build_frontend() -> Result<(), std::io::Error> {
    let current_dir = std::env::current_dir()?;
    let frontend_dir = current_dir.join("alu-panel");

    std::env::set_current_dir(&frontend_dir)?;

    Command::new("pnpm").arg("install").status()?;

    Command::new("pnpm").arg("build").status()?;

    std::env::set_current_dir(current_dir)?;
    tracing::info!("frontend built");
    Ok(())
}
