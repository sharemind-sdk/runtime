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
use std::net::SocketAddr;
use std::path::PathBuf;

use sharemind_runtime::helper::server::KeyPath;
use sharemind_runtime::{
    ffi::facility::network::NetworkFacility,
    helper::server::{run_execute_command, setup_loghard_logging, Features},
    network::zeromq::server::{WorkerContext, ZeromqNetworkFactory},
    SharemindRuntime,
};

#[derive(Clone, Parser, Debug)]
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

    #[cfg(feature = "tabledb")]
    /// Path to the tabledb module configuration file.
    #[arg(long = "tabledb-conf", default_value = "tabledb.conf")]
    tabledb_conf_path: PathBuf,

    #[cfg(feature = "keydb")]
    /// Path to the keydb module configuration file.
    #[arg(long = "keydb-conf", default_value = "keydb.conf")]
    keydb_conf_path: PathBuf,

    #[cfg(feature = "profiler")]
    /// Path to the profiler output file
    #[arg(long = "profiler-out", default_value = "profiler.txt")]
    profiler_output_path: String,

    /// Additional configuration file search paths.
    ///
    /// Useful for finding config files in non-standard locations.
    #[arg(
        long = "config-path",
        env = "SHAREMIND_RUNTIME_CONFIG_PATH",
        value_delimiter = ':'
    )]
    config_paths: Vec<PathBuf>,

    /// Path to the SecreC bytecode to execute
    #[arg(long)]
    bytecode: String,

    /// IP address and port to listen on for application layer argstream communication.
    ///
    /// Default: no argstreams being processed.
    #[arg(long = "argstream-listen")]
    argstream_listen_addr: Option<SocketAddr>,

    /// Program argument slots in the form 'key:path'.
    #[arg(long = "arg")]
    argument_slots: Vec<KeyPath>,

    /// Program result slots in the form 'key:path'.
    #[arg(long = "result")]
    result_slots: Vec<KeyPath>,
    // TODO: The number of published results may be unknown in advance.
    //  We could add a path for storing any number of indexed results produced by the bytecode.
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args = Args::parse();
    let mut logger = setup_loghard_logging()?;
    let mut features = Features::new(
        #[cfg(feature = "profiler")]
        cli_args.profiler_output_path,
        #[cfg(feature = "tabledb")]
        cli_args.tabledb_conf_path,
        #[cfg(feature = "keydb")]
        cli_args.keydb_conf_path,
        cli_args.config_paths,
        cli_args.pd_module_path,
        cli_args.pd_module_conf_path,
        cli_args.pdk_name,
        cli_args.pd_conf_path,
    );
    features
        .add_algorithms_module()
        .add_profiler_if_enabled(&mut logger)?
        .add_tabledb_if_enabled()?
        .add_keydb_if_enabled();

    let network_context = WorkerContext::new(
        cli_args.id,
        &cli_args.mpc_left_addr,
        &cli_args.mpc_right_listen_addr.to_string(),
        None::<String>.as_deref(),
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

    run_execute_command(
        runtime,
        cli_args.bytecode,
        cli_args.argstream_listen_addr,
        cli_args.argument_slots,
        cli_args.result_slots,
    )?;
    Ok(())
}
