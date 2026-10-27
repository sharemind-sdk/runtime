# Build and use emulator with Docker

This user guide describes how to build and use the emulator with Docker.
We assume the commands are being executed from the same directory as this guide.

## Build

Build the image of the emulator:
```sh
docker build -t sharemind.example/sharemind-mpc/runtime/emulator:latest -f emulator.dockerfile ../..
```

Optionally, specify build-args, defaults are:
1. `--build-arg SHAREMIND_REPOSITORIES_ROOT=https://github.com/sharemind-sdk`;
2. `--build-arg BUILD_REPOSITORY=build-sdk.git`;
3. `--build-arg BUILD_SDK_BRANCH=master`;
4. `--build-arg BUILD_CONFIG_TYPE=public`.

## Usage

```sh
# Run the demo
docker run -it --rm sharemind.example/sharemind-mpc/runtime/emulator:latest --bytecode=secrec/hellosecrec.sb
```

Optionally, use your own mounted bytecode and data:

```sh
BYTECODE_PATH="$(pwd)/../../secrec/test_emu.sb"

DATA_PATH="$(pwd)/data"
mkdir -p "${DATA_PATH}"
perl -e 'print pack("Q< Q< Q<", 2, 4, 8)'  > "${DATA_PATH}/in1.dat"
perl -e 'print pack("Q< Q< Q<", 3, 7, 10)' > "${DATA_PATH}/in2.dat"

docker run -it --rm \
  -v "${BYTECODE_PATH}:/tmp/bytecode:ro" \
  -v "${DATA_PATH}:/data" \
  "sharemind.example/sharemind-mpc/runtime/emulator:latest" \
  --bytecode="/tmp/bytecode" \
   --arg="in1:/data/in1.dat" \
   --arg="in2:/data/in2.dat" \
   --result="out1:/data/out1.dat"
```

## TableDB test

The image is built with the `tabledb` feature and contains
`secrec/tabledb_simple.sb`, compiled from
[tabledb_simple.sc](../../secrec/tabledb_simple.sc).

Unlike the runtime, which forwards the tabledb calls over RPC to an external
database backend, the emulator uses an in-memory mock database with one data
source, `DS1`. So no database is needed, and the mock starts empty on every run.

The example creates a table with 10 columns, inserts 5 rows (row `i` holds the
value `i` in every column) and reads the columns back. Run it and check the
table size:

```sh
docker run --rm sharemind.example/sharemind-mpc/runtime/emulator:latest \
  --bytecode=secrec/tabledb_simple.sb 2>&1 | tee tabledb.log
grep -q 'Columns: 10' tabledb.log && grep -q 'Rows: 5' tabledb.log \
  && grep -q 'Execution successful' tabledb.log \
  && echo "OK: tabledb table created and read" || echo "FAIL: see tabledb.log"
```

Every column read back should print the values `{0, 1, 2, 3, 4}`.

## KeyDB test

The image is built with the `keydb` feature and contains
`secrec/keydb_example.sb`, compiled from
[keydb_example.sc](../../secrec/keydb_example.sc). The image's
`/etc/sharemind/keydb.conf` points at the Redis host `redis`.

Run the example against a temporary Redis and check that every value read back
matches the value written:

```sh
docker compose -f keydb.compose.yaml run --rm emulator 2>&1 | tee keydb.log
docker compose -f keydb.compose.yaml down
grep -q 'Get my_pd_array' keydb.log \
  && diff <(grep -o 'Set my_.*' keydb.log | cut -c5-) <(grep -o 'Get my_.*' keydb.log | cut -c5-) \
  && echo "OK: keydb values match" || echo "FAIL: see keydb.log"
```
