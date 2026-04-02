fn main() {
    // Embed app.manifest (DPI awareness, Common Controls v6, execution level)
    let _ = embed_resource::compile("app.rc", embed_resource::NONE);

    // Tell the linker to use the Windows subsystem (no console window)
    println!("cargo:rustc-link-arg=/SUBSYSTEM:WINDOWS");
    println!("cargo:rustc-link-arg=/ENTRY:mainCRTStartup");
}
