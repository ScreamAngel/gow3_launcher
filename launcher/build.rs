fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("gow3.ico");
        res.set("ProductName", "God of War III Remastered Launcher");
        res.set("FileDescription", "God of War III Remastered PC Launcher");
        res.set("LegalCopyright", "God of War III Remastered");
        if let Err(e) = res.compile() {
            eprintln!("Warning: failed to compile Windows icon resource: {}", e);
        }
    }
}
