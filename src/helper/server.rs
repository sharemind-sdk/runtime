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

use crate::config::{build_config_search_paths, resolve_config_path};
use crate::ffi::loghard::LogHardLogger;
use crate::ffi::vm::ProcessFacilityMap;
use crate::program_io::streams::{read_program_argument_stream, write_program_result_stream};
use crate::program_io::{ProgramArgument, ProgramArgumentMap};
#[cfg(feature = "tabledb")]
use crate::syscalls::tabledb::rpc::TabledbRpc;
use crate::SharemindRuntime;
use anyhow::{anyhow, Context};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::exit;
use tracing_subscriber::fmt::time::LocalTime;

#[derive(Clone, Debug)]
pub struct KeyPath {
    pub key: String,
    pub path: PathBuf,
}

pub struct ModuleSpec {
    pub path: PathBuf,
    pub conf_path: Option<PathBuf>,
}

impl std::str::FromStr for KeyPath {
    type Err = String;

    /// Parse "key:path" into KeyPath.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let idx = s
            .find(':')
            .ok_or_else(|| format!("invalid key:path format: no ':' found in '{s}'"))?;

        let key = &s[..idx];
        let path = &s[idx + 1..];

        if key.is_empty() {
            return Err(format!(
                "invalid key:path format: key must not be empty in '{s}'"
            ));
        }
        if path.is_empty() {
            return Err(format!(
                "invalid key:path format: path must not be empty in '{s}'"
            ));
        }

        Ok(KeyPath {
            key: key.to_string(),
            path: PathBuf::from(path),
        })
    }
}

pub struct Features {
    // Paths to feature modules and their configurations
    pub modules: Vec<ModuleSpec>,
    // Collection of process facilities required by the feature modules
    pub process_facilities: ProcessFacilityMap,
    // Store the live objects of process facilities.
    pub process_facility_store: Vec<Box<dyn std::any::Any>>,

    #[cfg(feature = "tabledb")]
    pub tabledb_conf_path: PathBuf,
    #[cfg(feature = "keydb")]
    pub keydb_conf_path: PathBuf,
    #[cfg(feature = "profiler")]
    profiler_output_path: String,
    config_paths: Vec<PathBuf>,
    pd_module_path: Option<PathBuf>,
    pd_module_conf_path: Option<PathBuf>,
    pdk_name: String,
    pd_conf_path: Option<PathBuf>,
}

impl Features {
    pub fn new(
        #[cfg(feature = "profiler")] profiler_output_path: String,
        #[cfg(feature = "tabledb")] tabledb_conf_path: PathBuf,
        #[cfg(feature = "keydb")] keydb_conf_path: PathBuf,
        config_paths: Vec<PathBuf>,
        pd_module_path: Option<PathBuf>,
        pd_module_conf_path: Option<PathBuf>,
        pdk_name: String,
        pd_conf_path: Option<PathBuf>,
    ) -> Self {
        Features {
            modules: Vec::new(),
            process_facilities: ProcessFacilityMap::new(),
            process_facility_store: Vec::new(),
            #[cfg(feature = "profiler")]
            profiler_output_path,
            #[cfg(feature = "tabledb")]
            tabledb_conf_path,
            #[cfg(feature = "keydb")]
            keydb_conf_path,
            config_paths,
            pd_module_path,
            pd_module_conf_path,
            pdk_name,
            pd_conf_path,
        }
    }

    /// For binaries that never load the profiler, tabledb or keydb modules
    /// (e.g. protocol_io_server), even when built with those features.
    ///
    /// Their settings are left empty, so do not call `add_profiler_if_enabled`,
    /// `add_tabledb_if_enabled` or `add_keydb_if_enabled` on the result.
    pub fn without_feature_modules(
        config_paths: Vec<PathBuf>,
        pd_module_path: Option<PathBuf>,
        pd_module_conf_path: Option<PathBuf>,
        pdk_name: String,
        pd_conf_path: Option<PathBuf>,
    ) -> Self {
        Self::new(
            #[cfg(feature = "profiler")]
            String::new(),
            #[cfg(feature = "tabledb")]
            PathBuf::new(),
            #[cfg(feature = "keydb")]
            PathBuf::new(),
            config_paths,
            pd_module_path,
            pd_module_conf_path,
            pdk_name,
            pd_conf_path,
        )
    }

    pub fn add_algorithms_module(&mut self) -> &mut Features {
        // Contains algorithm syscalls used in SecreC programs
        self.modules.push(ModuleSpec {
            path: PathBuf::from("libsharemind_mod_algorithms.so"),
            conf_path: None,
        });
        self
    }

    pub fn add_profiler_if_enabled(
        &mut self,
        logger: &mut LogHardLogger,
    ) -> Result<&mut Self, String> {
        #[cfg(feature = "profiler")]
        {
            // Create execution profiler process facility
            let mut profiler = Box::new(crate::ffi::facility::profiler::ProfilerFacility::new(
                logger,
            )?);
            profiler
                .start_log(&std::ffi::CString::new(self.profiler_output_path.as_str()).unwrap());
            self.process_facilities.set_facility(profiler.as_mut());
            self.process_facility_store.push(profiler);

            self.modules.push(ModuleSpec {
                path: PathBuf::from("libsharemind_mod_executionprofiler.so"),
                conf_path: None,
            });
        }
        Ok(self)
    }

    pub fn add_tabledb_if_enabled(&mut self) -> Result<&mut Self, String> {
        #[cfg(feature = "tabledb")]
        {
            // Create the tabledb RPC facility for the tabledb syscalls
            let mut tabledb_rpc = Box::new(
                TabledbRpc::new(std::path::Path::new("/tmp/tabledb_rpc.sock"))
                    .map_err(|e| format!("Error creating TabledbRpc facility: {}", e))?,
            );
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

    /// Loads mod_keydb, which gives SecreC programs a Redis key-value store.
    /// The module connects directly to the Redis hosts listed in its
    /// configuration. In a three-node deployment, every node has its own Redis
    /// instance.
    ///
    /// Access control is left to an outer layer, so the configuration must set
    /// `[AccessControl] Enabled = false`.
    ///
    /// No `ConsensusService` module facility is provided either.
    pub fn add_keydb_if_enabled(&mut self) -> &mut Self {
        #[cfg(feature = "keydb")]
        {
            self.modules.push(ModuleSpec {
                path: PathBuf::from("libsharemind_mod_keydb.so"),
                conf_path: Some(self.keydb_conf_path.clone()),
            });
        }
        self
    }

    pub fn load_modules(
        &mut self,
        runtime: &mut SharemindRuntime,
    ) -> Result<&mut Self, Box<dyn std::error::Error>> {
        let config_search_dirs = build_config_search_paths("sharemind", &*self.config_paths);
        for module in &self.modules {
            let conf_path = module
                .conf_path
                .as_deref()
                .map(|p| resolve_config_path(p, &config_search_dirs))
                .transpose()?;
            runtime.load_module(&module.path, conf_path.as_deref())?;
        }
        Ok(self)
    }

    pub fn load_protection_domain(
        &mut self,
        runtime: &mut SharemindRuntime,
    ) -> Result<&mut Self, Box<dyn std::error::Error>> {
        let config_search_dirs = build_config_search_paths("sharemind", &*self.config_paths);
        // Load protection domain
        let pd_module_path = self
            .pd_module_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(format!("libsharemind_mod_{}.so", self.pdk_name)));
        let pd_module_conf_path = self
            .pd_module_conf_path
            .as_deref()
            .map(|p| resolve_config_path(p, &config_search_dirs))
            .transpose()?;
        let pd_conf_path = resolve_config_path(
            &*self
                .pd_conf_path
                .clone()
                .unwrap_or_else(|| PathBuf::from(format!("{}.conf", self.pdk_name))),
            &config_search_dirs,
        )?;

        runtime.load_pd(
            &self.pdk_name,
            &pd_conf_path,
            &pd_module_path,
            pd_module_conf_path.as_deref(),
        )?;
        Ok(self)
    }
}

pub fn setup_loghard_logging() -> Result<LogHardLogger, Box<dyn Error>> {
    // Initialize tracing for structured logging.
    // Environment variable RUST_LOG can be used to customize the logging level.
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or("info".into());
    let timer = LocalTime::new(time::format_description::parse(
        "[year]-[month padding:zero]-[day padding:zero] [hour]:[minute]:[second]",
    )?);
    let sub = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_timer(timer)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(sub)
        .expect("Failed to set tracing global default subscriber");

    // Setup LogHard logging
    // Shared between the module and Rust's logging facade.
    #[allow(unused_mut)]
    let mut logger = LogHardLogger::new(
        &tracing::level_filters::LevelFilter::current().to_string(),
        c"",
    );
    if let Err(e) = log::set_boxed_logger(Box::new(logger.sublogger(c"[Log]"))) {
        println!("{}", e);
        exit(1);
    };
    Ok(logger)
}

pub fn run_execute_command(
    runtime: &mut SharemindRuntime,
    bytecode: String,
    argstream_listen_addr: Option<SocketAddr>,
    argument_slots: Vec<KeyPath>,
    result_slots: Vec<KeyPath>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate inputs before performing any potentially expensive operations.

    // Check for duplicates in argument slots
    {
        let mut seen = HashSet::new();
        if let Some(dupe) = argument_slots.iter().find(|s| !seen.insert(s.key.clone())) {
            return Err(format!(
                "Provided argument slots contained a duplicate key: {}",
                dupe.key
            )
            .into());
        }
    }

    // Convert result slots vector to hashmap, while checking for duplicates.
    let result_slot_map = result_slots
        .into_iter()
        .try_fold(HashMap::new(), |mut acc, slot| {
            if acc.insert(slot.key.clone(), slot.path).is_some() {
                Err(anyhow!("Duplicate key: {}", slot.key))
            } else {
                Ok(acc)
            }
        })
        .context("Provided result slots contained duplicate keys")?;

    // Receive arguments from argstream, if available (optional)
    let receive_argstream = |socket_addr: &SocketAddr| -> std::io::Result<(
        TcpListener,
        TcpStream,
        ProgramArgumentMap,
    )> {
        let listener = TcpListener::bind(socket_addr)?;
        tracing::debug!("Listening for program arguments at {socket_addr}");
        let (mut socket, client_addr) = listener.accept()?;
        tracing::debug!("Client connected from {client_addr}");
        let argument_map = read_program_argument_stream(&mut socket)?;
        tracing::info!(
                    "Received {} program arguments from argstream",
                    argument_map.len()
                );
        Ok((listener, socket, argument_map))
    };

    let (_argstream_listener, mut argstream_socket, mut argument_map) =
        if let Some(addr) = &argstream_listen_addr {
            receive_argstream(addr)
                .map(|(l, s, m)| (Some(l), Some(s), m))
                .context("Failed to receive program arguments from argstream")?
        } else {
            (None, None, ProgramArgumentMap::new())
        };

    // Read arguments from slots provided via CLI
    for slot in &argument_slots {
        match argument_map.entry(slot.key.clone()) {
            Entry::Occupied(_) => {
                return Err(format!("Duplicate program argument: {}", &slot.key).into());
            }
            Entry::Vacant(vacant_entry) => {
                tracing::info!("Program argument '{}': {}", slot.key, slot.path.display());
                let data = fs::read(&slot.path)
                    .with_context(|| format!("Failed to read program argument '{}'", slot.key))?;
                vacant_entry.insert(ProgramArgument { data });
            }
        }
    }

    tracing::info!(
        "Read {} program arguments from provided slots",
        argument_slots.len()
    );

    // Execute a bytecode process
    tracing::info!("Executing program: {bytecode}");
    let time_start = std::time::Instant::now();
    let result_map = runtime.execute_program(&bytecode, argument_map)?;
    let duration = time_start.elapsed();
    tracing::info!(
        "Execution successful (took {} microseconds)",
        duration.as_micros()
    );

    // Write published program results to the argstream socket (optional)
    if let Some(argstream_socket) = argstream_socket.as_mut() {
        tracing::info!("Sending {} program results via argstream", result_map.len());
        write_program_result_stream(argstream_socket, &result_map)
            .context("Failed to send program results via argstream")?;
        tracing::info!("Sent program results successfully");
    }

    //
    // Write published results to slots provided via CLI
    //

    // Check that every provided result slot is represented in published results
    for key in result_slot_map.keys() {
        if !result_map.contains_key(key) {
            return Err(format!("Result slot key '{key}' not found in published results").into());
        }
    }

    // Write published program results to slots
    let mut written_result_count: usize = 0;
    for (key, result) in &result_map {
        match result_slot_map.get(key) {
            Some(path) => {
                tracing::info!("Program result '{key}': {}", path.display());
                fs::write(path, &result.data)
                    .with_context(|| format!("Failed to write program result '{key}'"))?;
                written_result_count += 1;
            }
            None => tracing::warn!("No slot provided for program result '{key}'"),
        }
    }

    tracing::info!(
        "Wrote {written_result_count}/{} program results to provided slots",
        result_map.len()
    );
    Ok(())
}
