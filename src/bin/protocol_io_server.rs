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

use anyhow::Context;
use clap::{Parser, Subcommand};
use sharemind_runtime::{
    ffi::facility::network::NetworkFacility,
    helper::server::{setup_loghard_logging, Features},
    network::zeromq::server::{WorkerContext, ZeromqNetworkFactory},
    SharemindRuntime,
};
use std::fs::{self, OpenOptions};
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    /// Perform protocol layer upload operation
    MpcUpload {
        /// IP address and port to listen on for the MPC client upload communication.
        #[arg(long = "mpc-client-listen")]
        mpc_client_listen_addr: SocketAddr,

        /// Path for storing the data received from the client.
        #[arg(long)]
        data_path: PathBuf,

        /// Treat DATA_PATH as a pipe (default: treat as a file).
        ///
        /// file - truncate if exists, create if missing.
        /// pipe - append if path exists, error if missing.
        #[arg(long, default_value_t = false, verbatim_doc_comment)]
        pipe: bool,
    },

    /// Perform protocol layer download operation
    MpcDownload {
        /// IP address and port to listen on for the MPC client download communication.
        #[arg(long = "mpc-client-listen")]
        mpc_client_listen_addr: SocketAddr,

        /// Path to the file containing the data to be sent to the client.
        #[arg(long)]
        data_path: PathBuf,
    },
}

#[derive(Parser, Debug)]
#[command(author, version = sharemind_runtime::version::VERSION, about, long_about = None)]
struct Args {
    /// ID of this node (currently 1, 2 or 3).
    #[arg(long)]
    id: usize,

    /// IP address and port to listen on for right MPC peer.
    #[arg(long = "mpc-right-listen")]
    mpc_right_listen_addr: SocketAddr,

    /// IP address and port of left MPC peer to connect to.
    #[arg(long = "mpc-left-connect")]
    mpc_left_addr: String,

    /// Name of protection domain kind to instantiate.
    #[arg(long = "pdk")]
    pdk_name: String,

    /// Path to the protection domain module to load PDK from.
    ///
    /// Default: libsharemind_mod_{PDK}.so
    #[arg(long = "pd-module")]
    pd_module_path: Option<PathBuf>,

    /// Path to the protection domain module configuration file, if applicable.
    #[arg(long = "pd-module-conf")]
    pd_module_conf_path: Option<PathBuf>,

    /// Path to the protection domain configuration file.
    ///
    /// Default: {PDK}.conf
    #[arg(long = "pd-conf")]
    pd_conf_path: Option<PathBuf>,

    /// Additional configuration file search paths.
    ///
    /// Useful for finding config files in non-standard locations.
    #[arg(
        long = "config-path",
        env = "SHAREMIND_RUNTIME_CONFIG_PATH",
        value_delimiter = ':'
    )]
    config_paths: Vec<PathBuf>,

    /// Specify the Runtime command.
    #[command(subcommand)]
    command: Command,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args = Args::parse();
    let logger = setup_loghard_logging()?;
    // This binary is for IO and does not load additional features.
    let mut features = Features::without_feature_modules(
        cli_args.config_paths,
        cli_args.pd_module_path,
        cli_args.pd_module_conf_path,
        cli_args.pdk_name,
        cli_args.pd_conf_path,
    );

    // Create the network facility base object
    let mpc_client_listen_addr = match &cli_args.command {
        Command::MpcUpload {
            mpc_client_listen_addr,
            ..
        } => Some(mpc_client_listen_addr.to_string()),
        Command::MpcDownload {
            mpc_client_listen_addr,
            ..
        } => Some(mpc_client_listen_addr.to_string()),
    };
    let network_context = WorkerContext::new(
        cli_args.id,
        &cli_args.mpc_left_addr,
        &cli_args.mpc_right_listen_addr.to_string(),
        mpc_client_listen_addr.as_deref(),
    )?;

    let net_factory = Box::new(ZeromqNetworkFactory::new(network_context));
    let net_facility = NetworkFacility::new(net_factory);

    tracing::info!(node = cli_args.id, "starting");
    let runtime =
        &mut SharemindRuntime::new(&logger, net_facility, features.process_facilities.clone());
    features
        .load_modules(runtime)?
        .load_protection_domain(runtime)?;
    tracing::info!(node = cli_args.id, "started");

    match cli_args.command {
        Command::MpcUpload {
            mpc_client_listen_addr: _,
            data_path,
            pipe,
        } => {
            // If data_path is a file: truncate if exists, create if missing.
            // If data_path is a pipe: append if exists, error if missing.
            let mut output_writer = OpenOptions::new()
                .write(true)
                .create(!pipe)
                .truncate(!pipe)
                .open(&data_path)
                .with_context(|| {
                    format!(
                        "Failed to open path for writing uploaded data: {}",
                        data_path.display()
                    )
                })?;

            tracing::info!("Upload in progress");

            runtime
                .execute_upload_protocol(&mut output_writer)
                .map_err(anyhow::Error::from_boxed)
                .with_context(|| "Failed to execute the upload protocol")?;

            tracing::info!("Upload successful");
        }
        Command::MpcDownload {
            mpc_client_listen_addr: _,
            data_path,
        } => {
            let input_data = fs::read(&data_path).with_context(|| {
                format!(
                    "Failed to read data for download from path: {}",
                    data_path.display()
                )
            })?;

            tracing::info!("Download in progress");

            runtime
                .execute_download_protocol(&input_data)
                .map_err(anyhow::Error::from_boxed)
                .with_context(|| "Failed to execute the download protocol")?;

            tracing::info!("Download successful");
        }
    }

    Ok(())
}
