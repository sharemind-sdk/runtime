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

pub mod syscall;

use std::{
    collections::HashMap,
    ffi::{c_char, CStr, CString},
    ptr,
};

use crate::ffi::{facility::Facility, wrapper::WrappedC};
use libc::c_void;

use crate::program_io::{ProgramArgumentMap, ProgramResultMap};

use super::{
    bindings::{FinderMut, RustProcess, SharemindModuleApi0x1PdpiInfo},
    modapi::pdpi::Pdpi,
};

#[derive(Clone)]
pub struct ProcessFacilityMap {
    process_facilities: HashMap<CString, *mut c_void>,
}

impl ProcessFacilityMap {
    pub fn new() -> Self {
        Self {
            process_facilities: HashMap::new(),
        }
    }

    pub fn set_facility<T, F>(&mut self, facility: &mut F)
    where
        F: Facility<T>,
    {
        self.process_facilities
            .insert(facility.name().to_owned(), facility.inner() as *mut c_void);
    }

    pub fn get_facility(&self, name: &CStr) -> *mut c_void {
        match self.process_facilities.get(name) {
            Some(val) => *val,
            None => ptr::null_mut(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.process_facilities.is_empty()
    }
}

impl Default for ProcessFacilityMap {
    fn default() -> Self {
        Self::new()
    }
}

unsafe extern "C" fn find_process_facility(
    finder: *const FinderMut,
    name: *const c_char,
) -> *mut c_void {
    assert!(!finder.is_null());
    assert!(!name.is_null());
    ProcessFacilityFinderWrapper::get_wrapper(finder).get_facility(CStr::from_ptr(name))
}

#[repr(C)]
pub(crate) struct ProcessFacilityFinderWrapper {
    pub inner: FinderMut,
    process_facilities: ProcessFacilityMap,
}

impl WrappedC<FinderMut> for ProcessFacilityFinderWrapper {}

impl ProcessFacilityFinderWrapper {
    pub fn new(process_facilities: ProcessFacilityMap) -> Self {
        Self {
            inner: FinderMut {
                find: Some(find_process_facility),
            },
            process_facilities,
        }
    }

    pub fn get_facility(&self, name: &CStr) -> *mut c_void {
        self.process_facilities.get_facility(name)
    }
}

/// Retrieve Protection Domain Process Instance info by the index of a protection domain
/// in the linking units of the bytecode program executable.
unsafe extern "C" fn get_pdpi_info(
    facility: *mut RustProcess,
    pd_index: u64,
) -> *const SharemindModuleApi0x1PdpiInfo {
    assert!(!facility.is_null());

    if pd_index != 0 {
        tracing::error!("Unsupported protection domain index");
        return ptr::null();
    }

    if let Some(pdpi_info) = RustProcessWrapper::get_mut_wrapper(facility).get_pdpi_info() {
        pdpi_info
    } else {
        tracing::error!("No protection domain configured");
        ptr::null()
    }
}

/// This struct has a similar role to `ProcessWrapper` in the Sharemind MPC Server.
///
/// # Notes
///
/// ## Comparison to application server implementation
///
/// The server utilizes C++ inheritance to cast from the process facility to the
/// process wrapper, but in Rust we have to use a separate facility for the
/// process wrapper.
///
/// # Limitations
///
/// ## Number of simultaneous protection domains
///
/// Current implementation limits the number of protection domains usable at
/// the same time to at most one. Refer to `SharemindRuntime` documentation for
/// more details.
#[repr(C)]
pub(crate) struct RustProcessWrapper {
    inner: RustProcess,
    pdpi_info: Option<SharemindModuleApi0x1PdpiInfo>,
    pub program_arguments: ProgramArgumentMap,
    pub program_results: ProgramResultMap,
}

impl WrappedC<RustProcess> for RustProcessWrapper {}

impl RustProcessWrapper {
    pub fn new(pdpi: Option<&Pdpi>, program_arguments: ProgramArgumentMap) -> Self {
        Self {
            inner: RustProcess {
                get_pdpi_info: Some(get_pdpi_info),
            },
            pdpi_info: pdpi.map(|p| p.pdpi_info()),
            program_arguments,
            program_results: ProgramResultMap::new(),
        }
    }

    fn get_pdpi_info(&self) -> Option<&SharemindModuleApi0x1PdpiInfo> {
        self.pdpi_info.as_ref()
    }
}

impl Facility<RustProcess> for RustProcessWrapper {
    fn inner(&mut self) -> *mut RustProcess {
        &mut self.inner as *mut _
    }

    fn name(&self) -> &CStr {
        c"RustProcess"
    }
}
