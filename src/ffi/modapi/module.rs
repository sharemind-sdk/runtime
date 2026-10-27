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
    collections::{hash_map::Entry, HashMap},
    ffi::{CStr, CString},
    path::Path,
    ptr::{null, null_mut, NonNull},
    rc::Rc,
};

use crate::ffi::{
    bindings::{
        SharemindCodeBlock, SharemindModule, SharemindModuleApi0x1CReference,
        SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_OK, SharemindModuleApi0x1Reference,
        SharemindModuleApi_lastError, SharemindModuleApi_lastErrorString,
        SharemindModuleApi_newModule, SharemindModule_deinit, SharemindModule_free,
        SharemindModule_init, SharemindModule_isInitialized, SharemindModule_lastErrorString,
        SharemindModule_numSyscalls, SharemindModule_syscall, SharemindModule_syscallWrapper,
        SharemindSyscallContext, SharemindSyscallWrapper, SharemindSyscall_signature,
        SharemindSyscall_wrapper,
    },
    path::PathToCStringExt,
    vm::syscall::error::{SyscallError, SyscallErrorKind},
};

use crate::module_api_try;

use super::{module_api::ModuleApi, ModuleApiError};

pub struct Module {
    inner: NonNull<SharemindModule>,
    _module_api: Rc<ModuleApi>,
}

impl Module {
    pub fn new(
        module_api: Rc<ModuleApi>,
        module_path: &Path,
        conf_path: Option<&Path>,
    ) -> Result<Self, ModuleApiError> {
        let module_path_c = module_path.to_cstring().map_err(|e| {
            ModuleApiError::invalid_call(format!(
                "Invalid module path '{}': {e}",
                module_path.display()
            ))
        })?;

        // Keep CString alive to get a pointer to it.
        let conf_path_c = conf_path
            .map(|p| {
                p.to_cstring().map_err(|e| {
                    ModuleApiError::invalid_call(format!(
                        "Invalid config path '{}': {e}",
                        p.display()
                    ))
                })
            })
            .transpose()?;
        let conf_path_c_ptr = conf_path_c.as_ref().map_or(null(), |cstr| cstr.as_ptr());

        let module = NonNull::new(unsafe {
            SharemindModuleApi_newModule(
                module_api.handle(),
                module_path_c.as_ptr(),
                conf_path_c_ptr,
            )
        })
        .ok_or_else(|| {
            ModuleApiError::with_ffi_reason(
                unsafe { SharemindModuleApi_lastError(module_api.handle()) },
                unsafe { SharemindModuleApi_lastErrorString(module_api.handle()) },
            )
        })?;

        let module_guard = scopeguard::guard(module, |module| unsafe {
            SharemindModule_free(module.as_ptr())
        });

        module_api_try!(
            unsafe { SharemindModuleApi_lastError(module_api.handle()) },
            unsafe { SharemindModuleApi_lastErrorString(module_api.handle()) }
        )?;

        unsafe {
            module_api_try!(
                SharemindModule_init(module_guard.as_ptr()),
                SharemindModule_lastErrorString(module_guard.as_ptr())
            )
        }
        .map(|_| Self {
            inner: scopeguard::ScopeGuard::into_inner(module_guard), // disarm the guard
            _module_api: module_api,
        })
    }

    pub fn handle(&self) -> *mut SharemindModule {
        self.inner.as_ptr()
    }

    pub fn find_syscall(&self, signature: &str) -> SharemindSyscallWrapper {
        let signature_cstr = CString::new(signature).unwrap();
        unsafe {
            SharemindModule_syscallWrapper(self.handle() as *const _, signature_cstr.as_ptr())
        }
    }

    pub fn cache_syscalls(
        &self,
        cache: &mut HashMap<String, SharemindSyscallWrapper>,
    ) -> Result<(), String> {
        let num_syscalls = unsafe { SharemindModule_numSyscalls(self.handle()) };
        for syscall_idx in 0..num_syscalls {
            let syscall = unsafe { SharemindModule_syscall(self.handle(), syscall_idx) };
            let syscall_signature = unsafe {
                CStr::from_ptr(SharemindSyscall_signature(syscall) as *mut _)
                    .to_str()
                    .map_err(|_| "Invalid UTF-8 in module syscall signature")?
            };
            let syscall_wrapper = unsafe { SharemindSyscall_wrapper(syscall) };

            match cache.entry(syscall_signature.to_owned()) {
                Entry::Occupied(_) => {
                    return Err(format!("Duplicate syscall in cache: {syscall_signature}"))
                }
                Entry::Vacant(vacant_entry) => {
                    vacant_entry.insert(syscall_wrapper);
                }
            }
        }
        Ok(())
    }

    pub fn invoke_syscall(
        syscall_wrapper: &SharemindSyscallWrapper,
        args: Option<&mut [SharemindCodeBlock]>,
        refs: Option<&[SharemindModuleApi0x1Reference]>,
        crefs: Option<&[SharemindModuleApi0x1CReference]>,
        return_value: Option<&mut SharemindCodeBlock>,
        context: &mut SharemindSyscallContext,
    ) -> Result<(), SyscallError> {
        let syscall_callable = syscall_wrapper.callable.ok_or_else(|| {
            SyscallError::new(SyscallErrorKind::InvalidCall)
                .with_reason("Syscall callable pointer not set")
        })?;

        let (args_ptr, num_args) =
            args.map_or((null_mut(), 0), |slice| (slice.as_mut_ptr(), slice.len()));
        let refs_ptr = refs.map_or(null(), |slice| slice.as_ptr());
        let crefs_ptr = crefs.map_or(null(), |slice| slice.as_ptr());
        let retval_ptr = return_value.map_or(null_mut(), |value| value);

        let syscall_status = unsafe {
            syscall_callable(args_ptr, num_args, refs_ptr, crefs_ptr, retval_ptr, context)
        };

        match syscall_status {
            SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_OK => Ok(()),
            error_code => Err(SyscallError::new(SyscallErrorKind::from(error_code))),
        }
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        unsafe {
            if SharemindModule_isInitialized(self.handle()) {
                SharemindModule_deinit(self.handle());
            }
            SharemindModule_free(self.handle())
        }
    }
}
