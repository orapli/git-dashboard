fn main() {
    // Embed the exe icon and version metadata when building for Windows
    // (works both natively and when cross-compiling with mingw-w64).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Git Dashboard");
        res.set("FileDescription", "Git Repository Analysis Dashboard");
        res.set("LegalCopyright", "");
        if let Err(e) = res.compile() {
            // Don't fail the build over metadata; the exe just won't have an icon
            println!("cargo:warning=Windows resource embedding failed: {e}");
        }
    }
}
