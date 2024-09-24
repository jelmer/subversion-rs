use subversion::client::CheckoutOptions;

fn main() {
    let mut ctx = subversion::client::Context::new().unwrap();

    ctx.checkout(
        "http://svn.apache.org/repos/asf/subversion/trunk/subversion/libsvn_client",
        std::path::Path::new("libsvn_client"),
        &CheckoutOptions::default()
    )
    .unwrap();
}
