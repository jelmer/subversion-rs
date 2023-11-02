Subversion bindings for Rust
============================

This rust crate provides idiomatic bindings for the Subversion C libraries.

At the moment, it only covers the "client" library but the aim is to support
all of the public C API.

Example:

```rust

let mut ctx = subversion::client::Context::new().unwrap();

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
```
