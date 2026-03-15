extern crate bindgen;

fn create_svn_bindings(
    svn_path: &std::path::Path,
    out_path: &std::path::Path,
    include_paths: &[&std::path::Path],
) {
    let client_feature_enabled = std::env::var("CARGO_FEATURE_CLIENT").is_ok();
    let delta_feature_enabled = std::env::var("CARGO_FEATURE_DELTA").is_ok();
    let ra_feature_enabled = std::env::var("CARGO_FEATURE_RA").is_ok();
    let wc_feature_enabled = std::env::var("CARGO_FEATURE_WC").is_ok();

    let mut builder = bindgen::Builder::default()
        .header(svn_path.join("svn_dirent_uri.h").to_str().unwrap())
        .header(svn_path.join("svn_dso.h").to_str().unwrap())
        .header(svn_path.join("svn_path.h").to_str().unwrap())
        .header(svn_path.join("svn_version.h").to_str().unwrap())
        .header(svn_path.join("svn_error.h").to_str().unwrap())
        .header(svn_path.join("svn_error_codes.h").to_str().unwrap())
        .header(svn_path.join("svn_opt.h").to_str().unwrap())
        .header(svn_path.join("svn_repos.h").to_str().unwrap())
        .header(svn_path.join("svn_time.h").to_str().unwrap())
        .header(svn_path.join("svn_types.h").to_str().unwrap())
        .header(svn_path.join("svn_types_impl.h").to_str().unwrap())
        .header(svn_path.join("svn_props.h").to_str().unwrap())
        .header(svn_path.join("svn_fs.h").to_str().unwrap())
        .header(svn_path.join("svn_auth.h").to_str().unwrap())
        .header(svn_path.join("svn_config.h").to_str().unwrap())
        .header(svn_path.join("svn_mergeinfo.h").to_str().unwrap())
        .header(svn_path.join("svn_io.h").to_str().unwrap())
        .header(svn_path.join("svn_hash.h").to_str().unwrap())
        .header(svn_path.join("svn_iter.h").to_str().unwrap())
        .header(svn_path.join("svn_subst.h").to_str().unwrap())
        .header(svn_path.join("svn_utf.h").to_str().unwrap())
        .header(svn_path.join("svn_diff.h").to_str().unwrap())
        .header(svn_path.join("svn_cmdline.h").to_str().unwrap())
        .header(svn_path.join("svn_nls.h").to_str().unwrap())
        .header(svn_path.join("svn_x509.h").to_str().unwrap())
        .header(svn_path.join("svn_base64.h").to_str().unwrap())
        .header(svn_path.join("svn_cache_config.h").to_str().unwrap())
        .allowlist_file(r".*[/\\]svn_.*.h")
        .blocklist_type("apr_.*")
        .derive_default(true)
        .raw_line("#[allow(unused_imports)]")
        .raw_line("use apr_sys::{apr_file_t, apr_finfo_t, apr_getopt_t, apr_int64_t, apr_off_t, apr_pool_t, apr_size_t, apr_ssize_t, apr_status_t, apr_time_t, apr_int32_t, apr_uint32_t, apr_fileperms_t, apr_proc_t, apr_uint64_t, apr_dir_t, apr_getopt_option_t, apr_exit_why_e, apr_seek_where_t, apr_byte_t, apr_dso_handle_t};")
        .raw_line("#[allow(unused_imports)]")
        .raw_line("use apr::hash::apr_hash_t;")
        .raw_line("#[allow(unused_imports)]")
        .raw_line("use apr::tables::apr_array_header_t;")
        .clang_args(
            include_paths
                .iter()
                .map(|path| format!("-I{}", path.display())),
        );

    if client_feature_enabled {
        builder = builder.header(svn_path.join("svn_client.h").to_str().unwrap());
    }

    if wc_feature_enabled {
        builder = builder.header(svn_path.join("svn_wc.h").to_str().unwrap());
    }

    // Include svn_ra.h if ra feature OR client feature is enabled
    // (client library depends on ra)
    if ra_feature_enabled || client_feature_enabled {
        builder = builder.header(svn_path.join("svn_ra.h").to_str().unwrap());
    }

    if delta_feature_enabled {
        builder = builder.header(svn_path.join("svn_delta.h").to_str().unwrap());
    }

    // Generate bindings using bindgen
    let svn_bindings = builder.generate().expect("Failed to generate bindings");

    svn_bindings
        .write_to_file(out_path.join("bindings.rs"))
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
