fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("gow3.ico");
        res.set("ProductName", "God of War III Remastered Launcher");
        res.set("FileDescription", "God of War III Remastered PC Launcher");
        res.set("LegalCopyright", "God of War III Remastered");
        res.set("ProductVersion", "1.8.5.0");
        res.set("FileVersion", "1.8.5.0");
        if let Err(e) = res.compile() {
            eprintln!("Warning: failed to compile Windows icon resource: {}", e);
        }
    }
}
