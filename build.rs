use std::process::Command;
use std::result::Result;
pub fn main() -> Result<(), std::io::Error> {
    let current_dir = std::env::current_dir()?;
    let frontend_dir = current_dir.join("alu-panel");
    println!("cargo::rerun-if-changed={}", frontend_dir.display());
    Command::new("pnpm")
        .arg("install")
        .current_dir(&frontend_dir)
        .status()?;

    Command::new("pnpm")
        .arg("build")
        .current_dir(&frontend_dir)
        .status()?;

    // tracing::info!("frontend built");
    Ok(())
}
