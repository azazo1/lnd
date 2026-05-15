fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");

    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir");
    let output = std::path::Path::new(&crate_dir)
        .join("include")
        .join("lnd.h");
    if let Some(parent) = output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let config = cbindgen::Config::from_root_or_default(&crate_dir);
    if let Err(error) = cbindgen::generate_with_config(&crate_dir, config).map(|bindings| {
        bindings.write_to_file(&output);
    }) {
        panic!("failed to generate header: {error}");
    }
}
