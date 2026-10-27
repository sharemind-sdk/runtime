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

use std::{
    collections::HashMap,
    ffi::{c_char, CStr},
    ptr::{self, null_mut},
};

use libc::c_void;

use crate::ffi::{
    bindings::{FinderConst, SharemindModuleApi0x1Syscall, SharemindSyscallWrapper},
    facility::Facility,
    modapi::Module,
    wrapper::WrappedC,
};

#[derive(Debug, Default)]
pub struct SyscallCache {
    map: HashMap<String, SharemindSyscallWrapper>,
}

impl SyscallCache {
    pub fn cache_module_syscalls(&mut self, module: &Module) {
        module.cache_syscalls(&mut self.map).unwrap();
    }

    pub fn cache_raw_syscall(&mut self, signature: &str, syscall: SharemindModuleApi0x1Syscall) {
        self.map.insert(
            signature.to_owned(),
            SharemindSyscallWrapper {
                callable: syscall,
                internal: null_mut(),
            },
        );
    }

    pub fn find_syscall(&self, name: &str) -> Option<&SharemindSyscallWrapper> {
        #[cfg(not(feature = "profiler"))]
        {
            if name.starts_with("ProcessProfiler_") {
                tracing::error!(
                    "Tried to use a profiling syscall.\
                Profiler is a feature that should be enabled compile-time, but it was not."
                )
            }
        }
        self.map.get(name)
    }
}

unsafe extern "C" fn find_syscall(
    finder: *const FinderConst,
    name: *const c_char,
) -> *const c_void {
    assert!(!finder.is_null());
    assert!(!name.is_null());

    let name_str = CStr::from_ptr(name).to_str().expect("non utf8 name");
    tracing::debug!("Syscall lookup: {name_str}");

    match SyscallFinderWrapper::get_wrapper(finder)
        .syscall_cache
        .find_syscall(name_str)
    {
        Some(syscall_wrapper) => syscall_wrapper as *const _ as *const c_void,
        None => {
            tracing::error!("Syscall not found: {name_str}");
            ptr::null()
        }
    }
}

#[repr(C)]
pub struct SyscallFinderWrapper<'a> {
    pub inner: FinderConst,
    pub syscall_cache: &'a SyscallCache,
}

impl WrappedC<FinderConst> for SyscallFinderWrapper<'_> {}

impl<'a> SyscallFinderWrapper<'a> {
    pub fn new(syscall_cache: &'a SyscallCache) -> Self {
        Self {
            inner: FinderConst {
                find: Some(find_syscall),
            },
            syscall_cache,
        }
    }
}

impl Facility<FinderConst> for SyscallFinderWrapper<'_> {
    fn inner(&mut self) -> *mut FinderConst {
        &mut self.inner as *mut _
    }

    fn name(&self) -> &CStr {
        c"SyscallFinder"
    }
}
