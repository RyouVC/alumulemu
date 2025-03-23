use std::process::Command;
use std::result::Result;
pub fn main() -> Result<(), std::io::Error> {
    let current_dir = std::env::current_dir()?;
    let frontend_dir = current_dir.join("alu-panel");
    // println!("cargo::rerun-if-changed={}", frontend_dir.display());
    // Tell Cargo to only rebuild when specific files change
    // (avoiding alu-panel/dist)
    println!("cargo:rerun-if-changed=build.rs");
    println!(
        "cargo:rerun-if-changed={}",
        frontend_dir.join("src").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        frontend_dir.join("public").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        frontend_dir.join("package.json").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        frontend_dir.join("pnpm-lock.yaml").display()
    );
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
