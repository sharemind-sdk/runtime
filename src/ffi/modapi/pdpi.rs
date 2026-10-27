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

use std::{ptr::NonNull, rc::Rc};

use crate::{
    ffi::{
        bindings::{
            SharemindModuleApi0x1PdpiInfo, SharemindModule_handle, SharemindPd_handle,
            SharemindPd_lastError, SharemindPd_lastErrorString, SharemindPd_newPdpi, SharemindPdpi,
            SharemindPdpi_free, SharemindPdpi_handle, SharemindPdpi_isStarted,
            SharemindPdpi_lastErrorString, SharemindPdpi_start, SharemindPdpi_stop,
        },
        modapi::ModuleApiError,
    },
    module_api_try,
};

use super::Pd;

pub struct Pdpi {
    inner: NonNull<SharemindPdpi>,
    pd: Rc<Pd>,
}

impl Pdpi {
    pub fn new(pd: Rc<Pd>) -> Result<Self, ModuleApiError> {
        let pdpi = NonNull::new(unsafe { SharemindPd_newPdpi(pd.handle()) }).ok_or_else(|| {
            let error_code = unsafe { SharemindPd_lastError(pd.handle()) };
            let error_str = unsafe { SharemindPd_lastErrorString(pd.handle()) };
            ModuleApiError::with_ffi_reason(error_code, error_str)
        })?;

        let pdpi_guard =
            scopeguard::guard(pdpi, |pdpi| unsafe { SharemindPdpi_free(pdpi.as_ptr()) });

        unsafe {
            module_api_try!(
                SharemindPd_lastError(pd.handle()),
                SharemindPd_lastErrorString(pd.handle())
            )
        }
        .map(|_| Self {
            inner: scopeguard::ScopeGuard::into_inner(pdpi_guard), // disarm the guard
            pd,
        })
    }

    fn handle(&self) -> *mut SharemindPdpi {
        self.inner.as_ptr()
    }

    pub fn start(&self) -> Result<(), ModuleApiError> {
        unsafe {
            module_api_try!(
                SharemindPdpi_start(self.handle()),
                SharemindPdpi_lastErrorString(self.handle())
            )
        }
    }

    pub fn pdpi_info(&self) -> SharemindModuleApi0x1PdpiInfo {
        SharemindModuleApi0x1PdpiInfo {
            pdpiHandle: unsafe { SharemindPdpi_handle(self.handle()) },
            pdHandle: unsafe { SharemindPd_handle(self.pd.handle()) },
            pdkIndex: self.pd.pdk_index(),
            moduleHandle: unsafe { SharemindModule_handle(self.pd.module().handle()) },
        }
    }
}

impl Drop for Pdpi {
    fn drop(&mut self) {
        unsafe {
            if SharemindPdpi_isStarted(self.handle()) {
                SharemindPdpi_stop(self.handle());
            }
            SharemindPdpi_free(self.handle())
        }
    }
}
