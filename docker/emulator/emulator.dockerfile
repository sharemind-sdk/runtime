# syntax=docker/dockerfile:1
#
# Copyright (C) 2026 Cybernetica
#
# Research/Commercial License Usage
# Licensees holding a valid Research License or Commercial License
# for the Software may use this file according to the written
# agreement between you and Cybernetica.
#
# GNU General Public License Usage
# Alternatively, this file may be used under the terms of the GNU
# General Public License version 3.0 as published by the Free Software
# Foundation and appearing in the file LICENSE included in the
# packaging of this file.  Please review the following information to
# ensure the GNU General Public License version 3.0 requirements will be
# met: http://www.gnu.org/copyleft/gpl-3.0.html.
#
# For further information, please contact us at sharemind@cyber.ee.
#

# -- Stage 0: Prepare base layer (system dependencies) --
FROM rust:1.91-slim-trixie AS chef

# Install tools and dependencies
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    capnproto \
    git \
    cmake \
    build-essential \
    clang \
    libboost-system-dev \
    libboost-thread-dev \
    libboost-program-options-dev \
    libboost-test-dev \
    libboost-filesystem-dev \
    m4 \
    gnutls-dev \
    libcrypto++8 \
    libcrypto++-dev \
    libcrypto++-doc \
    libcrypto++-utils \
    libeigen3-dev \
    pkg-config \
    libssl-dev \
    llvm \
    libhiredis-dev \
    libhdf5-dev \
    libboost-dev \
    libboost-filesystem-dev \
    libboost-iostreams-dev \
    libboost-program-options-dev \
    libboost-system-dev \
    bison \
    flex \
    libmpfr-dev \
    doxygen
WORKDIR /app
RUN cargo install cargo-chef

# -- Stage 1: Prepare Recipe (dependencies only) --
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json --bin emulator

# -- Stage 2: Build Dependencies --
FROM chef AS builder

ARG SHAREMIND_REPOSITORIES_ROOT="https://github.com/sharemind-sdk"
ARG BUILD_REPOSITORY="build-sdk.git"
ARG BUILD_SDK_BRANCH="master"
ARG BUILD_CONFIG_TYPE="public"

WORKDIR /
RUN git clone "${SHAREMIND_REPOSITORIES_ROOT}/${BUILD_REPOSITORY}" build
COPY --from=planner /app/docker/build.config.local/${BUILD_CONFIG_TYPE} /build/config.local
RUN cd build \
    && git checkout ${BUILD_SDK_BRANCH} \
    && mkdir b \
    && cd b \
    && cmake -DSHAREMIND_REPOSITORIES_ROOT_OVERRIDE=${SHAREMIND_REPOSITORIES_ROOT} -DREPO_ROOT=${SHAREMIND_REPOSITORIES_ROOT} .. \
    && make
RUN ldconfig

WORKDIR /emu_cfg
RUN cp $(find /build/ -type f -wholename *packaging/configs/sharemind/shared3p_emu.conf | head -n 1) ./shared3p_emu.conf \
    && cp $(find /build/ -type f -wholename *packaging/configs/sharemind/shared3p_emu-models.conf | head -n 1) ./shared3p_emu-models.conf

WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this layer is cached until Cargo.toml changes
RUN cargo chef cook --release --recipe-path recipe.json --bin emulator --features profiler --features tabledb --features keydb

# -- Stage 3: Build Runtime (using cached dependencies) --
COPY . .
RUN SHAREMIND_PREFIX_PATH=/build/prefix cargo build --release --bin emulator -F profiler -F tabledb -F keydb


# -- Stage 4: Install built artefacts --
FROM debian:trixie-slim

# Install dependencies
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    libboost-system1.83-dev \
    libboost-thread1.83-dev \
    libboost-program-options1.83-dev \
    libboost-test1.83-dev \
    libboost-filesystem1.83-dev \
    libcrypto++8 \
    libhiredis1.1.0 \
    netcat-openbsd

# Install the runtime
COPY --from=builder /app/target/release/emulator /usr/local/bin/
COPY --from=builder /build/prefix/ /usr/
COPY --from=builder /app/secrec /secrec/
COPY --from=builder /emu_cfg/ /app/tests/tabledb_tests/config/ /app/docker/emulator/keydb.conf /etc/sharemind/

ENTRYPOINT [ "emulator", \
             "--pd-module=libsharemind_mod_shared3p_emu.so", \
             "--pd-conf=shared3p_emu.conf" \
]
