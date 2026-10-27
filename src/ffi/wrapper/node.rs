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
    bindings::{SharemindMessage, SharemindNetworkError__SHAREMIND_NETWORK_OK, SharemindNode},
    wrapper::{NetworkResult, SharemindNetworkError, WrappedC},
};
use std::ptr::null;

pub trait Node {
    fn get_node_number(&self) -> usize;
    fn send_message(&self, _: SharemindMessage) -> NetworkResult<()>;
    fn receive_message(&mut self) -> NetworkResult<SharemindMessage>;
    fn free_message(&self, _: SharemindMessage);
}

#[repr(C)]
pub struct NodeWrapper {
    pub inner: SharemindNode,
    parent: Box<dyn Node>,
    last_error: SharemindNetworkError,
}

impl NodeWrapper {
    pub fn new(parent: Box<dyn Node>) -> Self {
        Self {
            inner: SharemindNode {
                last_error: Some(last_error),
                clear_error: Some(clear_error),
                is_computing_node: Some(is_computing_node),
                get_node_number: Some(get_node_number),
                send_message: Some(send_message),
                receive_message: Some(receive_message),
                free_message: Some(free_message),
            },
            parent,
            last_error: SharemindNetworkError__SHAREMIND_NETWORK_OK,
        }
    }
}

impl WrappedC<SharemindNode> for NodeWrapper {}

unsafe extern "C" fn last_error(node: *const SharemindNode) -> SharemindNetworkError {
    assert!(!node.is_null());
    NodeWrapper::get_wrapper(node).last_error
}

unsafe extern "C" fn clear_error(node: *mut SharemindNode) {
    assert!(!node.is_null());
    let w = NodeWrapper::get_mut_wrapper(node);
    w.last_error = SharemindNetworkError__SHAREMIND_NETWORK_OK;
}

unsafe extern "C" fn is_computing_node(node: *const SharemindNode) -> bool {
    assert!(!node.is_null());
    true // Not expected to ever be false.
}

unsafe extern "C" fn get_node_number(node: *const SharemindNode) -> usize {
    assert!(!node.is_null());
    NodeWrapper::get_wrapper(node).parent.get_node_number()
}

unsafe extern "C" fn send_message(
    node: *mut SharemindNode,
    message: SharemindMessage,
) -> SharemindNetworkError {
    assert!(!node.is_null());
    assert!(!message.data.is_null());
    match NodeWrapper::get_wrapper(node).parent.send_message(message) {
        Ok(()) => SharemindNetworkError__SHAREMIND_NETWORK_OK,
        Err(e) => e,
    }
}

unsafe extern "C" fn receive_message(node: *mut SharemindNode) -> SharemindMessage {
    assert!(!node.is_null());
    let w = NodeWrapper::get_mut_wrapper(node);
    match w.parent.receive_message() {
        Ok(msg) => msg,
        Err(e) => {
            w.last_error = e;
            SharemindMessage {
                data: null(),
                size: 0,
            }
        }
    }
}

unsafe extern "C" fn free_message(node: *mut SharemindNode, message: *mut SharemindMessage) {
    assert!(!node.is_null());
    assert!(!message.is_null());
    NodeWrapper::get_mut_wrapper(node)
        .parent
        .free_message(*message)
}
