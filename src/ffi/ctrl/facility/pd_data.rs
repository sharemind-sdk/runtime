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
    ffi::{c_char, CString},
    slice,
};

use crate::ffi::{facility::Facility, wrapper::WrappedC};

use crate::ffi::ctrl::bindings::{
    SharemindControllerModuleApi0x1NodeConfiguration, SharemindControllerModuleApi0x1PdData,
    SharemindControllerModuleApi0x1PdDataError,
    SharemindControllerModuleApi0x1PdDataError__SHAREMIND_CONTROLLER_MODULE_API_0x1_PD_DATA_INVALID_ARGUMENT,
    SharemindControllerModuleApi0x1PdDataError__SHAREMIND_CONTROLLER_MODULE_API_0x1_PD_DATA_OK,
    SharemindControllerModuleApi0x1PdDataError__SHAREMIND_CONTROLLER_MODULE_API_0x1_PD_DATA_UNKNOWN_ERROR,
};

unsafe extern "C" fn last_error(
    facility: *const SharemindControllerModuleApi0x1PdData,
) -> SharemindControllerModuleApi0x1PdDataError {
    assert!(!facility.is_null());
    PdDataFacility::get_wrapper(facility).last_error()
}

unsafe extern "C" fn last_error_string(
    facility: *const SharemindControllerModuleApi0x1PdData,
) -> *const c_char {
    assert!(!facility.is_null());
    PdDataFacility::get_wrapper(facility).last_error_string()
}

unsafe extern "C" fn clear_error(facility: *mut SharemindControllerModuleApi0x1PdData) {
    assert!(!facility.is_null());
    PdDataFacility::get_mut_wrapper(facility).clear_error()
}

unsafe extern "C" fn register_pd_nodes(
    facility: *mut SharemindControllerModuleApi0x1PdData,
    conf_count: usize,
    confs: *const SharemindControllerModuleApi0x1NodeConfiguration,
) -> bool {
    assert!(!facility.is_null());
    assert!(conf_count > 0);
    assert!(!confs.is_null());
    let conf_slice = unsafe { slice::from_raw_parts(confs, conf_count) };
    PdDataFacility::get_mut_wrapper(facility).register_pd_nodes(conf_slice)
}

unsafe extern "C" fn clear_pd_nodes(facility: *mut SharemindControllerModuleApi0x1PdData) {
    assert!(!facility.is_null());
    PdDataFacility::get_mut_wrapper(facility).clear_pd_nodes()
}

/// Facility used by PD during initialization to register its node configuration.
///
/// This exists because the Sharemind libcontroller client-side library needed
/// a way to learn the number of nodes in the protection domain and how they map
/// onto the physical servers (miners). Libcontroller used this information in
/// the classify/declassify operations to construct the refs/crefs vectors in
/// correct order (the one that PD expects), while keeping the caller view intact.
/// It is mandatory, but feels like a workaround to counter the inability of
/// client-module-api to provide PD node configuration (improvement potential?).
///
/// The facility is kept around in original form to maintain PD module compatibility
/// with the Sharemind MPC platform and also to gather the number of nodes
/// exected by PD, so that correct number of inputs and outputs can be provided.
/// Unlike the libcontroller in Sharemind MPC, the Runtime client does not support
/// arbitrary mapping of logical PC nodes to physical servers (i.e. subsetting,
/// reordering). Instead we assume direct sequential node-to-server mapping.
#[repr(C)]
pub struct PdDataFacility {
    inner: SharemindControllerModuleApi0x1PdData,
    last_error: SharemindControllerModuleApi0x1PdDataError,
    last_error_string: CString,
    node_configurations: Vec<SharemindControllerModuleApi0x1NodeConfiguration>,
    expected_node_count: Option<usize>,
}

impl PdDataFacility {
    pub fn new(expected_node_count: Option<usize>) -> Self {
        Self {
            inner: SharemindControllerModuleApi0x1PdData {
                last_error: Some(last_error),
                last_error_string: Some(last_error_string),
                clear_error: Some(clear_error),
                register_pd_nodes: Some(register_pd_nodes),
                clear_pd_nodes: Some(clear_pd_nodes),
            },
            last_error: SharemindControllerModuleApi0x1PdDataError__SHAREMIND_CONTROLLER_MODULE_API_0x1_PD_DATA_OK,
            last_error_string: CString::from(c""),
            node_configurations: Vec::new(),
            expected_node_count,
        }
    }

    pub fn last_error(&self) -> SharemindControllerModuleApi0x1PdDataError {
        self.last_error
    }

    pub fn last_error_string(&self) -> *const c_char {
        self.last_error_string.as_ptr()
    }

    pub fn clear_error(&mut self) {
        self.last_error = SharemindControllerModuleApi0x1PdDataError__SHAREMIND_CONTROLLER_MODULE_API_0x1_PD_DATA_OK;
        self.last_error_string = CString::from(c"");
    }

    pub fn register_pd_nodes(
        &mut self,
        node_configurations: &[SharemindControllerModuleApi0x1NodeConfiguration],
    ) -> bool {
        // Validate preconditions
        if !self.node_configurations.is_empty() {
            self.last_error = SharemindControllerModuleApi0x1PdDataError__SHAREMIND_CONTROLLER_MODULE_API_0x1_PD_DATA_UNKNOWN_ERROR;
            self.last_error_string = c"Nodes already registered".to_owned();
            return false;
        }

        if node_configurations.is_empty() {
            self.last_error = SharemindControllerModuleApi0x1PdDataError__SHAREMIND_CONTROLLER_MODULE_API_0x1_PD_DATA_INVALID_ARGUMENT;
            self.last_error_string = c"No node configurations provided".to_owned();
            return false;
        }

        // Enforce direct sequential node-to-server mapping (no subsetting, no reordering)
        // Node and server numbers start from 1. All are computing nodes.
        if !node_configurations
            .iter()
            .enumerate()
            .all(|(i, c)| c.node_number == i + 1 && c.miner_number == i + 1 && c.is_computing_node)
        {
            self.last_error = SharemindControllerModuleApi0x1PdDataError__SHAREMIND_CONTROLLER_MODULE_API_0x1_PD_DATA_INVALID_ARGUMENT;
            self.last_error_string = c"Not a direct sequential node-to-server mapping".to_owned();
            return false;
        }

        // If remote servers were configured, then we can validate with their count
        if let Some(expected_node_count) = self.expected_node_count {
            if node_configurations.len() != expected_node_count {
                self.last_error = SharemindControllerModuleApi0x1PdDataError__SHAREMIND_CONTROLLER_MODULE_API_0x1_PD_DATA_INVALID_ARGUMENT;
                self.last_error_string = c"Invalid number of nodes".to_owned();
                return false;
            }

            let last_node_conf = node_configurations
                .last()
                .expect("We checked node configurations are not empty");
            if last_node_conf.miner_number != expected_node_count {
                self.last_error = SharemindControllerModuleApi0x1PdDataError__SHAREMIND_CONTROLLER_MODULE_API_0x1_PD_DATA_INVALID_ARGUMENT;
                self.last_error_string = c"Server (miner) numbers out of bounds".to_owned();
                return false;
            }
        }

        self.node_configurations = node_configurations.to_owned();
        true
    }

    pub fn clear_pd_nodes(&mut self) {
        self.node_configurations.clear();
    }

    pub fn node_configurations(&self) -> &[SharemindControllerModuleApi0x1NodeConfiguration] {
        &self.node_configurations
    }
}

impl WrappedC<SharemindControllerModuleApi0x1PdData> for PdDataFacility {}

impl Facility<SharemindControllerModuleApi0x1PdData> for PdDataFacility {
    fn inner(&mut self) -> *mut SharemindControllerModuleApi0x1PdData {
        &mut self.inner
    }

    fn name(&self) -> &std::ffi::CStr {
        c"PdData"
    }
}
