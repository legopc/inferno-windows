fn main() {
    // Embed Windows application manifest (activates Common Controls v6 for NWG).
    // Without this, GetWindowsSubclass is missing and the GUI fails to start.
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        embed_resource::compile("inferno_gui.rc", embed_resource::NONE);
    }
}
