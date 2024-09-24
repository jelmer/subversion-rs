use subversion::client::CatOptions;

fn main() {
    let mut ctx = subversion::client::Context::new().unwrap();

    let mut stdout = std::io::stdout();

    ctx.cat(
        "http://svn.apache.org/repos/asf/subversion/trunk/subversion/libsvn_client/cat.c",
        &mut stdout,
        &CatOptions::default()
    )
    .unwrap();
}
