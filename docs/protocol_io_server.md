# Protocol IO server binary

## Build

```sh
# Set the environment variable `SHAREMIND_PREFIX_PATH` to your Sharemind MPC installation path.
export SHAREMIND_PREFIX_PATH=/path/to/sharemind-mpc/prefix/

cargo build --bin protocol_io_server --release
```

This binary is only for data uploading and downloading.
Cargo features are not applicable.

## Usage

### Set up the environment

```sh
# Set the environment variable `SHAREMIND_PREFIX_PATH` to your Sharemind MPC installation path.
export SHAREMIND_PREFIX_PATH=/path/to/sharemind-mpc/prefix/

# Set paths for loading shared libraries, if necessary.
export LD_LIBRARY_PATH="${SHAREMIND_PREFIX_PATH}/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

# Set the executable. Invoke it via Cargo.
SERVER_BIN="cargo run --bin protocol_io_server --"
# But it can also be an absolute path.
# SERVER_BIN="runtime.git/target/release/protocol_io_server"

# The protocol IO servers. 
NODE1="127.0.0.1:30001"
NODE2="127.0.0.1:30002"
NODE3="127.0.0.1:30003"

# Print help.
${SERVER_BIN} --help
```

### Data upload

In protocol mode the PDK implements an online protocol that secret-shares and
securely uploads the data from client to server nodes as part of the same
protocol. The server side of the upload protocol can be invoked using the
runtime server.

The following example invokes the data upload protocol on three MPC nodes using
the `shared3a` protection domain kind. Each node connects to its left peer
forming a circle. The nodes listen to a client for data upload in protocol mode,
and store the received data to the path provided by the `--data-path` argument.

```sh
# Node 1
${SERVER_BIN} --id 1 --mpc-right-listen ${NODE1} --mpc-left-connect ${NODE3} --pdk shared3a mpc-upload --mpc-client-listen 127.0.0.1:40001 --data-path /path/to/node1/A1.bin

# Node 2
${SERVER_BIN} --id 2 --mpc-right-listen ${NODE2} --mpc-left-connect ${NODE1} --pdk shared3a mpc-upload --mpc-client-listen 127.0.0.1:40002 --data-path /path/to/node2/A1.bin

# Node 3
${SERVER_BIN} --id 3 --mpc-right-listen ${NODE3} --mpc-left-connect ${NODE2} --pdk shared3a mpc-upload --mpc-client-listen 127.0.0.1:40003 --data-path /path/to/node3/A1.bin
```

### Data download

Data download from server nodes to a client operates similarly to data upload.

To invoke the server-side data download protocol use the command line argument
`mpc-download` and point the `--data-path` argument to the secret-shared data to
be sent to the client.
