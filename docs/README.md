# Docs

## Structure

Each Markdown file here describes how to build and use the respective [binaries](../src/bin/.),
with [sharemind-runtime.md](sharemind-runtime.md) covering [main.rs](../src/main.rs).

## Cargo features

Optional functionality is enabled with Cargo features, e.g.
`cargo build --bin sharemind-runtime -F tabledb -F keydb`.
The Build section of each binary's doc lists the Cargo features it supports.

## Prerequisites

### Operating system

* Ubuntu 26.04 LTS (Resolute Raccoon)
* Ubuntu 24.04 LTS (Noble Numbat)
* Debian 13 (trixie)
* Debian 12 (bookworm)

### Sharemind SDK

Build Sharemind SDK: <https://github.com/sharemind-sdk/build-sdk>.
After this, the dependencies will be installed in the `prefix/` subdirectory under
`/path/to/sharemind-sdk/build-sdk/repository/`.
The absolute path to the `prefix/` subdirectory in other docs is referred to as `SHAREMIND_PREFIX_PATH`.

### System packages

1. Install (default) Rust and Cargo: <https://doc.rust-lang.org/cargo/getting-started/installation.html>

```sh
curl https://sh.rustup.rs -sSf | sh
source ~/.bashrc
```

2. Install the following system packages using the package manager:

```sh
sudo apt install clang build-essential cmake
# Install capnproto (https://capnproto.org/install.html)
sudo apt install capnproto
```
