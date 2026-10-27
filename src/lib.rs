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

// Sharemind error code variable names
#![allow(non_upper_case_globals)]

use std::{
    cell::RefCell,
    error::Error,
    ffi::{c_void, CString},
    io::Write,
    ops::DerefMut,
    path::Path,
    ptr,
    rc::Rc,
};

use crate::ffi::{
    bindings::{
        ProcessExitStatus_PROCESS_EXIT_OK, SharemindCodeBlock, SharemindModuleApi0x1CReference,
        SharemindModuleApi0x1PdpiInfo, SharemindModuleApi0x1SyscallContext,
        SharemindSyscallWrapper,
    },
    facility::{
        data_store::DataStoreFactory, network::NetworkFacility,
        pd_network::PdNetworkFacilityWrapper, process::ProcessFacility, random::RandomFacility,
    },
    io::{AsFfiCallback, WriteCallbackCtx},
    loghard::LogHardLogger,
    modapi::{self, pdpi::Pdpi, Module, ModuleApi, ModuleApiError, Pd},
    vm::{
        syscall::cache::{SyscallCache, SyscallFinderWrapper},
        ProcessFacilityFinderWrapper, ProcessFacilityMap, RustProcessWrapper,
    },
    wrapper::{pdpi_network_facility::PdpiNetworkFacilityWrapper, WrappedC},
};
use program_io::{ProgramArgumentMap, ProgramResultMap};
use scopeguard::defer;

#[macro_use]
extern crate static_assertions;

#[cfg(feature = "tabledb")]
pub mod tabledb_capnp {
    include!(concat!(env!("OUT_DIR"), "/tabledb_capnp.rs"));
}

pub mod common;
pub mod config;
pub mod ffi;
pub mod helper;
pub mod network;
pub mod program_io;
pub mod syscalls;
pub mod test_util;

pub mod version;

/// The runtime for executing Sharemind bytecode programs.
///
/// It uses the Sharemind Module API to glue together the syscalls from the
/// feature and protection domain modules with the Sharemind VM and auxiliary
/// functionalities in the form of facilities in order to create an environment
/// for executing the Sharemind bytecode programs.
///
/// # Limitations
///
/// ## Number of simultaneous protection domains
///
/// Current implementation limits the number of protection domains usable at
/// the same time to at most one. This was a conscious decision, because
/// while the support for multiple simultaneous protection domains looks
/// nice on paper, the feature did not find use in practice. If a bytecode
/// program uses more than one protection domain, then executing the
/// program would result in an error.
///
/// What is missing in order to support multiple protection domains:
/// 1. `NetworkFacility` needs to support creation of multiple networks.
///    For details refer to `NetworkFacility::new_network()` documentation.
/// 2. `Self::execute_program()` must create and initialize multiple PDPI
///    objects (one per protection domain).
/// 3. `RustProcessWrapper` must be refactored to work with multiple PDPI
///    objects.
/// 4. The order of PDPI (and by extention PDPI info) objects must be the
///    same as the order of PDs in the linking units of an executable
///    bytecode program. This is to allow `get_pdpi_info()` to find the
///    correct PDPI info by PD index. For details on how the Sharemind MPC
///    Server keeps PDs in the correct order refer to
///    `createUsedProtectionDomains()` in `server/src/CachedProgram.cpp`
pub struct SharemindRuntime {
    // General runtime structures
    mod_api: Rc<ModuleApi>,
    modules: Vec<Rc<Module>>,
    protection_domain: Option<Rc<Pd>>,
    syscall_cache: Box<SyscallCache>,
    runtime_syscalls_installed: bool,
    net_facility: Rc<RefCell<NetworkFacility>>, // used by pd and pdpi net facilities to create networks

    // Module-level facilities
    _logger: Box<LogHardLogger>,

    // PD-level facilities
    _pd_random_facility: Box<RandomFacility>,
    _pd_net_facility: Box<PdNetworkFacilityWrapper>,

    // Syscall-level process facilities
    predefined_process_facilities: ProcessFacilityMap,
}

impl SharemindRuntime {
    pub fn new(
        logger: &LogHardLogger,
        net_facility: NetworkFacility,
        process_facilities: ProcessFacilityMap,
    ) -> Self {
        let mod_api = Rc::new(ModuleApi::new().unwrap());

        // Install logger
        let mut sublogger = Box::new(
            logger.sublogger(
                CString::new(format!("[Node_{}]", net_facility.local_node_id()))
                    .expect("Invalid C string")
                    .as_c_str(),
            ),
        );
        mod_api.set_module_facility(sublogger.as_mut()).unwrap();

        // Some PDs might require Random as PD facility
        let mut random_facility = Box::new(RandomFacility::new());
        mod_api.set_pd_facility(random_facility.as_mut()).unwrap();

        // Make network facility shared and mutable
        let net_facility = Rc::new(RefCell::new(net_facility));

        // Install PD-level network facility
        let mut pd_net_facility = Box::new(PdNetworkFacilityWrapper::new(net_facility.clone()));
        mod_api.set_pd_facility(pd_net_facility.as_mut()).unwrap();

        Self {
            mod_api,
            modules: Vec::new(),
            protection_domain: None,
            syscall_cache: Box::new(SyscallCache::default()),
            runtime_syscalls_installed: false,
            net_facility,
            _logger: sublogger,
            _pd_random_facility: random_facility,
            _pd_net_facility: pd_net_facility,
            predefined_process_facilities: process_facilities,
        }
    }

    /// Load any module that implements the Sharemind Module API
    pub fn load_module(
        &mut self,
        module_path: &Path,
        conf_path: Option<&Path>,
    ) -> Result<Rc<Module>, Box<dyn Error>> {
        // Create the module
        let module =
            Module::new(self.mod_api.clone(), module_path, conf_path).inspect_err(|e| {
                tracing::error!(
                    "Failed to load module {:?} with module config path {:?}: {e}",
                    module_path,
                    conf_path
                )
            })?;

        let rc_module = Rc::new(module);
        self.modules.push(rc_module.clone());

        // Cache the syscalls provided by the module
        {
            self.syscall_cache.cache_module_syscalls(&rc_module);

            // Trigger reinstallation of runtime syscalls just in case
            // the loaded module has overridden any runtime syscalls.
            self.runtime_syscalls_installed = false;
        }

        if let Some(conf_path) = conf_path {
            tracing::info!("Loaded module {:?} with conf {:?}", module_path, conf_path);
        } else {
            tracing::info!("Loaded module {:?} with no conf", module_path);
        }
        Ok(rc_module)
    }

    /// Load a protection domain module
    pub fn load_pd(
        &mut self,
        pdk_name: &str,
        pd_conf_path: &Path,
        module_path: &Path,
        module_conf_path: Option<&Path>,
    ) -> Result<(), Box<dyn Error>> {
        if self.protection_domain.is_some() {
            return Err("Current implementation can load at most one protection domain".into());
        }

        // Load the module
        let module = self.load_module(module_path, module_conf_path)?;

        // Create the protection domain
        let pd = Pd::new(module, pdk_name, pd_conf_path)?;
        pd.start().inspect_err(|e| {
            tracing::error!(
                "Failed to start protection domain {} (with config path {}): {}",
                pdk_name,
                pd_conf_path.display(),
                e
            );
            self.modules.pop(); // unload module on error
        })?;
        self.protection_domain = Some(Rc::new(pd));
        tracing::info!(
            "Started protection domain: kind {:?}, conf {:?}",
            pdk_name,
            pd_conf_path
        );

        Ok(())
    }

    /// Execute a Sharemind bytecode program
    pub fn execute_program(
        &mut self,
        bytecode_path: &str,
        arguments: ProgramArgumentMap,
    ) -> Result<ProgramResultMap, Box<dyn Error>> {
        // Ensure the runtime syscall overrides are in place
        self.install_runtime_syscalls();

        // Install PDPI-level network facility
        let mut pdpi_net_facility = PdpiNetworkFacilityWrapper::new(self.net_facility.clone());
        self.mod_api.set_pdpi_facility(&mut pdpi_net_facility)?;
        defer! {
            self.mod_api.unset_pdpi_facility(&pdpi_net_facility);
        }

        // Install PDPI-level randomness facility
        let mut random_facility = RandomFacility::new();
        self.mod_api.set_pdpi_facility(&mut random_facility)?;
        defer! {
            self.mod_api.unset_pdpi_facility(&random_facility);
        }

        // Install PDPI-level process (metadata) facility
        let mut process_metadata_facility = ProcessFacility::new();
        self.mod_api
            .set_pdpi_facility(&mut process_metadata_facility)?;
        let mut process_metadata_facility = scopeguard::guard(process_metadata_facility, |f| {
            self.mod_api.unset_pdpi_facility(&f);
        });

        // Start PD process instance
        let pdpi = self
            .protection_domain
            .as_ref()
            .map(|pd| -> Result<Pdpi, ModuleApiError> {
                let pdpi = Pdpi::new(pd.clone())?;
                pdpi.start()?;
                Ok(pdpi)
            })
            .transpose()
            .map_err(|e| format!("PDPI error: {e}"))?;

        // Install syscall-level process facilities
        // Clone the map of predefined process facilities and add some more
        let mut process_facilities = self.predefined_process_facilities.clone();
        process_facilities.set_facility(process_metadata_facility.deref_mut());

        let mut process_wrapper = RustProcessWrapper::new(pdpi.as_ref(), arguments);
        process_facilities.set_facility(&mut process_wrapper);

        let mut data_store_factory = DataStoreFactory::new();
        process_facilities.set_facility(&mut data_store_factory);

        // Setup finders
        let mut syscall_finder = SyscallFinderWrapper::new(self.syscall_cache.as_ref());
        process_facilities.set_facility(&mut syscall_finder);
        let process_facility_finder = ProcessFacilityFinderWrapper::new(process_facilities);

        // Spawn the process
        unsafe {
            let bytecode_cstr = CString::new(bytecode_path).expect("Invalid C string");
            let exit_status = ffi::bindings::spawn(
                bytecode_cstr.as_ptr(),
                &process_facility_finder.inner as *const _,
                &syscall_finder.inner as *const _,
            );

            if exit_status != ProcessExitStatus_PROCESS_EXIT_OK {
                return Err(format!(
                    "error executing the program {}, exit status {}",
                    bytecode_path, exit_status
                )
                .into());
            }
        };

        Ok(process_wrapper.program_results)
    }

    pub fn execute_upload_protocol<W: Write>(
        &self,
        output_writer: &mut W,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut write_callback_ctx = WriteCallbackCtx::new(output_writer);

        // Prepare syscall parameters
        let (cb_fn, cb_ctx) = write_callback_ctx.as_ffi_callback();
        const pd_index: u64 = 0;
        let mut args = [
            SharemindCodeBlock { uint64: [pd_index] },
            SharemindCodeBlock { cp: [cb_fn] },
            SharemindCodeBlock { p: [cb_ctx] },
        ];

        self.execute_io_protocol_with("upload_vec", Some(&mut args), None)
    }

    /// # Design rationale: input provisioning method
    ///
    /// Protocols may invoke sub-protocols. Composable protocols expect inputs
    /// as implementation specific data structures, e.g. owned ShareVec<T> objects.
    ///
    /// There are multiple ways we could provide input to such protocols:
    /// a) Invoke the `new_vec<T>` syscall to allocate ShareVec<T> and then
    ///    `set_shares<T>` to set its contents (because it is a module impl
    ///    specific C++ object). That makes two additional syscalls and one
    ///    extra copy of input data - suboptimal.
    /// b) Pass syscall inputs via crefs. The syscall will have to copy data
    ///    into a ShareVec<T>, or its protocol interfaces could be extended
    ///    with a vector-like type that wraps a cref, e.g. CRefVec.
    /// c) Provide syscall with a reader callback. The syscall creates a new
    ///    ShareVec<T> and fills it directly using the provided callback.
    ///
    /// In all cases we are dealing with piped inputs - we do not know the data
    /// length until we've read all of it. In (a) and (b) the runtime reads the
    /// data into a buffer, in (c) the syscall does. Multiple buffer reallocations
    /// may happen as more data is appended. This can be alleviated by processing
    /// the input in smaller chunks in (c) or prepending the piped data with size.
    ///
    /// Downloading data is a one-shot operation done only a handful of times.
    /// It is better to keep the protocols simple, rather than over-engineer for
    /// memory or speed optimizations without a real need. We already use (b)
    /// for the upload operation on the client side. If protocols manage to wrap
    /// crefs, they could even achieve no extra copies of data. Streaming is not
    /// required as of now. So (b) it is.
    pub fn execute_download_protocol(
        &self,
        input_data: &[u8],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Prepare syscall parameters
        const pd_index: u64 = 0;
        let mut args = [SharemindCodeBlock { uint64: [pd_index] }];

        let mut crefs = vec![
                SharemindModuleApi0x1CReference {
                    internal: ptr::null(),
                    pData: ptr::null(),
                    size: 0
                };
                2 // last one is null-terminator
            ];

        crefs[0].pData = input_data.as_ptr() as *const c_void;
        crefs[0].size = input_data.len();

        self.execute_io_protocol_with("download_vec", Some(&mut args), Some(&crefs))
    }

    fn execute_io_protocol_with(
        &self,
        syscall_name: &str,
        args: Option<&mut [SharemindCodeBlock]>,
        crefs: Option<&[SharemindModuleApi0x1CReference]>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // TODO: if we require PD to exist, then maybe this should be a method
        //       on PD instead? Or a separate static function?
        let pd = self
            .protection_domain
            .as_ref()
            .ok_or("Missing protection domain")?;

        // TODO: Setup is similar to execute_program. Deduplicate?

        // Install PDPI-level network facility
        let mut pdpi_net_facility = PdpiNetworkFacilityWrapper::new(self.net_facility.clone());
        self.mod_api.set_pdpi_facility(&mut pdpi_net_facility)?;
        defer! {
            self.mod_api.unset_pdpi_facility(&pdpi_net_facility);
        }

        // Install PDPI-level randomness facility
        let mut random_facility = RandomFacility::new();
        self.mod_api.set_pdpi_facility(&mut random_facility)?;
        defer! {
            self.mod_api.unset_pdpi_facility(&random_facility);
        }

        // Install PDPI-level process (metadata) facility
        let mut process_metadata_facility = ProcessFacility::new();
        self.mod_api
            .set_pdpi_facility(&mut process_metadata_facility)?;
        defer! {
            self.mod_api.unset_pdpi_facility(&process_metadata_facility);
        }

        // Create PD Process Instance
        let pdpi = Pdpi::new(pd.clone())?;
        pdpi.start()
            .map_err(|e| format!("PDPI error on start: {e}"))?;

        // No syscall-level process facilities are expected
        assert!(self.predefined_process_facilities.is_empty());

        // Retrieve syscall
        let syscall_name = format!("{}::{}", pd.pdk_name(), syscall_name);
        let syscall_wrapper = self
            .syscall_cache
            .find_syscall(&syscall_name)
            .ok_or_else(|| format!("Syscall {syscall_name} not found"))?;

        // Single-use context for the operation syscall
        let mut syscall_context =
            SimpleSyscallContextWrapper::new(syscall_wrapper, Some(pdpi.pdpi_info()));

        // Execute syscall
        tracing::trace!("Invoking syscall: {syscall_name}");
        Module::invoke_syscall(
            syscall_wrapper,
            args,
            None,
            crefs,
            None,
            &mut syscall_context.inner,
        )
        .map_err(|e| format!("Syscall {syscall_name} failed: {e}").into())
    }

    /// Install syscalls provided by the runtime
    /// Tabledb module syscalls are overridden (when tabledb feature is enabled).
    fn install_runtime_syscalls(&mut self) {
        if !self.runtime_syscalls_installed {
            macro_rules! cache_raw_syscall {
                ($signature:expr, $syscall:expr) => {
                    self.syscall_cache
                        .cache_raw_syscall($signature, Some($syscall));
                };
            }
            /* Process I/O operations */
            cache_raw_syscall!("Process_logString", syscalls::io::log_string_syscall);
            cache_raw_syscall!("Process_argument", syscalls::io::argument_syscall);
            cache_raw_syscall!("Process_setResult", syscalls::io::set_result_syscall);

            /* PD-indifferent array rearranging operations */
            cache_raw_syscall!("arrgather", syscalls::arranging::arrgather_syscall);
            cache_raw_syscall!("arrscatter", syscalls::arranging::arrscatter_syscall);

            cache_raw_syscall!("blockingRandomize", syscalls::random::blocking_randomize);

            #[cfg(feature = "tabledb")]
            {
                /* High level database operations */
                cache_raw_syscall!("tdb_open", syscalls::tabledb::tdb_open);
                cache_raw_syscall!("tdb_close", syscalls::tabledb::tdb_close);
                cache_raw_syscall!("tdb_table_names", syscalls::tabledb::tdb_table_names);
                /* Table database API */
                cache_raw_syscall!("tdb_tbl_create", syscalls::tabledb::tdb_tbl_create);
                cache_raw_syscall!("tdb_tbl_create2", syscalls::tabledb::tdb_tbl_create2);
                cache_raw_syscall!("tdb_tbl_delete", syscalls::tabledb::tdb_tbl_delete);
                cache_raw_syscall!("tdb_tbl_exists", syscalls::tabledb::tdb_tbl_exists);
                cache_raw_syscall!("tdb_tbl_col_count", syscalls::tabledb::tdb_tbl_col_count);
                cache_raw_syscall!("tdb_tbl_col_names", syscalls::tabledb::tdb_tbl_col_names);
                cache_raw_syscall!("tdb_tbl_col_types", syscalls::tabledb::tdb_tbl_col_types);
                cache_raw_syscall!("tdb_tbl_row_count", syscalls::tabledb::tdb_tbl_row_count);
                cache_raw_syscall!("tdb_insert_row", syscalls::tabledb::tdb_insert_row);
                cache_raw_syscall!("tdb_insert_row2", syscalls::tabledb::tdb_insert_row2);
                cache_raw_syscall!("tdb_read_col", syscalls::tabledb::tdb_read_col);
                cache_raw_syscall!("tdb_get_attributes", syscalls::tabledb::tdb_get_attributes);
                cache_raw_syscall!("tdb_set_attributes", syscalls::tabledb::tdb_set_attributes);
            }
            self.runtime_syscalls_installed = true;
        }
    }
}

/// A syscall context that provides minimal set of functionality
/// required by the upload and download syscalls.
#[repr(C)]
struct SimpleSyscallContextWrapper {
    pub inner: SharemindModuleApi0x1SyscallContext,
    pdpi_info: Option<SharemindModuleApi0x1PdpiInfo>,
}

impl SimpleSyscallContextWrapper {
    pub fn new(
        syscall_wrapper: &SharemindSyscallWrapper,
        pdpi_info: Option<SharemindModuleApi0x1PdpiInfo>,
    ) -> Self {
        Self {
            inner: SharemindModuleApi0x1SyscallContext {
                vm_internal: ptr::null(),
                process_internal: ptr::null_mut(),
                moduleHandle: syscall_wrapper.internal,
                get_pdpi_info: Some(get_pdpi_info),
                // Not required by the upload_vec / download_vec syscalls
                processFacility: None,
                publicAlloc: None,
                publicFree: None,
                publicMemPtrSize: None,
                publicMemPtrData: None,
                allocPrivate: None,
                freePrivate: None,
                reservePrivate: None,
                releasePrivate: None,
            },
            pdpi_info,
        }
    }
}

impl WrappedC<SharemindModuleApi0x1SyscallContext> for SimpleSyscallContextWrapper {}

/// Retrieve Protection Domain Process Instance info by the index of a protection domain
/// in the linking units of the bytecode program executable.
unsafe extern "C" fn get_pdpi_info(
    c: *mut SharemindModuleApi0x1SyscallContext,
    pd_index: u64,
) -> *const SharemindModuleApi0x1PdpiInfo {
    assert!(!c.is_null());

    if pd_index != 0 {
        tracing::error!("Unsupported protection domain index");
        return ptr::null();
    }

    let w = SimpleSyscallContextWrapper::get_mut_wrapper(c);
    if let Some(pdpi_info) = &w.pdpi_info {
        pdpi_info
    } else {
        tracing::error!("No protection domain configured");
        ptr::null()
    }
}
