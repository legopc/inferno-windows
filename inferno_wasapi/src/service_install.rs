pub fn install_service() -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    std::process::Command::new("sc")
        .args(["create", "InfernoAoIP",
               &format!("binPath={} --service", exe.display()),
               "start=auto",
               "DisplayName=InfernoAoIP Dante Virtual Soundcard"])
        .status()?;
    tracing::info!("Windows Service installed. Start with: sc start InfernoAoIP");
    Ok(())
}

pub fn uninstall_service() -> std::io::Result<()> {
    std::process::Command::new("sc").args(["stop", "InfernoAoIP"]).status().ok();
    std::process::Command::new("sc").args(["delete", "InfernoAoIP"]).status()?;
    tracing::info!("Windows Service uninstalled");
    Ok(())
}
