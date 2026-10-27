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
    cell::RefCell,
    ffi::{c_char, CStr},
    ptr::{null, null_mut},
    rc::Rc,
    slice::from_raw_parts,
};

use crate::ffi::{
    bindings::{
        SharemindNetwork, SharemindNetworkConfiguration, SharemindNetworkError,
        SharemindNetworkError__SHAREMIND_NETWORK_OK, SharemindNodeConfiguration,
        SharemindPdNetworkFacility,
    },
    facility::{network::NetworkFacility, Facility},
    wrapper::{network_configuration::NetworkConfigurationWrapper, WrappedC},
};

#[repr(C)]
pub struct PdNetworkFacilityWrapper {
    inner: SharemindPdNetworkFacility,
    parent: Rc<RefCell<NetworkFacility>>,
    last_error: SharemindNetworkError,
}

impl PdNetworkFacilityWrapper {
    pub fn new(parent: Rc<RefCell<NetworkFacility>>) -> Self {
        Self {
            inner: SharemindPdNetworkFacility {
                last_error: Some(last_error),
                clear_error: Some(clear_error),
                haveServer: Some(have_server),
                init_configuration: Some(init_configuration),
                new_network: Some(new_network),
            },
            parent,
            last_error: SharemindNetworkError__SHAREMIND_NETWORK_OK,
        }
    }

    pub fn total_node_count(&self) -> usize {
        self.parent.borrow().total_node_count()
    }
}

impl WrappedC<SharemindPdNetworkFacility> for PdNetworkFacilityWrapper {}

impl Facility<SharemindPdNetworkFacility> for PdNetworkFacilityWrapper {
    fn name(&self) -> &CStr {
        c"Network"
    }

    fn inner(&mut self) -> *mut SharemindPdNetworkFacility {
        &mut self.inner as *mut _
    }
}

unsafe extern "C" fn last_error(
    facility: *const SharemindPdNetworkFacility,
) -> SharemindNetworkError {
    assert!(!facility.is_null());
    PdNetworkFacilityWrapper::get_wrapper(facility).last_error
}

unsafe extern "C" fn clear_error(facility: *mut SharemindPdNetworkFacility) {
    assert!(!facility.is_null());
    let f = PdNetworkFacilityWrapper::get_mut_wrapper(facility);
    f.last_error = SharemindNetworkError__SHAREMIND_NETWORK_OK;
}

/// # Dev Notes
///
/// In general, should the protocol layer even know about server names or
/// the mapping of logical nodes to physical servers? Maybe this should
/// be the responsibility of the application layer? Room for API improvement?
unsafe extern "C" fn have_server(
    facility: *const SharemindPdNetworkFacility,
    server_name: *const c_char,
    server_number_out: *mut usize,
) -> bool {
    assert!(!facility.is_null());
    assert!(!server_name.is_null());
    assert!(!server_number_out.is_null());
    let f = PdNetworkFacilityWrapper::get_wrapper(facility);
    let server_name_str = CStr::from_ptr(server_name).to_str().unwrap();
    match f.parent.borrow().node_id(server_name_str) {
        Some(nr) => {
            *server_number_out = nr;
            true
        }
        None => false, // Non-existence is not an error.
    }
}

unsafe extern "C" fn init_configuration(
    facility: *mut SharemindPdNetworkFacility,
    confs: *const SharemindNodeConfiguration,
    num_confs: usize,
) -> *const SharemindNetworkConfiguration {
    assert!(!facility.is_null());
    assert!(!confs.is_null());
    let f = PdNetworkFacilityWrapper::get_mut_wrapper(facility);
    match f
        .parent
        .borrow_mut()
        .init_configuration(from_raw_parts(confs, num_confs))
    {
        Ok(conf) => &conf.inner as *const _,
        Err(e) => {
            f.last_error = e;
            null()
        }
    }
}

/// # Notes
///
/// This method exists because the Sharemind Application Server supports the use
/// of multiple Protection Domains in a single deployment. Every PD requires its
/// own independent set of communication channels.
unsafe extern "C" fn new_network(
    facility: *mut SharemindPdNetworkFacility,
    config: *const SharemindNetworkConfiguration,
) -> *mut SharemindNetwork {
    assert!(!facility.is_null());
    assert!(!config.is_null());
    let f = PdNetworkFacilityWrapper::get_mut_wrapper(facility);
    let c = NetworkConfigurationWrapper::get_wrapper(config);
    match f.parent.borrow_mut().new_network(c) {
        Ok(network) => &mut network.inner as *mut _,
        Err(e) => {
            f.last_error = e;
            null_mut()
        }
    }
}
