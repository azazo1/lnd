fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=../../../cbindgen.toml");

    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir");
    let crate_dir = std::path::PathBuf::from(crate_dir);
    let workspace_root = crate_dir
        .ancestors()
        .nth(3)
        .expect("missing workspace root");
    let output = workspace_root.join("include").join("lnd.h");
    if let Some(parent) = output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let config_path = workspace_root.join("cbindgen.toml");
    let config = cbindgen::Config::from_file(&config_path)
        .unwrap_or_else(|error| panic!("failed to load {}: {error}", config_path.display()));
    if let Err(error) = cbindgen::generate_with_config(&crate_dir, config).map(|bindings| {
        bindings.write_to_file(&output);
    }) {
        panic!("failed to generate header: {error}");
    }
}
