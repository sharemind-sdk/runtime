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

use crate::{
    ffi::{
        facility::network::NetworkFactory,
        wrapper::{network::Network, node::NodeWrapper},
    },
    network::zeromq::ZeromqNode,
};

pub struct WorkerContext {
    pub id: usize,
    pub left: zmq::Socket,
    pub right: zmq::Socket,
    pub client: Option<zmq::Socket>,
}

impl WorkerContext {
    pub fn new(
        id: usize,
        left_address: &str,
        right_listen_address: &str,
        client_listen_address: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let ctx = zmq::Context::new();

        let socket_right = ctx.socket(zmq::DEALER)?;
        socket_right
            .bind(&format!("tcp://{right_listen_address}"))
            .map_err(|e| format!("Failed to bind to address {right_listen_address}: {e}"))?;
        tracing::trace!("Listening to right peer on {right_listen_address}");

        let socket_left = ctx.socket(zmq::DEALER)?;
        socket_left
            .connect(&format!("tcp://{left_address}"))
            .map_err(|e| format!("Failed to connect to address {left_address}: {e}"))?;
        tracing::trace!("Connected to left peer at {left_address}");

        let socket_client = client_listen_address
            .map(|addr| -> Result<zmq::Socket, Box<dyn std::error::Error>> {
                let socket_client = ctx.socket(zmq::PAIR)?;
                socket_client
                    .bind(&format!("tcp://{addr}"))
                    .map_err(|e| format!("Failed to bind to address {addr}: {e}"))?;
                tracing::trace!("Listening to client on {addr}");
                Ok(socket_client)
            })
            .transpose()?;

        Ok(Self {
            id,
            left: socket_left,
            right: socket_right,
            client: socket_client,
        })
    }
}

/// Server-side network based on ZeroMQ.
///
/// Network size is fixed at 3 server nodes in total: 2 remote + 1 local (oblique).
/// Local node is part of the network, but doesn't have a node object (no talking to ourselves).
///
/// We allow retrieving the client node (if available) through this network
/// as a workaround to avoid implementing a fullblown ClientNetwork facility.
/// This may be subject to change in the future.
pub struct ZeromqNetwork {
    local_node_id: usize,
    nodes: [NodeWrapper; 2],
    client: Option<NodeWrapper>,
}

impl ZeromqNetwork {
    pub fn new(context: WorkerContext) -> Self {
        let local_node_id = context.id;
        Self {
            local_node_id,
            nodes: [
                NodeWrapper::new(Box::new(ZeromqNode {
                    node_number: ((local_node_id + 1) % 3) + 1,
                    socket: context.left,
                })),
                NodeWrapper::new(Box::new(ZeromqNode {
                    node_number: (local_node_id % 3) + 1,
                    socket: context.right,
                })),
            ],
            client: context.client.map(|socket| {
                NodeWrapper::new(Box::new(ZeromqNode {
                    node_number: 0,
                    socket,
                }))
            }),
        }
    }
}

fn is_next(local: usize, remote: usize) -> bool {
    assert!((1..=3).contains(&local));
    assert!((1..=3).contains(&remote));
    local == remote - 1 || local == 3 && remote == 1
}

impl Network for ZeromqNetwork {
    /// Retrieve a network node by node id.
    ///
    /// All server nodes part of the network have id > 0.
    /// As a special case the client node has id = 0 and can only be retrieved if available.
    fn get_node(&mut self, node_id: usize) -> Option<&mut NodeWrapper> {
        assert_ne!(node_id, self.local_node_id);
        if node_id == 0 {
            self.client.as_mut()
        } else {
            self.nodes
                .get_mut(is_next(self.local_node_id, node_id) as usize)
        }
    }
}

/// Server-side ZeroMQ network factory
pub struct ZeromqNetworkFactory {
    local_node_id: usize,
    net_ctx: Option<WorkerContext>,
}

impl ZeromqNetworkFactory {
    pub fn new(net_ctx: WorkerContext) -> Self {
        Self {
            local_node_id: net_ctx.id,
            net_ctx: Some(net_ctx),
        }
    }
}

impl NetworkFactory for ZeromqNetworkFactory {
    /// # Limitations
    ///
    /// ## One network at a time
    ///
    /// The original C++ code created overlay networks that multiplex over
    /// the actual communication channels and thus had separate contexts.
    /// Current implementation holds communication channels directly inside
    /// the net_ctx, which is consumed by the first new network (i.e. channels
    /// or sockets are moved out of net_ctx) making two subsequent calls to
    /// new_network not possible. This limits us to one usable network at a
    /// time, which is enough for the Shared3P protection domain (for now).
    /// We can solve this by multiplexing the communication (like originally) or
    /// using separate channels/sockets for each network (may run out of ports).
    fn new_network(&mut self) -> Option<Box<dyn Network>> {
        self.net_ctx
            .take()
            .map(|context| Box::new(ZeromqNetwork::new(context)) as Box<dyn Network>)
    }

    fn total_node_count(&self) -> usize {
        3
    }

    fn local_node_id(&self) -> usize {
        self.local_node_id
    }
}
