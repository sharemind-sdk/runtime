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

use crate::ffi::{bindings::SharemindNetworkConfiguration, wrapper::WrappedC};

#[repr(C)]
pub struct NetworkConfigurationWrapper {
    pub inner: SharemindNetworkConfiguration,
    pub local_node_number: usize,
    pub local_is_computing_node: bool,
    pub local_is_main_node: bool,
}

impl NetworkConfigurationWrapper {
    pub fn new(local_node_number: usize, local_is_computing_node: bool) -> Self {
        Self {
            inner: SharemindNetworkConfiguration {
                get_local_node_number: Some(get_local_node_number),
                get_local_is_computing_node: Some(get_local_is_computing_node),
                get_local_is_master_node: Some(get_local_is_master_node),
            },
            local_node_number,
            local_is_computing_node,
            local_is_main_node: local_node_number == 1,
        }
    }
}

impl WrappedC<SharemindNetworkConfiguration> for NetworkConfigurationWrapper {}

unsafe extern "C" fn get_local_node_number(c: *const SharemindNetworkConfiguration) -> usize {
    assert!(!c.is_null());
    NetworkConfigurationWrapper::get_wrapper(c).local_node_number
}

/// It exists because the Application Server supports passive PD nodes.
unsafe extern "C" fn get_local_is_computing_node(c: *const SharemindNetworkConfiguration) -> bool {
    assert!(!c.is_null());
    NetworkConfigurationWrapper::get_wrapper(c).local_is_computing_node
}

/// It exists because on the server-side one of computing nodes is the main node.
unsafe extern "C" fn get_local_is_master_node(c: *const SharemindNetworkConfiguration) -> bool {
    assert!(!c.is_null());
    NetworkConfigurationWrapper::get_wrapper(c).local_is_main_node
}
