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

use super::ModuleApiError;
use crate::ffi::facility::{Facility, SetFacilityFn};

use crate::{
    ffi::bindings::{
        SharemindModuleApi, SharemindModuleApiError,
        SharemindModuleApiError_SHAREMIND_MODULE_API_OK, SharemindModuleApi_free,
        SharemindModuleApi_lastErrorString, SharemindModuleApi_new,
        SharemindModuleApi_setModuleFacility, SharemindModuleApi_setPdFacility,
        SharemindModuleApi_setPdpiFacility, SharemindModuleApi_unsetModuleFacility,
        SharemindModuleApi_unsetPdFacility, SharemindModuleApi_unsetPdpiFacility,
    },
    module_api_try,
};
use std::{
    ffi::{c_char, c_void},
    ptr::{null_mut, NonNull},
};

pub struct ModuleApi {
    inner: NonNull<SharemindModuleApi>,
}

impl ModuleApi {
    pub fn new() -> Result<Self, ModuleApiError> {
        let mut error_code = SharemindModuleApiError_SHAREMIND_MODULE_API_OK;
        let mut error_str: *const c_char = null_mut();

        let modapi = NonNull::new(unsafe {
            SharemindModuleApi_new(null_mut(), &mut error_code, &mut error_str)
        })
        .ok_or_else(|| ModuleApiError::with_ffi_reason(error_code, error_str))?;

        let modapi_guard = scopeguard::guard(modapi, |modapi| unsafe {
            SharemindModuleApi_free(modapi.as_ptr())
        });

        module_api_try!(error_code, error_str).map(|_| Self {
            inner: scopeguard::ScopeGuard::into_inner(modapi_guard), // disarm the guard
        })
    }

    pub fn handle(&self) -> *mut SharemindModuleApi {
        self.inner.as_ptr()
    }

    pub fn set_module_facility<T, F>(&self, facility: &mut F) -> Result<(), ModuleApiError>
    where
        F: Facility<T>,
    {
        self.set_facility(facility, SharemindModuleApi_setModuleFacility)
    }

    pub fn unset_module_facility<T, F>(&self, facility: &F) -> bool
    where
        F: Facility<T>,
    {
        unsafe { SharemindModuleApi_unsetModuleFacility(self.handle(), facility.name().as_ptr()) }
    }

    pub fn set_pd_facility<T, F>(&self, facility: &mut F) -> Result<(), ModuleApiError>
    where
        F: Facility<T>,
    {
        self.set_facility(facility, SharemindModuleApi_setPdFacility)
    }

    pub fn unset_pd_facility<T, F>(&self, facility: &F) -> bool
    where
        F: Facility<T>,
    {
        unsafe { SharemindModuleApi_unsetPdFacility(self.handle(), facility.name().as_ptr()) }
    }

    pub fn set_pdpi_facility<T, F>(&self, facility: &mut F) -> Result<(), ModuleApiError>
    where
        F: Facility<T>,
    {
        self.set_facility(facility, SharemindModuleApi_setPdpiFacility)
    }

    pub fn unset_pdpi_facility<T, F>(&self, facility: &F) -> bool
    where
        F: Facility<T>,
    {
        unsafe { SharemindModuleApi_unsetPdpiFacility(self.handle(), facility.name().as_ptr()) }
    }

    fn set_facility<T, F>(
        &self,
        facility: &mut F,
        f: SetFacilityFn<SharemindModuleApi, SharemindModuleApiError>,
    ) -> Result<(), ModuleApiError>
    where
        F: Facility<T>,
    {
        module_api_try!(
            unsafe {
                f(
                    self.handle(),
                    facility.name().as_ptr(),
                    facility.inner() as *mut c_void,
                    null_mut(),
                )
            },
            unsafe { SharemindModuleApi_lastErrorString(self.handle()) }
        )
    }
}

impl Drop for ModuleApi {
    fn drop(&mut self) {
        // we first need to free all the related PDs?
        unsafe { SharemindModuleApi_free(self.handle()) }
    }
}
