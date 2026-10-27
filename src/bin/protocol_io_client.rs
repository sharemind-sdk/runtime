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

use clap::Parser;
use sharemind_runtime::ffi::facility::network::NetworkFacility;
use sharemind_runtime::helper::client;
use sharemind_runtime::helper::client::Command;
use sharemind_runtime::network::zeromq;
use sharemind_runtime::network::zeromq::client::{NetworkConfig, NodeConfig, ZeromqNetworkFactory};
use std::cell::RefCell;
use std::error::Error;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Parser, Debug)]
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

    /// Each `--server` argument describes an MPC server that the PD needs to communicate with.
    /// The format of the argument is a comma-separated string of key=value pairs.
    /// The key for address is `addr`.
    #[arg(long = "server", verbatim_doc_comment)]
    servers: Vec<String>,

    /// Specify the operation to execute.
    #[command(subcommand)]
    command: Command,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args = Args::parse();
    client::setup_logging()?;
    let Args {
        servers,
        pd_module_path,
        pdk_name,
        pd_conf_path,
        pd_typename,
        plaintext_path,
        pipe,
        command,
        ..
    } = cli_args;
    let network_facility = create_network_facility(servers)?;
    let (_pd_name, pd) =
        client::create_protection_domain(pd_module_path, pdk_name, pd_conf_path, network_facility)?;

    // Execute the selected operation
    match command {
        Command::Classify => {
            tracing::info!(plaintext = ?plaintext_path, "Classify");
            // Read the input value data
            let plaintext_data = client::read_input_value_data(plaintext_path)?;
            // In the protocol mode the "classify" operation does not return
            // the protected data, but sends it directly to the server-side
            // PD counterparts on each MPC node as part of the protocol.
            pd.classify_protocol(&plaintext_data, &pd_typename)?;
        }

        Command::Declassify => {
            let mut plaintext_writer = client::prepare_output_data_writer(pipe, plaintext_path)?;
            // In the protocol mode the "declassify" operation receives the
            // protected data directly from the server-side PD counterparts
            // on each MPC node, then reconstructs the plaintext data and
            // writes it to the provided output path.
            pd.declassify_protocol(&pd_typename, &mut plaintext_writer)?;
        }
    }
    tracing::info!("Success");

    Ok(())
}

/// Create the network facility if any servers have been configured.
/// PDs in protocol mode require network communication with the physical server nodes.
fn create_network_facility(
    servers: Vec<String>,
) -> Result<Option<Rc<RefCell<NetworkFacility>>>, Box<dyn Error>> {
    if servers.is_empty() {
        return Err("Servers have not been configured".into());
    }

    // Load server network configuration (addresses, TLS, etc.)
    let server_configs = servers
        .iter()
        .map(|s| NodeConfig::from_arg(s).map_err(|e| format!("Invalid server '{s}': {e}")))
        .collect::<Result<Vec<_>, _>>()?;
    let network_config = NetworkConfig {
        nodes: server_configs,
    };

    // Create the network facility
    let network_context = zeromq::client::WorkerContext::new(&network_config)
        .map_err(|e| format!("Unable to create network context: {e}"))?;

    let network_factory = Box::new(ZeromqNetworkFactory::new(network_context));
    Ok(Some(Rc::new(RefCell::new(NetworkFacility::new(
        network_factory,
    )))))
}
