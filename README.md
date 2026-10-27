# Sharemind MPC Runtime

This project builds a low footprint wrapper around Sharemind MPC components.
It facilitates the integration of MPC-based data analysis with software systems and platforms.

## Features

- **Flexibility:** enables a wide range of MPC integration patterns, adaptable to different system architectures.
- **Rapid prototyping:** supports MPC emulation, experimental lab deployments, performance evaluation.
- **Battle-tested MPC:** harness the machine optimized protocols and the SecreC programming language developed and perfected for the Sharemind MPC platform.

## Quickstart

The simplest way to try out the Runtime is by using it to execute SecreC bytecode on the [shared3p-emu](https://github.com/sharemind-sdk/mod_shared3p_emu) protocol set.
This configuration is referred to as the *emulator*, as it involves a single Runtime instance and no distributed MPC protocol.

```
docker build -t sharemind.example/sharemind-mpc/runtime/emulator:latest -f ./docker/emulator/emulator.dockerfile .
docker run -it --rm sharemind.example/sharemind-mpc/runtime/emulator:latest --bytecode=secrec/hellosecrec.sb
```

## Repository contents

| Path              | Description                                 |
|-------------------|---------------------------------------------|
| [cpp/](cpp)       | C++ source.                                 |
| [docker/](docker) | Dockerfiles and Docker usage guides.        |
| [docs/](docs)     | Documentation for the [binaries](src/bin/.) |
| [schema/](schema) | TableDB Cap'n Proto schema.                 |
| [secrec/](secrec) | SecreC for demo and testing.                |
| [src/](src)       | Runtime source.                             |
| [tests/](tests)   | Fixtures and tests.                         |

## License

Sharemind MPC Runtime is available under two licenses:

- **GNU General Public License version 3.0**. See [LICENSE](LICENSE).
- **Research or Commercial License** from Cybernetica, for use under the terms
  of a written agreement with Cybernetica.

For research or commercial licensing, contact us at <sharemind@cyber.ee>.
