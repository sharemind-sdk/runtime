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

use crate::ffi::bindings::{
    SharemindCoreRandomEngineKind__SHAREMIND_RANDOM_CHACHA20,
    SharemindRandomEngineBufferingMode__SHAREMIND_RANDOM_BUFFERING_THREAD,
    SharemindRandomEngineConf, SharemindRandomFacility, SharemindRandomFacility_free,
    SharemindRandomFacility_new,
};

use super::Facility;

// Backed by Sharemind's librandom
pub struct RandomFacility {
    inner: *mut SharemindRandomFacility,
    _configuration: SharemindRandomEngineConf,
}

impl RandomFacility {
    pub fn new() -> Self {
        let configuration = SharemindRandomEngineConf {
            coreEngine: SharemindCoreRandomEngineKind__SHAREMIND_RANDOM_CHACHA20,
            bufferMode: SharemindRandomEngineBufferingMode__SHAREMIND_RANDOM_BUFFERING_THREAD,
            bufferSize: 1048576,
        };
        let inner = unsafe { SharemindRandomFacility_new(&configuration) };
        Self {
            inner,
            _configuration: configuration,
        }
    }
}

impl Default for RandomFacility {
    fn default() -> Self {
        Self::new()
    }
}

impl Facility<SharemindRandomFacility> for RandomFacility {
    fn inner(&mut self) -> *mut SharemindRandomFacility {
        self.inner
    }

    fn name(&self) -> &CStr {
        c"Random"
    }
}

impl Drop for RandomFacility {
    fn drop(&mut self) {
        unsafe { SharemindRandomFacility_free(self.inner) };
    }
}
