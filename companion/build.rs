// The Windows .exe carries the app icon and version details (Explorer, the
// taskbar, Installed apps). Nothing to do for other targets.
fn main() {
    println!("cargo:rerun-if-changed=../packaging/windows/hawaos.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../packaging/windows/hawaos.ico")
        .set("ProductName", "HawaOS Companion")
        .set("FileDescription", "HawaOS Companion")
        .set("CompanyName", "TheCodeHeist");
    if let Err(e) = res.compile() {
        println!("cargo:warning=no icon embedded in the .exe: {e}");
    }
}
