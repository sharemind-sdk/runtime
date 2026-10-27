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

#![doc = include_str!("../../docs/secret_share_converter.md")]
use anyhow::Context;
use clap::Parser;
use sharemind_runtime::ffi::ctrl::cmodapi::pd::Value;
use sharemind_runtime::helper::client;
use sharemind_runtime::helper::client::Command;
use sharemind_runtime::helper::client::ProtectionDomainService;
use std::fs::File;
use std::path::PathBuf;
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
};

#[derive(Clone, Parser, Debug)]
#[command(author, version = sharemind_runtime::version::VERSION, about, long_about = None)]
struct Args {
    /// Path to the loadable PD module file
    ///
    /// Default: libsharemind_mod_{PDK}_ctrl.so
    #[arg(long = "pd-module")]
    pd_module_path: Option<PathBuf>,

    /// Path to the PD configuration file.
    #[arg(long = "pd-conf")]
    pd_conf_path: Option<PathBuf>,

    /// Name of the Protection Domain Kind to instantiate from the module as PD.
    #[arg(long = "pdk")]
    pdk_name: String,

    /// Path to the plaintext data file (input or output).
    #[arg(long)]
    plaintext_path: PathBuf,

    /// Paths to the protected data files (input or output).
    #[arg(long = "protected-path")]
    protected_paths: Vec<PathBuf>,

    /// Treat output paths as pipes (default: treat as files).
    ///
    /// file - truncate if exists, create if missing.
    /// pipe - append if path exists, error if missing.
    #[arg(long, default_value_t = false, verbatim_doc_comment)]
    pipe: bool,

    /// Name of the PD data type associated with protected data.
    #[arg(long)]
    pd_typename: String,

    /// Specify the operation to execute.
    #[command(subcommand)]
    command: Command,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args = Args::parse();
    client::setup_logging()?;
    let Args {
        pd_module_path,
        pdk_name,
        pd_conf_path,
        plaintext_path,
        pipe,
        command,
        ..
    } = cli_args.clone();
    let (pd_name, pd) =
        client::create_protection_domain(pd_module_path, pdk_name, pd_conf_path, None)?;

    // Execute the selected operation
    match command {
        Command::Classify => {
            tracing::info!(plaintext = ?plaintext_path, "Classify");
            // Read the input value data
            let plaintext_data = client::read_input_value_data(plaintext_path)?;
            classify(&cli_args, &pd_name, &pd, plaintext_data)?;
        }

        Command::Declassify => {
            let mut plaintext_writer = client::prepare_output_data_writer(pipe, plaintext_path)?;
            declassify(cli_args, pd_name, pd, &mut plaintext_writer)?;
        }
    }
    tracing::info!("Success");

    Ok(())
}

fn declassify(
    cli_args: Args,
    pd_name: String,
    pd: ProtectionDomainService,
    plaintext_writer: &mut File,
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate CLI args
    if cli_args.protected_paths.len() != pd.node_count() {
        return Err("Number of protected data paths must match the PD node count".into());
    }

    // Load the protected data into value sets
    let value_name = "value".to_owned();
    let protected_value_sets = cli_args
        .protected_paths
        .iter()
        .map(|p| -> anyhow::Result<HashMap<String, Value>> {
            let data =
                fs::read(p).with_context(|| format!("Failed to read input: {}", p.display()))?;

            let value = Value {
                pd_name: pd_name.clone(),
                type_name: cli_args.pd_typename.clone(),
                data,
            };

            let value_set = HashMap::from([(value_name.clone(), value)]);
            Ok(value_set)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    // In the local mode the "declassify" operation returns the
    // reconstructed plaintext values.
    tracing::debug!("Starting declassify...");
    let plaintext_values = pd.declassify(&protected_value_sets)?;
    assert_eq!(plaintext_values.len(), 1);
    let value = plaintext_values
        .get(&value_name)
        .ok_or("Expected value not found in the output")?;

    tracing::info!("Writing output...");
    // TODO: What serialization should we use?
    //  For now write data without type information, just like input was.
    plaintext_writer.write_all(&value.data).with_context(|| {
        format!(
            "Failed to write output: {}",
            cli_args.plaintext_path.display()
        )
    })?;

    Ok(())
}

fn classify(
    cli_args: &Args,
    pd_name: &String,
    pd: &ProtectionDomainService,
    plaintext_data: Vec<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate CLI args
    if cli_args.protected_paths.len() != pd.node_count() {
        return Err("Number of protected data paths must match the PD node count".into());
    }

    // Currently we process only one value.
    let value = Value {
        pd_name: pd_name.clone(),
        type_name: cli_args.pd_typename.clone(),
        data: plaintext_data,
    };

    // Value name is irrelevant, unless we want to process more than
    // one value or send the value directly to the servers afterward.
    let value_name = "value".to_owned();
    let plaintext_values = HashMap::from([(value_name.clone(), value)]);

    // In the local mode the "classify" operation returns the
    // protected value sets (one per MPC node) in the protection
    // domain representation. Each set must contain one protected
    // value component corresponding to the original input value.
    tracing::debug!("Starting classify...");
    let protected_value_sets = pd.classify(&plaintext_values)?;
    assert_eq!(protected_value_sets.len(), pd.node_count());
    assert!(protected_value_sets.iter().all(|s| s.len() == 1));

    // Store the protected values
    // TODO: Or do we send protected values directly to the servers?
    tracing::info!("Writing outputs...");
    for (value_set, output_path) in protected_value_sets.iter().zip(&cli_args.protected_paths) {
        let value = value_set
            .get(&value_name)
            .ok_or("Expected value not found in the results of the 'classify' operation")?;

        // TODO: How we communicate pd_typename to the server?

        // If path is a file: truncate if exists, create if missing.
        // If path is a pipe: append if exists, error if missing.
        OpenOptions::new()
            .write(true)
            .create(!cli_args.pipe)
            .truncate(!cli_args.pipe)
            .open(output_path)
            .with_context(|| format!("Failed to open output path: {}", output_path.display()))?
            .write_all(&value.data)
            .with_context(|| format!("Failed to write output: {}", output_path.display()))?;
    }
    Ok(())
}
