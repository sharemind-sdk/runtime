/*
 * Copyright (C) 2026 Cybernetica
 *
 * Research/Commercial License Usage
 * Licensees holding a valid Research License or Commercial License
 * for the Software may use this file according to the written
 * agreement between you and Cybernetica.
 *
 * GNU General Public License Usage
 * Alternatively, this file may be used under the terms of the GNU
 * General Public License version 3.0 as published by the Free Software
 * Foundation and appearing in the file LICENSE included in the
 * packaging of this file.  Please review the following information to
 * ensure the GNU General Public License version 3.0 requirements will be
 * met: http://www.gnu.org/copyleft/gpl-3.0.html.
 *
 * For further information, please contact us at sharemind@cyber.ee.
 */

use std::env;
use std::path::PathBuf;

fn main() {
    // Use vergen-gitcl to emit git commit and dirty status
    let gitcl = vergen_gitcl::GitclBuilder::default()
        .describe(true, true, None)
        .build()
        .unwrap();
    let build = vergen_gitcl::BuildBuilder::all_build().unwrap();
    vergen_gitcl::Emitter::default()
        .add_instructions(&gitcl)
        .unwrap()
        .add_instructions(&build)
        .unwrap()
        .emit()
        .expect("Unable to emit vergen rustc-env variables");

    let sm_prefix_path = std::env::var("SHAREMIND_PREFIX_PATH").unwrap_or(String::from("/usr"));

    // Make sharemind env variable available via `cargo run`
    println!("cargo:rustc-env=SHAREMIND_PREFIX_PATH={}", sm_prefix_path);

    println!("cargo:rustc-link-search={}/lib", sm_prefix_path);
    // Build supplementary API in ./cpp/ using CMake
    let mut cmake_conf = cmake::Config::new("cpp");
    cmake_conf.define("CMAKE_PREFIX_PATH", &sm_prefix_path);
    #[cfg(feature = "profiler")]
    cmake_conf.define("ENABLE_PROFILER", "ON");
    #[cfg(feature = "tabledb")]
    cmake_conf.define("ENABLE_TABLEDB", "ON");

    let cmake_dst = cmake_conf.build();
    // Link it statically
    println!("cargo:rustc-link-search=native={}", cmake_dst.display());
    println!("cargo:rustc-link-lib=static=supplementary");
    println!("cargo:rustc-link-lib=stdc++");

    // Link sharemind deps dynamically
    println!("cargo:rustc-link-lib=LogHard");
    println!("cargo:rustc-link-lib=sharemind_random");
    println!("cargo:rustc-link-lib=sharemind_datastoremanager");
    println!("cargo:rustc-link-lib=sharemind_executable");
    #[cfg(feature = "profiler")]
    println!("cargo:rustc-link-lib=sharemind_executionprofiler");
    println!("cargo:rustc-link-lib=sharemind_modapi");
    #[cfg(feature = "tabledb")]
    {
        println!("cargo:rustc-link-lib=sharemind_mod_tabledb");
        println!("cargo:rustc-link-lib=sharemind_configuration");
    }
    println!("cargo:rustc-link-lib=sharemind_softfloat");
    println!("cargo:rustc-link-lib=sharemind_vm");
    println!("cargo:rustc-link-lib=sharemind_vmi");

    #[cfg(feature = "ctrl")]
    println!("cargo:rustc-link-lib=sharemind_cmodapi");

    // Change detection
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=cpp");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Generate bindings.rs
    #[allow(unused_mut)]
    let mut bindgen_builder = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        .clang_arg(format!("-I{}/include/", sm_prefix_path))
        .clang_arg("-Icpp")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    #[cfg(feature = "profiler")]
    {
        bindgen_builder = bindgen_builder.clang_arg("-DENABLE_PROFILER");
    }
    #[cfg(feature = "tabledb")]
    {
        bindgen_builder = bindgen_builder.clang_arg("-DENABLE_TABLEDB");
    }

    bindgen_builder
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings")
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    #[cfg(feature = "ctrl")]
    bindgen::Builder::default()
        .header("wrapper_ctrl.h")
        .clang_arg(format!("-I{}/include/", sm_prefix_path))
        .clang_arg("-Icpp")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(out_path.join("bindings_ctrl.rs"))
        .expect("Couldn't write Ctrl bindings!");

    #[cfg(feature = "tabledb")]
    {
        // Compile our Cap'n Proto schema
        capnpc::CompilerCommand::new()
            .src_prefix("schema")
            .file("schema/tabledb.capnp")
            .run()
            .expect("schema compiler command");
    }
}
