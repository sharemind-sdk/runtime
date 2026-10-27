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
    error::Error,
    ffi::{c_char, CStr},
};

use crate::ffi::ctrl::bindings::{
    SharemindControllerModuleApiError, SharemindControllerModuleApiError_toString,
};

#[derive(Debug)]
pub struct ControllerModuleApiError {
    pub error_code: SharemindControllerModuleApiError,
    pub error_str: &'static str,
    pub reason: String, // owned, as C-strings could be invalidated by next module call.
}

impl ControllerModuleApiError {
    pub fn new(error_code: SharemindControllerModuleApiError, reason: String) -> Self {
        Self {
            error_code,
            error_str: unsafe {
                CStr::from_ptr(SharemindControllerModuleApiError_toString(error_code))
                    .to_str()
                    .unwrap()
            },
            reason,
        }
    }

    pub fn with_ffi_reason(
        error_code: SharemindControllerModuleApiError,
        reason: *const c_char,
    ) -> Self {
        Self::new(error_code, unsafe {
            CStr::from_ptr(reason).to_str().unwrap().to_owned()
        })
    }
}

impl Error for ControllerModuleApiError {}

impl std::fmt::Display for ControllerModuleApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "SharemindControllerModuleApiError[{}]: {}; reason: {}",
            self.error_code, self.error_str, self.reason
        )
    }
}

#[macro_export]
macro_rules! controller_module_api_err {
    ($testVal:expr, $errorstringFn:expr) => {
        match ($testVal) {
            $crate::ffi::ctrl::bindings::SharemindControllerModuleApiError_SHAREMIND_CONTROLLER_MODULE_API_OK => Ok(()),
            e => {
                // Expand metavariable outside of unsafe block so that the
                // unsafety checks apply to the macro argument.
                // https://rust-lang.github.io/rust-clippy/master/index.html#macro_metavars_in_unsafe
                let error_str = $errorstringFn;
                Err(ControllerModuleApiError::with_ffi_reason(e, error_str))
            },
        }
    };
}
