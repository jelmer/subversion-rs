# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust crate that provides idiomatic bindings for the Subversion C libraries. It currently covers the client library and is working towards supporting all of the public C API.

## Build System

The project uses `bindgen` to generate Rust bindings from Subversion C headers, and `system-deps` to find system library dependencies. The bindings are generated at build time through `build.rs`.

## Common Commands

### Building
```bash
cargo build
# Build with all features:
cargo build-all-features
```

### Testing
```bash
cargo test
# Test with all features:
cargo test-all-features
# Run a single test:
cargo test test_name
```

### Linting and Formatting
```bash
cargo fmt
cargo fmt -- --check  # Check without modifying
```

### Examples
```bash
cargo run --example checkout --features client
cargo run --example cat --features client
```

## Architecture

### Feature Flags

The crate uses feature flags to control which Subversion modules are included:
- `client` - Client operations
- `ra` - Repository access
- `wc` - Working copy management
- `delta` - Delta operations
- `pyo3` - Python bindings
- `url` - URL parsing utilities

Default features: `["ra", "wc", "client", "delta"]`

### Module Structure

- `src/lib.rs` - Main library entry point and common types
- `src/client.rs` - Client API bindings (checkout, commit, etc.)
- `src/ra.rs` - Repository access layer
- `src/wc.rs` - Working copy management
- `src/generated.rs` - Auto-generated bindings from C headers (created by build.rs)
- `src/error.rs` - Error handling types
- `src/auth.rs` - Authentication utilities

### FFI Bindings

The project generates FFI bindings at build time through `build.rs`, which uses `bindgen` to process Subversion C headers. The generated bindings are placed in `src/generated.rs` but this file is not checked into version control.

### APR Integration

The crate depends on Apache Portable Runtime (APR) and uses the `apr` crate for Rust bindings. Memory management typically involves APR pools.

## Dependencies

System dependencies (require development packages):
- `libsvn_client` (>= 1.14)
- `libsvn_subr` (>= 1.14)
- `libapr` and `libaprutil`
- Additional SVN libraries based on enabled features

On Ubuntu/Debian:
```bash
sudo apt install libapr1-dev libaprutil1-dev libsvn-dev pkg-config
```