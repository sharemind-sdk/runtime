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

use core::slice;
use std::{
    cmp::min,
    mem::ManuallyDrop,
    sync::mpsc::{Receiver, Sender},
};

use crate::ffi::{
    bindings::SharemindMessage,
    facility::network::NetworkFactory,
    wrapper::{
        network::Network,
        node::{Node, NodeWrapper},
        NetworkResult,
    },
};

use self::context::WorkerContext;

#[derive(Debug)]
pub struct InmemoryNode {
    node_number: usize,
    // Send and receive the struct containing pointer to the message's data.
    // Sender passes ownership of the struct to the recipient.
    // Receiver will have to free both the message and data pointed to.
    recv: Receiver<Vec<u8>>,
    send: Sender<Vec<u8>>,
}

impl Node for InmemoryNode {
    fn get_node_number(&self) -> usize {
        self.node_number
    }

    /*
     * The thing with sending is that after something has been sent,
     * the module considers the memory under `data` safe to use again.
     * We'd have to copy that data into a vec so that it wouldn't turn stale before ingestion by
     * the recipient.
     */
    fn send_message(&self, msg: SharemindMessage) -> NetworkResult<()> {
        tracing::debug!(
            size = msg.size,
            data = (0..min(5, msg.size))
                .map(|i| unsafe { *(msg.data as *const u8).add(i) })
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<String>>()
                .join(""),
            data_ptr = format!("{:?}", msg.data),
            node = self.node_number,
            "send"
        );
        let vec_msg = unsafe { slice::from_raw_parts(msg.data as *const u8, msg.size) }.to_vec();
        match self.send.send(vec_msg) {
            Err(e) => panic!("{:?}", e),
            _ => Ok(()),
        }
    }

    fn receive_message(&mut self) -> NetworkResult<SharemindMessage> {
        let vec_msg = match self.recv.recv() {
            Ok(msg) => msg,
            Err(e) => panic!("{:?}", e),
        };
        // keep the data in a boxed slice and drop it manually when free() is called.
        let boxed_msg = ManuallyDrop::new(vec_msg.into_boxed_slice());
        let msg = SharemindMessage {
            data: boxed_msg.as_ptr() as *const _,
            size: boxed_msg.len(),
        };
        tracing::debug!(
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

pub mod context;

pub struct InmemoryNetwork {
    nodes: [NodeWrapper; 2],
    local_node_id: usize,
}

impl Network for InmemoryNetwork {
    fn get_node(&mut self, node_id: usize) -> Option<&mut NodeWrapper> {
        assert_ne!(node_id, self.local_node_id);
        self.nodes
            .get_mut(is_next(self.local_node_id, node_id) as usize)
    }
}

fn is_next(local: usize, remote: usize) -> bool {
    assert!((1..=3).contains(&local));
    assert!((1..=3).contains(&remote));
    local == remote - 1 || local == 3 && remote == 1
}

impl InmemoryNetwork {
    pub fn new(context: WorkerContext) -> Self {
        let local_node_id = context.id;
        Self {
            nodes: [
                NodeWrapper::new(Box::new(InmemoryNode {
                    node_number: ((local_node_id + 1) % 3) + 1,
                    recv: context.recv_left,
                    send: context.send_left,
                })),
                NodeWrapper::new(Box::new(InmemoryNode {
                    node_number: (local_node_id % 3) + 1,
                    recv: context.recv_right,
                    send: context.send_right,
                })),
            ],
            local_node_id,
        }
    }
}

pub struct InmemoryNetworkFactory {
    local_node_id: usize,
    net_ctx: Option<WorkerContext>,
}

impl InmemoryNetworkFactory {
    pub fn new(net_ctx: WorkerContext) -> Self {
        Self {
            local_node_id: net_ctx.id,
            net_ctx: Some(net_ctx),
        }
    }
}

impl NetworkFactory for InmemoryNetworkFactory {
    fn new_network(&mut self) -> Option<Box<dyn Network>> {
        self.net_ctx
            .take()
            .map(|context| Box::new(InmemoryNetwork::new(context)) as Box<dyn Network>)
    }

    fn total_node_count(&self) -> usize {
        3
    }

    fn local_node_id(&self) -> usize {
        self.local_node_id
    }
}
