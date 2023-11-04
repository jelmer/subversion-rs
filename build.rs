extern crate bindgen;
extern crate pkg_config;

fn create_svn_bindings(out_path: &std::path::Path) {
    let pc_svn = pkg_config::Config::new()
        .probe("libsvn_client")
        .unwrap_or_else(|e| panic!("Failed to find svn library: {}", e));

    let svn_path = pc_svn
        .include_paths
        .iter()
        .find(|x| x.join("svn_client.h").exists())
        .expect("Failed to find svn_client.h");

    // Generate bindings using bindgen
    let svn_bindings = bindgen::Builder::default()
        .header(svn_path.join("svn_client.h").to_str().unwrap())
        .header(svn_path.join("svn_dirent_uri.h").to_str().unwrap())
        .header(svn_path.join("svn_version.h").to_str().unwrap())
        .header(svn_path.join("svn_error.h").to_str().unwrap())
        .header(svn_path.join("svn_opt.h").to_str().unwrap())
        .header(svn_path.join("svn_repos.h").to_str().unwrap())
        .header(svn_path.join("svn_time.h").to_str().unwrap())
        .header(svn_path.join("svn_types.h").to_str().unwrap())
        .header(svn_path.join("svn_types_impl.h").to_str().unwrap())
        .allowlist_file(".*/svn_.*.h")
        .blocklist_type("apr_.*")
        .derive_default(true)
        .clang_args(
            pc_svn
                .include_paths
                .iter()
                .map(|path| format!("-I{}", path.display())),
        )
        .generate()
        .expect("Failed to generate bindings");

    svn_bindings
        .write_to_file(out_path.join("subversion.rs"))
        .expect("Failed to write bindings");
}

fn main() {
    system_deps::Config::new().probe().unwrap();

    let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    create_svn_bindings(out_path.as_path());
}
