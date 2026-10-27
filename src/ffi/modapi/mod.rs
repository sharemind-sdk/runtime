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

pub mod module;
pub mod module_api;
pub mod pd;
pub mod pdpi;

use std::{
    error::Error,
    ffi::{c_char, CStr},
    fmt,
};

pub use module::Module;
pub use module_api::ModuleApi;
pub use pd::Pd;

use crate::ffi::bindings::{
    SharemindModuleApiError_SHAREMIND_MODULE_API_GENERAL_ERROR,
    SharemindModuleApiError_SHAREMIND_MODULE_API_INVALID_CALL, SharemindModuleApiError_toString,
};

use super::bindings::SharemindModuleApiError;

#[derive(Debug)]
pub struct ModuleApiError {
    error_code: SharemindModuleApiError,
    error_str: &'static str,
    reason: String, // owned, as C-strings could be invalidated by next module call.
}

impl ModuleApiError {
    pub fn new(error_code: SharemindModuleApiError, reason: String) -> Self {
        Self {
            error_code,
            error_str: unsafe {
                CStr::from_ptr(SharemindModuleApiError_toString(error_code))
                    .to_str()
                    .unwrap()
            },
            reason,
        }
    }

    pub fn with_ffi_reason(error_code: SharemindModuleApiError, reason: *const c_char) -> Self {
        Self::new(error_code, unsafe {
            CStr::from_ptr(reason).to_str().unwrap().to_owned()
        })
    }

    pub fn invalid_call(reason: String) -> Self {
        Self::new(
            SharemindModuleApiError_SHAREMIND_MODULE_API_INVALID_CALL,
            reason,
        )
    }

    pub fn general_error(reason: String) -> Self {
        Self::new(
            SharemindModuleApiError_SHAREMIND_MODULE_API_GENERAL_ERROR,
            reason,
        )
    }
}

impl Error for ModuleApiError {}

impl fmt::Display for ModuleApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SharemindModuleApiError[{}]: {}; reason: {}",
            self.error_code, self.error_str, self.reason
        )
    }
}

#[macro_export]
macro_rules! module_api_try {
    ($testVal:expr, $errorstringFn:expr) => {
        match ($testVal) {
            $crate::ffi::bindings::SharemindModuleApiError_SHAREMIND_MODULE_API_OK => Ok(()),
            e => {
                // Expand metavariable outside of unsafe block so that the
                // unsafety checks apply to the macro argument.
                // https://rust-lang.github.io/rust-clippy/master/index.html#macro_metavars_in_unsafe
                let error_str = $errorstringFn;
                Err(ModuleApiError::with_ffi_reason(e, error_str))
            },
        }
    };
}
