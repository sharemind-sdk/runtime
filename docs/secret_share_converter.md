# Secret share converter binary

## Build

```sh
# Set the environment variable `SHAREMIND_PREFIX_PATH` to your Sharemind MPC installation path.
export SHAREMIND_PREFIX_PATH=/path/to/sharemind-mpc/prefix/

# Build the binary.
cargo build --bin secret_share_converter --release -F ctrl
```

The `ctrl` Cargo feature (controller module API) is required to build this binary.
Other features are not applicable.

## Usage

### Set up the environment

```sh
# Set your Sharemind MPC installation path.
export SHAREMIND_PREFIX_PATH=/path/to/sharemind-mpc/prefix/

# Set paths for loading shared libraries, if necessary.
export LD_LIBRARY_PATH="${SHAREMIND_PREFIX_PATH}/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

# Set the executable. Invoke it via Cargo.
CLIENT_BIN="cargo run --bin secret_share_converter -F ctrl --"
# But it can also be an absolute path.
# CLIENT_BIN="runtime.git/target/release/secret_share_converter"
```

### Secret-share and reconstruct data offline


Secret-sharing splits plaintext data into protected shares, one per MPC server.
To secret-share the plaintext data on the local machine, please invoke the
client with the following command:

```sh
# (optionally) Prepare plaintext input data. 
# perl -e 'print pack("Q<", 42)' > /path/to/plaintext.bin

# Use a protection domain module that implements offline secret-sharing.
PD_MODULE_PATH="${SHAREMIND_PREFIX_PATH}/lib/libsharemind_mod_shared3p_ctrl.so"

# Classify.
${CLIENT_BIN} \
    --pd-module ${PD_MODULE_PATH} \
    --pdk shared3p \
    --plaintext-path /path/to/plaintext.bin \
    --protected-path /path/to/shares1.bin \
    --protected-path /path/to/shares2.bin \
    --protected-path /path/to/shares3.bin \
    --pd-typename uint64 \
    classify
```

The `classify` operation reads the plaintext input data from the path specified
by the `--plaintext-path` argument and writes the secret-shared data to the
paths specified by the `--protected-path` arguments.

Share reconstruction is the reverse of secret-sharing. To reconstruct the
secret-shared data, use the `declassify` operation instead of `classify` with
the rest of arguments unchanged. It will read the secret-shared data from the
paths specified by `--protected-path` and write the reconstructed data to the
path specified by `--plaintext-path`.

### Upload and download data

The `shared3p` performs secret-sharing and reconstruction of data offline. 
In this case the user is responsible for transmitting the secret-shared data manually 
between the client and the server hosts using system tools, e.g. `nc`or `socat`.
