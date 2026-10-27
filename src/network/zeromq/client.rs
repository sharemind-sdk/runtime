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

use std::net::SocketAddr;

use crate::{
    ffi::{
        facility::network::NetworkFactory,
        wrapper::{network::Network, node::NodeWrapper},
    },
    network::zeromq::ZeromqNode,
};

#[derive(Debug)]
pub struct NodeConfig {
    pub address: SocketAddr,
    // pub ca: Option<String>,
    // pub cert: Option<String>,
}

impl NodeConfig {
    /// Parses node configuration from an argument string.
    /// Format: addr={host:port}
    pub fn from_arg(arg: &str) -> Result<Self, String> {
        let mut address = None;
        // let mut ca = None;
        // let mut cert = None;

        for kv in arg.split(',') {
            let mut parts = kv.splitn(2, '=');
            let key = parts.next().unwrap().trim();
            let val = parts
                .next()
                .ok_or_else(|| format!("Invalid pair: {kv}"))?
                .trim();

            match key {
                "addr" => {
                    address = Some(
                        val.parse::<SocketAddr>()
                            .map_err(|e| format!("Error parsing address: {e}"))?,
                    )
                }
                // "ca" => ca = Some(val.to_string()),
                // "cert" => cert = Some(val.to_string()),
                _ => return Err(format!("Unknown key: {key}")),
            }
        }

        Ok(NodeConfig {
            address: address.ok_or("Missing addr")?,
            // ca,
            // cert,
        })
    }
}

pub struct NetworkConfig {
    pub nodes: Vec<NodeConfig>,
    // pub client_cert: String,
    // pub client_key: String,
}

pub struct WorkerContext {
    nodes: Vec<zmq::Socket>,
}

impl WorkerContext {
    pub fn new(config: &NetworkConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let ctx = zmq::Context::new();
        let mut nodes = Vec::with_capacity(config.nodes.len());
        for node_config in &config.nodes {
            let socket = ctx.socket(zmq::PAIR)?;
            socket
                .connect(&format!("tcp://{}", node_config.address))
                .map_err(|e| format!("Failed to connect to {}: {e}", node_config.address))?;
            nodes.push(socket);
        }
        tracing::trace!(nodes = nodes.len(), "Created context");
        Ok(Self { nodes })
    }
}

/// Client-side network of remote nodes based on ZeroMQ.
pub struct ZeromqNetwork {
    nodes: Vec<NodeWrapper>,
}

impl ZeromqNetwork {
    pub fn new(context: WorkerContext) -> Self {
        Self {
            nodes: context
                .nodes
                .into_iter()
                .enumerate()
                .map(|(idx, socket)| {
                    NodeWrapper::new(Box::new(ZeromqNode {
                        node_number: idx + 1,
                        socket,
                    }))
                })
                .collect(),
        }
    }
}

impl Network for ZeromqNetwork {
    fn get_node(&mut self, node_id: usize) -> Option<&mut NodeWrapper> {
        assert!(node_id > 0);
        self.nodes.get_mut(node_id - 1)
    }
}

/// Client-side ZeroMQ network factory
pub struct ZeromqNetworkFactory {
    node_count: usize,
    net_ctx: Option<WorkerContext>,
}

impl ZeromqNetworkFactory {
    pub fn new(net_ctx: WorkerContext) -> Self {
        Self {
            node_count: net_ctx.nodes.len(),
            net_ctx: Some(net_ctx),
        }
    }
}

impl NetworkFactory for ZeromqNetworkFactory {
    /// # Limitations
    ///
    /// Currently no more than one network can be created.
    /// See discussion of similar limitation in runtime/src/network/zeromq/mod.rs
    fn new_network(&mut self) -> Option<Box<dyn Network>> {
        self.net_ctx
            .take()
            .map(|context| Box::new(ZeromqNetwork::new(context)) as Box<dyn Network>)
    }

    fn total_node_count(&self) -> usize {
        self.node_count
    }

    fn local_node_id(&self) -> usize {
        0 // Client is node 0
    }
}
