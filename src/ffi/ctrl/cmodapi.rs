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

pub mod data_type;
pub mod error;
pub mod module;
pub mod pd;

use std::{
    ffi::{c_char, c_void},
    ptr::{null_mut, NonNull},
};

use crate::ffi::facility::{Facility, SetFacilityFn};
use error::ControllerModuleApiError;

use crate::controller_module_api_err;

use super::bindings::{
    SharemindControllerModuleApi, SharemindControllerModuleApiError,
    SharemindControllerModuleApiError_SHAREMIND_CONTROLLER_MODULE_API_OK,
    SharemindControllerModuleApi_free, SharemindControllerModuleApi_lastErrorString,
    SharemindControllerModuleApi_new, SharemindControllerModuleApi_setModuleFacility,
    SharemindControllerModuleApi_setPdFacility, SharemindControllerModuleApi_unsetModuleFacility,
    SharemindControllerModuleApi_unsetPdFacility,
};

pub struct ControllerModuleApi {
    inner: NonNull<SharemindControllerModuleApi>,
}

impl ControllerModuleApi {
    pub fn new() -> Result<Self, ControllerModuleApiError> {
        let mut error_code = SharemindControllerModuleApiError_SHAREMIND_CONTROLLER_MODULE_API_OK;
        let mut error_str: *const c_char = null_mut();

        let cmodapi = NonNull::new(unsafe {
            SharemindControllerModuleApi_new(&mut error_code, &mut error_str)
        })
        .ok_or_else(|| ControllerModuleApiError::with_ffi_reason(error_code, error_str))?;

        let cmodapi_guard = scopeguard::guard(cmodapi, |cmodapi| unsafe {
            SharemindControllerModuleApi_free(cmodapi.as_ptr())
        });

        controller_module_api_err!(error_code, error_str).map(|_| Self {
            inner: scopeguard::ScopeGuard::into_inner(cmodapi_guard), // disarm the guard
        })
    }

    pub fn handle(&self) -> *mut SharemindControllerModuleApi {
        self.inner.as_ptr()
    }

    pub fn set_module_facility<T, F>(
        &self,
        facility: &mut F,
    ) -> Result<(), ControllerModuleApiError>
    where
        F: Facility<T>,
    {
        self.set_facility(facility, SharemindControllerModuleApi_setModuleFacility)
    }

    pub fn unset_module_facility<T, F>(&self, facility: &F) -> bool
    where
        F: Facility<T>,
    {
        unsafe {
            SharemindControllerModuleApi_unsetModuleFacility(
                self.handle(),
                facility.name().as_ptr(),
            )
        }
    }

    #[allow(dead_code)]
    pub fn set_pd_facility<T, F>(&self, facility: &mut F) -> Result<(), ControllerModuleApiError>
    where
        F: Facility<T>,
    {
        self.set_facility(facility, SharemindControllerModuleApi_setPdFacility)
    }

    #[allow(dead_code)]
    pub fn unset_pd_facility<T, F>(&self, facility: &F) -> bool
    where
        F: Facility<T>,
    {
        unsafe {
            SharemindControllerModuleApi_unsetPdFacility(self.handle(), facility.name().as_ptr())
        }
    }

    fn set_facility<T, F>(
        &self,
        facility: &mut F,
        f: SetFacilityFn<SharemindControllerModuleApi, SharemindControllerModuleApiError>,
    ) -> Result<(), ControllerModuleApiError>
    where
        F: Facility<T>,
    {
        controller_module_api_err!(
            unsafe {
                f(
                    self.handle(),
                    facility.name().as_ptr(),
                    facility.inner() as *mut c_void,
                    null_mut(),
                )
            },
            unsafe { SharemindControllerModuleApi_lastErrorString(self.handle()) }
        )
    }
}

impl Drop for ControllerModuleApi {
    fn drop(&mut self) {
        unsafe { SharemindControllerModuleApi_free(self.handle()) };
    }
}
