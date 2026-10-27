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
    bindings::{SharemindMessage, SharemindNetworkError__SHAREMIND_NETWORK_UNKNOWN_ERROR},
    wrapper::{node::Node, NetworkResult},
};

pub mod client;
pub mod server;

use std::{cmp::min, mem::ManuallyDrop, slice};

pub struct ZeromqNode {
    pub node_number: usize,
    // Send and receive the struct containing pointer to the message's data.
    // Sender passes ownership of the struct to the recipient.
    // Receiver will have to free both the message and data pointed to.
    pub socket: zmq::Socket,
}

impl Node for ZeromqNode {
    fn get_node_number(&self) -> usize {
        self.node_number
    }

    // The thing with sending is that after something has been sent,
    // the module considers the memory under `data` safe to use again.
    // We'd have to copy that data into a vec so that it wouldn't turn stale
    // before ingestion by the recipient.
    fn send_message(&self, msg: SharemindMessage) -> NetworkResult<()> {
        tracing::trace!(
            size = msg.size,
            data = (0..min(5, msg.size))
                .map(|i| unsafe { *(msg.data as *const u8).add(i) })
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<String>>()
                .join(""),
            data_ptr = format!("{:?}", msg.data),
            node = self.node_number,
            socket = format!("{:p}", &self.socket),
            "send"
        );

        // send() converts byte slice into zmq::Message by copying data
        let slice = unsafe { slice::from_raw_parts(msg.data as *const u8, msg.size) };
        self.socket
            .send(slice, 0)
            .inspect_err(|e| tracing::error!(node = self.node_number, error = e.message(), "send"))
            .map_err(|_| SharemindNetworkError__SHAREMIND_NETWORK_UNKNOWN_ERROR)
    }

    fn receive_message(&mut self) -> NetworkResult<SharemindMessage> {
        // This should result in at least two allocations for each call:
        //   (1) for zmq_msg_t which holds the data pointer.
        //   (2) Copying out the data into a vector, since I don't want to do unsafe trickery here,
        //       nor know whether this would be safe.
        // If this is a problem, then a global `zmq_msg_t` could be kept around which is reused all
        // the time, and below `free_message` is just a noop. This assumes that `receive_message`
        // and its associated `free_message` are always called as pairs, not intertwined with
        // another `receive_message`.
        let vec_msg = self
            .socket
            .recv_bytes(0)
            .inspect_err(|e| tracing::error!(node = self.node_number, error = e.message(), "recv"))
            .map_err(|_| SharemindNetworkError__SHAREMIND_NETWORK_UNKNOWN_ERROR)?;

        // keep the data in a boxed slice and drop it manually when free() is called.
        let boxed_msg = ManuallyDrop::new(vec_msg.into_boxed_slice());
        let msg = SharemindMessage {
            data: boxed_msg.as_ptr() as *const _,
            size: boxed_msg.len(),
        };

        tracing::trace!(
            size = msg.size,
            data = (0..min(5, msg.size))
                .map(|i| unsafe { *(msg.data as *const u8).add(i) })
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<String>>()
                .join(""),
            data_ptr = format!("{:?}", msg.data),
            node = self.node_number,
            "recv"
        );
        Ok(msg)
    }

    fn free_message(&self, msg: SharemindMessage) {
        drop(unsafe {
            Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                msg.data as *mut u8,
                msg.size,
            ))
        });
    }
}
