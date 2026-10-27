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

use std::ptr::null_mut;

use crate::ffi::wrapper::{
    network_configuration::NetworkConfigurationWrapper, node::NodeWrapper, WrappedC,
};

use crate::ffi::bindings::{SharemindNetwork, SharemindNode};

pub trait Network {
    fn get_node(&mut self, node_id: usize) -> Option<&mut NodeWrapper>;
}

#[repr(C)]
pub struct NetworkWrapper {
    pub inner: SharemindNetwork,
    parent: Box<dyn Network>,
}

impl NetworkWrapper {
    pub fn new(
        _network_configuration: &NetworkConfigurationWrapper,
        network: Box<dyn Network>,
    ) -> Self {
        Self {
            inner: SharemindNetwork {
                free: Some(free),
                get_node: Some(get_node),
                get_configuration: None, // Never called by PD module
            },
            parent: network,
        }
    }
}

impl WrappedC<SharemindNetwork> for NetworkWrapper {}

// network is not heap allocated
unsafe extern "C" fn free(_network: *mut SharemindNetwork) {}

unsafe extern "C" fn get_node(
    network: *mut SharemindNetwork,
    node_id: usize,
) -> *mut SharemindNode {
    assert!(!network.is_null());
    let w = NetworkWrapper::get_mut_wrapper(network);
    match w.parent.get_node(node_id) {
        Some(node) => &mut node.inner as *mut _,
        None => null_mut(),
    }
}
