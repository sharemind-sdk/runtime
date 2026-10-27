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

use std::ffi::{c_void, CStr};

use crate::{
    ffi::{
        bindings::{
            SharemindDataStore, SharemindDataStoreFactory, SharemindModuleApi0x1SyscallContext,
        },
        vm::syscall::error::{SyscallError, SyscallErrorKind},
    },
    syserr,
};

use super::error::ModuleApiFinderError;

impl SharemindDataStoreFactory {
    fn get_data_store(
        &mut self,
        name: &CStr,
    ) -> Result<&mut SharemindDataStore, ModuleApiFinderError> {
        const context: &str = "Data store";
        let get_datastore_fn = self
            .get_datastore
            .ok_or(ModuleApiFinderError::NullFunctionPointer(context))?;
        let data_store_ptr = unsafe { get_datastore_fn(self, name.as_ptr()) };
        unsafe { data_store_ptr.as_mut() }
            .ok_or_else(|| ModuleApiFinderError::NotFound(context, name.to_owned()))
    }
}

impl SharemindDataStore {
    pub fn clear(&mut self) {
        let clear_fn = self
            .clear
            .expect("Function 'SharemindDataStore::clear' shall be set");
        unsafe { clear_fn(self) }
    }

    pub fn get<T>(&mut self, key: &CStr) -> Option<&mut T> {
        let get_fn = self
            .get
            .expect("Function 'SharemindDataStore::get' shall be set");
        unsafe {
            let val_ptr = get_fn(self, key.as_ptr()) as *mut T;
            val_ptr.as_mut()
        }
    }

    /// # Safety
    /// This function is unsafe because it may dereference the value pointer.
    pub unsafe fn set(
        &mut self,
        key: &CStr,
        value: *mut c_void,
        deleter: Option<unsafe extern "C" fn(*mut c_void)>,
    ) -> bool {
        let set_fn = self
            .set
            .expect("Function 'SharemindDataStore::set' shall be set");
        unsafe { set_fn(self, key.as_ptr(), value, deleter) }
    }

    pub fn remove(&mut self, key: &CStr) -> bool {
        let remove_fn = self
            .remove
            .expect("Function 'SharemindDataStore::remove' shall be set");
        unsafe { remove_fn(self, key.as_ptr()) }
    }
}

#[allow(dead_code)]
pub fn get_data_store<'a>(
    ctx: &'a SharemindModuleApi0x1SyscallContext,
    ds_name: &'a CStr,
) -> Result<&'a mut SharemindDataStore, SyscallError> {
    let ds_factory = ctx.get_process_facility::<SharemindDataStoreFactory>(c"DataStoreFactory")?;

    ds_factory
        .get_data_store(ds_name)
        .map_err(|e| syserr!(SyscallErrorKind::GeneralError, e.to_string()))
}
