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

#![doc = include_str!("../../docs/emulator.md")]
use clap::Parser;
use sharemind_runtime::{
    ffi::facility::network::NetworkFacility,
    helper::server::{run_execute_command, setup_loghard_logging, Features, KeyPath},
    network::in_memory::{context::WorkerContext, InmemoryNetworkFactory},
    SharemindRuntime,
};
use std::path::PathBuf;
use std::sync::mpsc::channel;

#[cfg(feature = "tabledb")]
use ::{
    sharemind_runtime::{
        helper::server::ModuleSpec,
        syscalls::tabledb::rpc::TabledbRpc,
        test_util::tabledb_mock::{MockDatabaseServer, MockDatabaseState},
    },
    std::{cell::RefCell, rc::Rc},
};

#[derive(Parser, Debug)]
struct Args {
    /// Path to the loadable PD module file
    ///
    /// Default: libsharemind_mod_shared3p_emu.so
    #[arg(long = "pd-module")]
    pd_module_path: Option<PathBuf>,

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

    /// Program argument slots in the form 'key:path'.
    #[arg(long = "arg")]
    argument_slots: Vec<KeyPath>,

    /// Program result slots in the form 'key:path'.
    #[arg(long = "result")]
    result_slots: Vec<KeyPath>,
    // TODO: The number of published results may be unknown in advance.
    //  We could add a path for storing any number of indexed results produced by the bytecode.
}

trait EmulatorFeatures {
    fn add_mocked_tabledb_if_enabled(&mut self) -> Result<&mut Self, Box<dyn std::error::Error>>;
}

impl EmulatorFeatures for Features {
    fn add_mocked_tabledb_if_enabled(&mut self) -> Result<&mut Self, Box<dyn std::error::Error>> {
        #[cfg(feature = "tabledb")]
        {
            // Create the TableDB RPC facility mocked with an in-memory database with observable shared state
            let tabledb_state = Rc::new(RefCell::new(MockDatabaseState::new()));
            let tabledb_client = MockDatabaseServer::new(tabledb_state.clone()).into_client();
            let mut tabledb_rpc = Box::new(TabledbRpc::with_client(tabledb_client)?);
            // Set up the mock database state
            tabledb_state
                .borrow_mut()
                .modify_datasource_tables("DS1", true, |ds_tables| {
                    ds_tables.clear(); // reset tables
                    Ok(())
                })
                .unwrap();
            self.process_facilities.set_facility(tabledb_rpc.as_mut());
            self.process_facility_store.push(tabledb_rpc);

            // Contains tabledb and vmap syscalls.
            // Some of tabledb syscalls we later override with Runtime implementations.
            self.modules.push(ModuleSpec {
                path: PathBuf::from("libsharemind_mod_tabledb.so"),
                conf_path: Some(self.tabledb_conf_path.clone()),
            });
        }
        Ok(self)
    }
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
        Some(
            cli_args
                .pd_module_path
                .unwrap_or_else(|| PathBuf::from("libsharemind_mod_shared3p_emu.so")),
        ),
        None,
        "shared3p".to_string(),
        cli_args.pd_conf_path,
    );
    features
        .add_algorithms_module()
        .add_profiler_if_enabled(&mut logger)?
        .add_mocked_tabledb_if_enabled()?
        .add_keydb_if_enabled();

    // The emulator is a single node and the emulated PD does not communicate
    // with peers, so use an in-memory network that nothing is connected to.
    let (send_left, recv_left) = channel();
    let (send_right, recv_right) = channel();
    let network_context = WorkerContext {
        id: 0,
        send_left,
        send_right,
        recv_left,
        recv_right,
    };
    let net_factory = Box::new(InmemoryNetworkFactory::new(network_context));
    let net_facility = NetworkFacility::new(net_factory);

    tracing::info!("starting emulator");
    let runtime =
        &mut SharemindRuntime::new(&logger, net_facility, features.process_facilities.clone());
    features
        .load_modules(runtime)?
        .load_protection_domain(runtime)?;
    tracing::info!("started emulator");

    run_execute_command(
        runtime,
        cli_args.bytecode,
        None,
        cli_args.argument_slots,
        cli_args.result_slots,
    )?;

    Ok(())
}
