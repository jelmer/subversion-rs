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

    let pc_apr = pkg_config::Config::new()
        .probe("apr-1")
        .unwrap_or_else(|e| panic!("Failed to find apr library: {}", e));

    let apr_path = pc_apr
        .include_paths
        .iter()
        .find(|x| x.join("apr.h").exists())
        .expect("Failed to find apr.h");

    // Generate bindings using bindgen
    let svn_bindings = bindgen::Builder::default()
        .header(svn_path.join("svn_client.h").to_str().unwrap())
        .header(svn_path.join("svn_version.h").to_str().unwrap())
        .header(svn_path.join("svn_error.h").to_str().unwrap())
        .header(apr_path.join("apr.h").to_str().unwrap())
        .header(apr_path.join("apr_allocator.h").to_str().unwrap())
        .header(apr_path.join("apr_general.h").to_str().unwrap())
        .header(apr_path.join("apr_errno.h").to_str().unwrap())
        .header(apr_path.join("apr_pools.h").to_str().unwrap())
        .header(apr_path.join("apr_version.h").to_str().unwrap())
        .allowlist_file(".*/svn_client.h")
        .allowlist_file(".*/svn_version.h")
        .allowlist_file(".*/svn_error.h")
        .allowlist_file(".*/apr.h")
        .allowlist_file(".*/apr_general.h")
        .allowlist_file(".*/apr_allocator.h")
        .allowlist_file(".*/apr_version.h")
        .allowlist_file(".*/apr_errno.h")
        .allowlist_file(".*/apr_pools.h")
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
