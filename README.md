Subversion bindings for Rust
============================

This rust crate provides idiomatic bindings for the Subversion C libraries.

At the moment, it only covers the "client" library but the aim is to support
all of the public C API.

Example:

```rust
use subversion::client::CheckoutOptions;

let mut ctx = subversion::client::Context::new().unwrap();

ctx.checkout(
    "http://svn.apache.org/repos/asf/subversion/trunk/subversion/libsvn_client",
    std::path::Path::new("libsvn_client"),
    CheckoutOptions {
        peg_revision: Revision::Head,
        revision: Revision::Head,
        depth: Depth::Infinity,
        ..default::Default()
    }
)
.unwrap();
```
