extern crate bindgen;

fn create_svn_bindings(
    svn_path: &std::path::Path,
    out_path: &std::path::Path,
    include_paths: &[&std::path::Path],
) {
    let client_feature_enabled = std::env::var("CARGO_FEATURE_CLIENT").is_ok();

    let mut builder = bindgen::Builder::default()
        .header(svn_path.join("svn_dirent_uri.h").to_str().unwrap())
        .header(svn_path.join("svn_version.h").to_str().unwrap())
        .header(svn_path.join("svn_error.h").to_str().unwrap())
        .header(svn_path.join("svn_opt.h").to_str().unwrap())
        .header(svn_path.join("svn_repos.h").to_str().unwrap())
        .header(svn_path.join("svn_time.h").to_str().unwrap())
        .header(svn_path.join("svn_types.h").to_str().unwrap())
        .header(svn_path.join("svn_types_impl.h").to_str().unwrap())
        .header(svn_path.join("svn_wc.h").to_str().unwrap())
        .header(svn_path.join("svn_props.h").to_str().unwrap())
        .allowlist_file(".*/svn_.*.h")
        .blocklist_type("apr_.*")
        .derive_default(true)
        .clang_args(
            include_paths
                .iter()
                .map(|path| format!("-I{}", path.display())),
        );

    if client_feature_enabled {
        builder = builder.header(svn_path.join("svn_client.h").to_str().unwrap());
    }

    // Generate bindings using bindgen
    let svn_bindings = builder.generate().expect("Failed to generate bindings");

    svn_bindings
        .write_to_file(out_path.join("subversion.rs"))
        .expect("Failed to write bindings");
}

fn main() {
    let deps = system_deps::Config::new().probe().unwrap();

    let svn = deps.get_by_name("libsvn_subr").unwrap();

    let svn_path = svn
        .include_paths
        .iter()
        .find(|x| x.join("svn_config.h").exists())
        .expect("Failed to find svn_config.h");

    let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    create_svn_bindings(
        svn_path.as_path(),
        out_path.as_path(),
        svn.include_paths
            .iter()
            .map(|x| x.as_path())
            .collect::<Vec<_>>()
            .as_slice(),
    );
}
