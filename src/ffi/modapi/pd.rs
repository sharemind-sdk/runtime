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

use crate::ffi::path::PathToCStringExt;

use super::module::Module;
use crate::ffi::bindings::*;
use crate::modapi::ModuleApiError;
use std::ffi::{CStr, CString};
use std::path::Path;
use std::ptr::NonNull;
use std::rc::Rc;

use crate::module_api_try;

pub struct Pd {
    inner: NonNull<SharemindPd>,
    module: Rc<Module>,
}

impl Pd {
    pub fn new(
        module: Rc<Module>,
        pdk_name: &str,
        config_path: &Path,
    ) -> Result<Self, ModuleApiError> {
        let pdk_name_c = CString::new(pdk_name)
            .map_err(|e| ModuleApiError::invalid_call(format!("Invalid PDK name: {e}")))?;
        let config_path_c = config_path.to_cstring().map_err(|e| {
            ModuleApiError::invalid_call(format!(
                "Invalid config path '{}': {e}",
                config_path.display()
            ))
        })?;

        let pdk =
            NonNull::new(unsafe { SharemindModule_findPdk(module.handle(), pdk_name_c.as_ptr()) })
                .ok_or_else(|| ModuleApiError::general_error("PDK not found".into()))?;

        let pd = NonNull::new(unsafe {
            SharemindPdk_newPd(
                pdk.as_ptr(),
                pdk_name_c.as_ptr(), // TODO: should we use a separate PD name instead?
                config_path_c.as_ptr(),
            )
        })
        .ok_or_else(|| {
            ModuleApiError::with_ffi_reason(
                unsafe { SharemindPdk_lastError(pdk.as_ptr()) },
                unsafe { SharemindPdk_lastErrorString(pdk.as_ptr()) },
            )
        })?;

        let pd_guard = scopeguard::guard(pd, |pd| unsafe { SharemindPd_free(pd.as_ptr()) });

        unsafe {
            module_api_try!(
                SharemindPdk_lastError(pdk.as_ptr()),
                SharemindPdk_lastErrorString(pdk.as_ptr())
            )
        }
        .map(|_| Self {
            inner: scopeguard::ScopeGuard::into_inner(pd_guard), // disarm the guard
            module,
        })
    }

    pub fn handle(&self) -> *mut SharemindPd {
        self.inner.as_ptr()
    }

    pub fn start(&self) -> Result<(), ModuleApiError> {
        unsafe {
            module_api_try!(
                SharemindPd_start(self.handle()),
                SharemindPd_lastErrorString(self.handle())
            )
        }
    }

    pub fn pdk_index(&self) -> usize {
        unsafe {
            let pdk_ptr = SharemindPd_pdk(self.handle());
            assert!(!pdk_ptr.is_null());
            SharemindPdk_index(pdk_ptr)
        }
    }

    pub fn pdk_name(&self) -> &str {
        unsafe {
            let pdk_ptr = SharemindPd_pdk(self.handle());
            assert!(!pdk_ptr.is_null());
            let name_ptr = SharemindPdk_name(pdk_ptr);
            assert!(!name_ptr.is_null());
            CStr::from_ptr(name_ptr).to_str().unwrap()
        }
    }

    pub fn module(&self) -> Rc<Module> {
        self.module.clone()
    }
}

impl Drop for Pd {
    fn drop(&mut self) {
        unsafe {
            if SharemindPd_isStarted(self.handle()) {
                SharemindPd_stop(self.handle());
            }
            SharemindPd_free(self.handle())
        }
    }
}
