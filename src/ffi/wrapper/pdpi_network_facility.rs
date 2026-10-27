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

use crate::ffi::{
    bindings::{
        SharemindNetwork, SharemindNetworkConfiguration, SharemindNetworkError,
        SharemindNetworkError__SHAREMIND_NETWORK_OK, SharemindPdpiNetworkFacility,
    },
    facility::{network::NetworkFacility, Facility},
    wrapper::{network_configuration::NetworkConfigurationWrapper, WrappedC},
};

use std::{cell::RefCell, ffi::CStr, ptr::null_mut, rc::Rc};

#[repr(C)]
pub struct PdpiNetworkFacilityWrapper {
    inner: SharemindPdpiNetworkFacility,
    last_error: SharemindNetworkError,
    pub parent: Rc<RefCell<NetworkFacility>>,
}

impl WrappedC<SharemindPdpiNetworkFacility> for PdpiNetworkFacilityWrapper {}

impl Facility<SharemindPdpiNetworkFacility> for PdpiNetworkFacilityWrapper {
    fn name(&self) -> &CStr {
        c"Network"
    }

    fn inner(&mut self) -> *mut SharemindPdpiNetworkFacility {
        &mut self.inner as *mut _
    }
}

unsafe extern "C" fn last_error(
    facility: *const SharemindPdpiNetworkFacility,
) -> SharemindNetworkError {
    assert!(!facility.is_null());
    PdpiNetworkFacilityWrapper::get_wrapper(facility).last_error
}

unsafe extern "C" fn clear_error(facility: *mut SharemindPdpiNetworkFacility) {
    let w = PdpiNetworkFacilityWrapper::get_mut_wrapper(facility);
    w.last_error = SharemindNetworkError__SHAREMIND_NETWORK_OK;
}

unsafe extern "C" fn new_network(
    facility: *mut SharemindPdpiNetworkFacility,
    config: *const SharemindNetworkConfiguration,
) -> *mut SharemindNetwork {
    assert!(!facility.is_null());
    assert!(!config.is_null());
    let w = PdpiNetworkFacilityWrapper::get_mut_wrapper(facility);
    let c = NetworkConfigurationWrapper::get_wrapper(config);
    match w.parent.borrow_mut().new_network(c) {
        Ok(network) => &mut network.inner as *mut _,
        Err(e) => {
            w.last_error = e;
            null_mut()
        }
    }
}

impl PdpiNetworkFacilityWrapper {
    pub fn new(network_facility: Rc<RefCell<NetworkFacility>>) -> Self {
        Self {
            inner: SharemindPdpiNetworkFacility {
                last_error: Some(last_error),
                clear_error: Some(clear_error),
                new_network: Some(new_network),
            },
            parent: network_facility,
            last_error: SharemindNetworkError__SHAREMIND_NETWORK_OK,
        }
    }
}
