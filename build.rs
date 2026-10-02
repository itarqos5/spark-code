// SPDX-License-Identifier: MIT
fn main() {
    println!("cargo:rerun-if-changed=assets/branding/spark-code.ico");
    println!("cargo:rerun-if-changed=Cargo.toml");

    // Check the target, not cfg!(windows): the build script also runs on Linux
    // when cross-compiling the Windows executables.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/branding/spark-code.ico")
            .set_language(0x0409)
            .set("ProductName", "Spark Code")
            .set("FileDescription", "Spark Code")
            .set("InternalName", "spark-code")
            .set("CompanyName", "Spark Code contributors")
            .set("LegalCopyright", "MIT-licensed Spark Code contributors")
            .set("ProductVersion", env!("CARGO_PKG_VERSION"))
            .set("FileVersion", env!("CARGO_PKG_VERSION"));
        resource
            .compile()
            .expect("compile Spark Code Windows icon and version resources");
    }
}
