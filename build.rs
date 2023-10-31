extern crate bindgen;
extern crate pkg_config;

fn main() {
    system_deps::Config::new().probe().unwrap();

    let pc_svn = pkg_config::Config::new()
        .probe("libsvn_client")
        .unwrap_or_else(|e| panic!("Failed to find svn library: {}", e));

    let svn_path = pc_svn
        .include_paths
        .iter()
        .find(|x| x.join("svn_client.h").exists())
        .expect("Failed to find svn_client.h");

    // Generate bindings using bindgen
    let bindings = bindgen::Builder::default()
        .header(svn_path.join("svn_client.h").to_str().unwrap())
        .header(svn_path.join("svn_version.h").to_str().unwrap())
        .allowlist_file(".*/svn_client.h")
        .allowlist_file(".*/svn_version.h")
        .clang_args(
            pc_svn
                .include_paths
                .iter()
                .map(|path| format!("-I{}", path.display())),
        )
        .generate()
        .expect("Failed to generate bindings");

    let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("subversion.rs"))
        .expect("Failed to write bindings");
}
