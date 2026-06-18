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

Password store auth providers
-----------------------------

Bindings for the platform-specific password store auth providers are behind
optional features, since each needs an extra Subversion auth library at build
and runtime:

- `gnome-keyring` links `libsvn_auth_gnome_keyring` (needs `libsecret`).
- `kwallet` links `libsvn_auth_kwallet` (needs KWallet / Qt5 / KF5).
- `gpg-agent` lives in `libsvn_subr`, so it needs no extra system library.
