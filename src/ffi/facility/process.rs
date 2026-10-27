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

use std::ffi::CStr;

use super::Facility;

use crate::ffi::bindings::SharemindProcessFacility;

unsafe extern "C" fn printable_id(
    facility: *const SharemindProcessFacility,
) -> *const ::std::os::raw::c_char {
    assert!(!facility.is_null());
    c"1".as_ptr()
}

pub struct ProcessFacility {
    inner: SharemindProcessFacility,
}

impl ProcessFacility {
    pub fn new() -> Self {
        Self {
            inner: SharemindProcessFacility {
                printableId: Some(printable_id),
                localId: None,
                localIdSize: None,
                globalId: None,
                globalIdSize: None,
                clientAuth: None,
                programName: None,
            },
        }
    }
}

impl Default for ProcessFacility {
    fn default() -> Self {
        Self::new()
    }
}

impl Facility<SharemindProcessFacility> for ProcessFacility {
    fn inner(&mut self) -> *mut SharemindProcessFacility {
        &mut self.inner as *mut _
    }

    fn name(&self) -> &CStr {
        c"ProcessFacility"
    }
}

impl Drop for ProcessFacility {
    fn drop(&mut self) {}
}
