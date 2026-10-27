# Emulator binary

## Build

```shell
# Set the environment variable `SHAREMIND_PREFIX_PATH` to your Sharemind MPC installation path.
export SHAREMIND_PREFIX_PATH=/path/to/sharemind-mpc/prefix/

# Build the binary with no additional features enabled.
cargo build --release --bin emulator
# Build the binary with profiler, tabledb and keydb features enabled.
cargo build --release --bin emulator -F profiler -F tabledb -F keydb
```

### Cargo features

Optional features can be enabled with `-F <feature>`.

| Feature    | Description                                                                                          | Options                                                                                                                                                                   | Sharemind repositories                                                                                                                                         |
|------------|------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `profiler` | Records the durations of code sections marked in SecreC programs.                                    | `--profiler-out`                                                                                                                                                          | [mod_executionprofiler](https://github.com/sharemind-sdk/mod_executionprofiler), [libexecutionprofiler](https://github.com/sharemind-sdk/libexecutionprofiler) |
| `tabledb`  | Table database for SecreC programs, backed by an in-memory mock database with the data source `DS1`. | `--tabledb-conf` ([tabledb.conf](../tests/tabledb_tests/config/tabledb.conf))                                                                                             | [mod_tabledb](https://github.com/sharemind-sdk/mod_tabledb)                                                                                                    |
| `keydb`    | Redis key-value store for SecreC programs.                                                           | `--keydb-conf` ([keydb.conf](https://github.com/sharemind-sdk/mod_keydb/blob/master/packaging/configs/sharemind/keydb.conf)) (must set `[AccessControl] Enabled = false`) | [mod_keydb](https://github.com/sharemind-sdk/mod_keydb)                                                                                                        |

## Usage

### Set up the environment

```sh
# Set runtime.git path.
export RUNTIME_REPO_PATH=/path/to/this/repository
cd ${RUNTIME_REPO_PATH}

# Set the environment variable `SHAREMIND_PREFIX_PATH` to your Sharemind MPC installation path.
export SHAREMIND_PREFIX_PATH=/path/to/sharemind-mpc/prefix/

# Set paths for loading shared libraries, if necessary.
export LD_LIBRARY_PATH="${SHAREMIND_PREFIX_PATH}/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

# Set the executable. Invoke it via Cargo.
EMULATOR_BIN="cargo run --bin emulator -F profiler -F tabledb --"
# But it can also be an absolute path.
# EMULATOR_BIN="target/release/emulator"

# Print help.
${EMULATOR_BIN} --help
```

### Prepare configs and data

```sh
# Configs
SHARED3P_EMU_CFG_PATH=$(find ${SHAREMIND_PREFIX_PATH}/.. -type f -wholename *packaging/configs/sharemind/shared3p_emu.conf | head -n 1)
TABLEDB_CFG_PATH="tests/tabledb_tests/config/tabledb.conf"

# Data
mkdir data
perl -e 'print pack("Q< Q< Q<", 2, 4, 8)' > data/in1.dat
perl -e 'print pack("Q< Q< Q<", 3, 7, 10)' > data/in2.dat
```

### Run emulator

```sh
# Execute the SecreC code.
${EMULATOR_BIN} \
  --pd-conf ${SHARED3P_EMU_CFG_PATH} \
  --tabledb-conf ${TABLEDB_CFG_PATH} \
  --bytecode secrec/test_emu.sb \
  --arg in1:data/in1.dat \
  --arg in2:data/in2.dat \
  --result out1:data/out1.dat

# Validate the output.
if cmp -s data/out1.dat <(perl -e 'print pack("Q< Q< Q<", 6, 28, 80)'); then
   echo -e "\n✅ OK: Received output matches expected output."
else
   echo -e "\n❌ FAIL: Received output does not match expected output."
fi
```
