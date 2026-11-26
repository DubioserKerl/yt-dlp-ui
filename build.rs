fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=resources/yt-dlp-ui.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("resources/yt-dlp-ui.ico")
            .compile()?;
    }

    Ok(())
}
