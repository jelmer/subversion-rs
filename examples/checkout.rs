use subversion::{Depth, Revision};

fn main() {
    let mut pool = apr::Pool::default();
    let ctx = subversion::client::Context::new(&mut pool);

    ctx.checkout(
        "http://svn.apache.org/repos/asf/subversion/trunk/subversion/libsvn_client",
        std::path::Path::new("libsvn_client"),
        Revision::Head,
        Revision::Head,
        Depth::Infinity,
        false,
        false,
    )
    .unwrap();
}
