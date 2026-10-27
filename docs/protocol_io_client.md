# Protocol IO client binary

## Build

```sh
# Set the environment variable `SHAREMIND_PREFIX_PATH` to your Sharemind MPC installation path.
export SHAREMIND_PREFIX_PATH=/path/to/sharemind-mpc/prefix/

# Build the binary.
cargo build --bin protocol_io_client --release -F ctrl
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
CLIENT_BIN="cargo run --bin protocol_io_client -F ctrl --"
# But it can also be an absolute path.
# CLIENT_BIN="runtime.git/target/release/protocol_io_client"

# The protocol IO servers. 
NODE1="127.0.0.1:30001"
NODE2="127.0.0.1:30002"
NODE3="127.0.0.1:30003"

# Print help.
${CLIENT_BIN} --help
```

### Upload and download data in protocol mode

Some protection domain kind implementations combine the secret-sharing and
upload operations into a single online protocol. Similarly, they combine the
download and reconstruction operations. The runtime client facilitates the PDKs
with data transfer between the client and the servers, and hence, must be
invoked with information about the MPC servers instead of the protected files.

The following examples invoke the client-side data upload and download protocols
implemented by the protection domain client module.

```sh
# Use a protection domain module that implements secret-sharing in protocol mode
PD_MODULE_PATH="${SHAREMIND_PREFIX_PATH}/lib/libsharemind_mod_shared3a_ctrl.so"

# Upload
${CLIENT_BIN} \
    --pd-module ${PD_MODULE_PATH} \
    --pdk shared3a \
    --server addr=${NODE1} \
    --server addr=${NODE2} \
    --server addr=${NODE3} \
    --plaintext-path /path/to/plaintext_in.bin \
    --pd-typename uintp64 \
    classify

# Download
${CLIENT_BIN} \
    --pd-module ${PD_MODULE_PATH} \
    --pdk shared3a \
    --server addr=${NODE1} \
    --server addr=${NODE2} \
    --server addr=${NODE3} \
    --plaintext-path /path/to/plaintext_out.bin \
    --pd-typename uintp64 \
    declassify
```
