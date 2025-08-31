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
        .allowlist_file(".*/svn_.*.h")
        .blocklist_type("apr_.*")
        .derive_default(true)
        .raw_line("use apr_sys::apr_file_t;")
        .raw_line("use apr_sys::apr_finfo_t;")
        .raw_line("use apr_sys::apr_getopt_t;")
        .raw_line("use apr_sys::apr_int64_t;")
        .raw_line("use apr_sys::apr_off_t;")
        .raw_line("use apr_sys::apr_pool_t;")
        .raw_line("use apr_sys::apr_size_t;")
        .raw_line("use apr_sys::apr_ssize_t;")
        .raw_line("use apr_sys::apr_status_t;")
        .raw_line("use apr_sys::apr_time_t;")
        .raw_line("use apr_sys::apr_int32_t;")
        .raw_line("use apr_sys::apr_uint32_t;")
        .raw_line("use apr_sys::apr_fileperms_t;")
        .raw_line("use apr_sys::apr_proc_t;")
        .raw_line("use apr_sys::apr_uint64_t;")
        .raw_line("use apr_sys::apr_dir_t;")
        .raw_line("use apr::hash::apr_hash_t;")
        .raw_line("use apr::tables::apr_array_header_t;")
        .raw_line("use apr_sys::apr_getopt_option_t;")
        .raw_line("use apr_sys::apr_exit_why_e;")
        .raw_line("use apr_sys::apr_seek_where_t;")
        .raw_line("use apr_sys::apr_byte_t;")
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

    if ra_feature_enabled {
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
