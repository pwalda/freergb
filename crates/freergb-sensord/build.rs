// Version information in freergb-sensord.exe's properties: Windows shows it in Services and
// Task Manager, and code signing (SignPath) requires the product name and version on every
// signed file. Only a Windows host has the resource compiler, so a cross-lint skips it.
fn main() {
    if cfg!(windows) && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        tauri_winres::WindowsResource::new()
            .set("ProductName", "freeRGB")
            .set("FileDescription", "freeRGB sensor service")
            .set("OriginalFilename", "freergb-sensord.exe")
            .set("LegalCopyright", "GPL-2.0-or-later")
            .compile()
            .expect("compile the Windows version resource");
    }
}
