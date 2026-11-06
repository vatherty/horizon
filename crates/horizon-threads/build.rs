fn main() {
    println!("cargo:rerun-if-changed=src/process_memory_ios.c");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
        cc::Build::new()
            .file("src/process_memory_ios.c")
            .warnings_into_errors(true)
            .compile("horizon_process_memory_ios");
    }
}
