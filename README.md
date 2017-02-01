# rustyd

## Rust

To install Rust 1.12 and cargo on FreeBSD:

```shell
pkg install rust cargo
```

To update Rust to the latest version:

```shell
pkg install sudo
curl -ssf https://static.rust-lang.org/rustup.sh | sh
```

## libdtrace bindings

To install clang from ports so that bindgen can find libclang.so:

```shell
pkg install clang38
```

To install snappy

```shell
pkg install snappy
```

Rust bindings for `libdtrace` can be (more or less) automatically generated
using `rust-bindgen`:

```shell
cargo install bindgen

export C_INCLUDE_PATH=/usr/src/sys/cddl/compat/opensolaris:/usr/src/sys/cddl/contrib/opensolaris/uts/common

~/.cargo/bin/bindgen --output libdtrace.rs --builtins /usr/src/cddl/contrib/opensolaris/lib/libdtrace/common/dtrace.h
```

### build.rs

`libdtrace` has dependencies on `libproc`, `librtld_db` and a number of other
libraries. Dependencies can be specified in `build.rs` as follows:

```rust
fn main() {
    println!("cargo:rustc-link-lib=dylib=dtrace");
    println!("cargo:rustc-link-lib=dylib=proc");
    println!("cargo:rustc-link-lib=dylib=rtld_db");
    println!("cargo:rustc-link-lib=dylib=ctf");
    println!("cargo:rustc-link-lib=dylib=elf");
    println!("cargo:rustc-link-lib=dylib=c");
    println!("cargo:rustc-link-lib=dylib=z");
    println!("cargo:rustc-link-lib=dylib=pthread");
    println!("cargo:rustc-link-lib=dylib=util");
}
```

### Manual tweaks and issues

Some of the generated types do not implemented the `Debug` trait. As a
temporary fix remove the `#[derive[Debug]]` attribute. (I haven't investigated
the root cause of this.)

`cargo build` passes the command line argument `-Wl,-as-needed` to the linker,
However when installing FreeBSD from an ISO libstrace doesn't included it's
dependencies on other shraed objects in `NEEDED` sections. When rebuilding
world from source the resulting `libdtrace` does specify its dependencies.

Change `arg` of `dtrace_progam_strcompile` to `* const * const` from
`* mut * mut`.

Remove `* mut` from `dtrace_work` `arg3` and `arg4`.
