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

use std::{error::Error, ffi::CString, path::Path, ptr::NonNull, rc::Rc};

use crate::{
    controller_module_api_err,
    ffi::ctrl::bindings::{
        SharemindControllerModule, SharemindControllerModuleApi_lastError,
        SharemindControllerModuleApi_lastErrorString, SharemindControllerModuleApi_newModule,
        SharemindControllerModule_deinit, SharemindControllerModule_findDataType,
        SharemindControllerModule_findPdk, SharemindControllerModule_free,
        SharemindControllerModule_init, SharemindControllerModule_isInitialized,
        SharemindControllerModule_lastErrorString, SharemindControllerPdk,
    },
};

use super::{data_type::ControllerDataType, ControllerModuleApi, ControllerModuleApiError};

/// A Sharemind controller module.
///
/// This structure uses cmodapi FFI bindings to load the controller module
/// and provides access to the Protection Domain Kinds and their data types
/// defined in the module.
pub struct ControllerModule {
    inner: NonNull<SharemindControllerModule>,
    _module_api: Rc<ControllerModuleApi>,
}

impl ControllerModule {
    pub fn new(
        module_api: Rc<ControllerModuleApi>,
        module_path: &Path,
    ) -> Result<Self, Box<dyn Error>> {
        let module_path_c = CString::new(
            module_path
                .to_str()
                .ok_or("module_path has invalid encoding")?,
        )?;

        let module = NonNull::new(unsafe {
            SharemindControllerModuleApi_newModule(module_api.handle(), module_path_c.as_ptr())
        })
        .ok_or_else(|| {
            ControllerModuleApiError::with_ffi_reason(
                unsafe { SharemindControllerModuleApi_lastError(module_api.handle()) },
                unsafe { SharemindControllerModuleApi_lastErrorString(module_api.handle()) },
            )
        })?;

        let module_guard = scopeguard::guard(module, |module| unsafe {
            SharemindControllerModule_free(module.as_ptr())
        });

        controller_module_api_err!(
            unsafe { SharemindControllerModuleApi_lastError(module_api.handle()) },
            unsafe { SharemindControllerModuleApi_lastErrorString(module_api.handle()) }
        )?;

        controller_module_api_err!(
            unsafe { SharemindControllerModule_init(module_guard.as_ptr()) },
            unsafe { SharemindControllerModule_lastErrorString(module_guard.as_ptr()) }
        )?;

        Ok(Self {
            inner: scopeguard::ScopeGuard::into_inner(module_guard), // disarm the guard
            _module_api: module_api,
        })
    }

    fn handle(&self) -> *mut SharemindControllerModule {
        self.inner.as_ptr()
    }

    /// TODO: Refactor to fix the `clippy::mut_from_ref` lint.
    /// See comment for SharemindModuleApi0x1SyscallContext::get_process_facility()
    /// in runtime/src/ffi/vm/syscall/interface.rs
    #[allow(clippy::mut_from_ref)]
    pub fn find_pdk(&self, name: &str) -> Option<&mut SharemindControllerPdk> {
        let name_c = CString::new(name).unwrap();
        unsafe { SharemindControllerModule_findPdk(self.handle(), name_c.as_ptr()).as_mut() }
    }

    pub fn find_datatype(&'_ self, signature: &str) -> Option<ControllerDataType<'_>> {
        let signature_c = CString::new(signature).unwrap();
        let dt = NonNull::new(unsafe {
            SharemindControllerModule_findDataType(self.handle(), signature_c.as_ptr())
        });
        dt.map(|p| ControllerDataType::from_ptr(p, self))
    }
}

impl Drop for ControllerModule {
    fn drop(&mut self) {
        unsafe {
            if SharemindControllerModule_isInitialized(self.handle()) {
                SharemindControllerModule_deinit(self.handle());
            }
            SharemindControllerModule_free(self.handle())
        }
    }
}
