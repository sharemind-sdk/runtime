# `sharemind-runtime` server binary

## Introduction

This user guide briefly describes the Sharemind MPC Runtime server and how to
use it on command line.

## Functionality

Runtime server executes bytecode using the built-in bytecode virtual machine.
The VM differentiates between the public and protected data domains (protection
domains) and operates with the protected data using plug-in protection domain
kind implementations (Multi-Party Computation protocols packaged as loadable
modules with specific API).

MPC protocols involve multiple nodes (parties) communicating over the network to
carry out data operations. One runtime server represents a single MPC node.
Multiple runtime servers together form a single MPC deployment that can
collaboratively execute MPC protocols as specified in a bytecode program.

During bytecode execution each runtime server node in a single MPC deployment
executes an identical bytecode program. Each bytecode execution consumes some
inputs and produces some outputs as specified in the bytecode.

The runtime is responsible for executing MPC protocols. It does not have any kind 
of session management and operates via one-shot activations (one execution per 
invocation). As such, it requires an external orchestration layer to manage user 
sessions and operation workflows.

This binary does not handle uploading or downloading data. That is done by
separate binaries. For example, see:

* [secret_share_converter](secret_share_converter.md): the client binary for `shared3p` 
  secret sharing and recombination.
* [protocol_io_client](protocol_io_client.md): the client binary for the `shared3a`
  upload and download flow.
* [protocol_io_server](protocol_io_server.md): the server binary for the `shared3a` 
  upload and download flow.

## Build

```sh
# Set the environment variable `SHAREMIND_PREFIX_PATH` to your Sharemind MPC installation path.
export SHAREMIND_PREFIX_PATH=/path/to/sharemind-mpc/prefix/

# Build the binary with no additional features enabled.
cargo build --bin sharemind-runtime --release
# Build the binary with profiler, tabledb and keydb features enabled.
cargo build --bin sharemind-runtime --release -F profiler -F tabledb -F keydb
```

### Cargo features

Optional features can be enabled with `-F <feature>`.

| Feature    | Description                                                                                                                    | Options                                                                                                                                                                   | Sharemind repositories                                                                                                                                         |
|------------|--------------------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `profiler` | Records the durations of code sections marked in SecreC programs.                                                              | `--profiler-out`                                                                                                                                                          | [mod_executionprofiler](https://github.com/sharemind-sdk/mod_executionprofiler), [libexecutionprofiler](https://github.com/sharemind-sdk/libexecutionprofiler) |
| `tabledb`  | Table database for SecreC programs. The database calls are forwarded over RPC (`/tmp/tabledb_rpc.sock`) to a database backend. | `--tabledb-conf` ([tabledb.conf](../tests/tabledb_tests/config/tabledb.conf))                                                                                             | [mod_tabledb](https://github.com/sharemind-sdk/mod_tabledb)                                                                                                    |
| `keydb`    | Redis key-value store for SecreC programs.                                                                                     | `--keydb-conf` ([keydb.conf](https://github.com/sharemind-sdk/mod_keydb/blob/master/packaging/configs/sharemind/keydb.conf)) (must set `[AccessControl] Enabled = false`) | [mod_keydb](https://github.com/sharemind-sdk/mod_keydb)                                                                                                        |

## Usage

### Set up the environment

```sh
# Set your Sharemind MPC installation path.
export SHAREMIND_PREFIX_PATH=/path/to/sharemind-mpc/prefix/

# Set paths for loading shared libraries, if necessary.
export LD_LIBRARY_PATH="${SHAREMIND_PREFIX_PATH}/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

# Set the executable. Invoke it via Cargo.
SERVER_BIN="cargo run --bin sharemind-runtime --"
# But it can also be an absolute path.
# SERVER_BIN="runtime.git/target/release/sharemind-runtime"

# Addresses of MPC nodes
NODE1="127.0.0.1:30001"
NODE2="127.0.0.1:30002"
NODE3="127.0.0.1:30003"

# Print help.
${SERVER_BIN} --help
```

### Bytecode execution

Runtime server executes the bytecode given by the `--bytecode PATH` command
line argument. Optionally, a number of inputs and outputs may be provided. The
runtime server reads inputs from paths provided by the `--arg=KEY:PATH`
arguments, and writes outputs to paths provided by the `--result=KEY:PATH`
arguments.

The following example executes a bytecode program on three runtime server nodes
with one input argument `A1` and output result `R1` using the `shared3a`
protection domain kind.

```sh
BYTECODE="/path/to/bytecode.sb"

# Node 1
${SERVER_BIN} --id 1 --mpc-right-listen ${NODE1} --mpc-left-connect ${NODE3} --pdk shared3a --bytecode ${BYTECODE} --arg A1:/path/to/node1/A1.bin --result R1:/path/to/node1/R1.bin

# Node 2
${SERVER_BIN} --id 2 --mpc-right-listen ${NODE2} --mpc-left-connect ${NODE1} --pdk shared3a --bytecode ${BYTECODE} --arg A1:/path/to/node2/A1.bin --result R1:/path/to/node2/R1.bin

# Node 3
${SERVER_BIN} --id 3 --mpc-right-listen ${NODE3} --mpc-left-connect ${NODE2} --pdk shared3a --bytecode ${BYTECODE} --arg A1:/path/to/node3/A1.bin --result R1:/path/to/node3/R1.bin
```

