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
        SharemindNetworkError__SHAREMIND_NETWORK_CONFIGURATION_ALREADY_INITIALIZED,
        SharemindNetworkError__SHAREMIND_NETWORK_CONFIGURATION_LIMITS_REACHED,
        SharemindNetworkError__SHAREMIND_NETWORK_INVALID_ARGUMENT, SharemindNodeConfiguration,
    },
    wrapper::{
        network::{Network, NetworkWrapper},
        network_configuration::NetworkConfigurationWrapper,
        NetworkResult,
    },
};

pub trait NetworkFactory {
    fn new_network(&mut self) -> Option<Box<dyn Network>>;
    fn total_node_count(&self) -> usize;
    fn local_node_id(&self) -> usize;
}

/// The protection domain network facility implements the network API that is
/// consumed by the protection domain modules.
///
/// # Regarding the use of server-side PD network APIs on the client-side
///
/// The PD network API was originally designed for the server-side PD modules
/// compatible with the Sharemind Application Server. The API accounts for
/// the server's support of multiple protection domains in a single deployment,
/// where each PD operates on a subset of computing nodes and requires its own
/// independent set of communication channels. Some of API methods and structures
/// may feel redundant when applied in the client-side modules and in conjunction
/// with the Sharemind Runtime, which only supports one PD with a single set of
/// computing nodes. For example:
///
/// * init_configuration()
///
///   Historically: this was used by PDs to tell the server which physical MPC
///   server nodes they wanted to operate on, so the respective servers can form
///   new networks with each other. This is still relevant if we want to stay
///   compatible with the Sharemind Application Server platform.
///
///   Simplification possibility: In Sharemind Runtime with a single PD all
///   nodes are computing nodes, no "passive" nodes exist, and no
///   logical-to-physical node mapping configuration is necessary.
///   The client is configured with the servers of relevant computing parties.
///   The PD can assume all computing nodes map directly to the configured servers.
///   Although in that case this method is less useful, it can still be used for
///   validating whether the PD node expectations can be met by the server
///   configuration of the Client app. However, the same can be achieved by
///   iterating over SharemindNetwork::get_node and checking for nullptr. Also,
///   the PD nodes are registered with the "PdData" facility and this info could
///   be reused by the client app in its PD network implementation (dont repeat
///   yourself).
///
/// * SharemindNetworkConfiguration
///
///   The configuration returned by `init_configuration()` describes the
///   role of a local PD node in relation to the whole deployment. This was
///   useful on the server-side, but has little use on the client-side. From the
///   client's perspective the local node can always be described as
///   `{ node_number = 0, is_computing_node = false, is_master_node = false}`.
///
/// In any case, to stay fully compatible with the original Sharemind
/// Application Server paradigm we need to support the creation of multiple
/// independent networks.
pub struct NetworkFacility {
    factory: Box<dyn NetworkFactory>,
    networks: Vec<NetworkWrapper>,
    config: Option<Box<NetworkConfigurationWrapper>>,
}

impl NetworkFacility {
    pub fn new(factory: Box<dyn NetworkFactory>) -> Self {
        Self {
            factory,
            networks: Vec::new(),
            config: None,
        }
    }

    pub fn total_node_count(&self) -> usize {
        self.factory.total_node_count()
    }

    pub fn local_node_id(&self) -> usize {
        self.factory.local_node_id()
    }

    /// This is mostly for compliance with the Sharemind network API
    pub fn node_id(&self, server_name: &str) -> Option<usize> {
        assert_eq!(self.factory.total_node_count(), 3);
        match server_name {
            s if s == "DebugMiner1" || s == "Server1" || s == "DebugServer1" => Some(1),
            s if s == "DebugMiner2" || s == "Server2" || s == "DebugServer2" => Some(2),
            s if s == "DebugMiner3" || s == "Server3" || s == "DebugServer3" => Some(3),
            _ => None,
        }
    }

    /// Initialize the network configuration object required by the Sharemind
    /// network API.
    ///
    /// # Node mapping
    ///
    /// This method is mostly for compliance with the Sharemind network APIs.
    /// Unlike the Sharemind Application Server, the Runtime does not support
    /// arbitrary mapping of logical nodes to physical servers (i.e. subsetting,
    /// reordering). Instead we assume direct sequential node-to-server mapping.
    pub fn init_configuration(
        &mut self,
        node_configurations: &[SharemindNodeConfiguration],
    ) -> NetworkResult<&NetworkConfigurationWrapper> {
        if self.config.is_some() {
            return Err(SharemindNetworkError__SHAREMIND_NETWORK_CONFIGURATION_ALREADY_INITIALIZED);
        }

        // Validate node configurations. Assume direct node to server mapping.
        if node_configurations.len() != self.factory.total_node_count() {
            return Err(SharemindNetworkError__SHAREMIND_NETWORK_CONFIGURATION_LIMITS_REACHED);
        }
        for (i, c) in node_configurations.iter().enumerate() {
            if c.node_number != i + 1 || c.miner_number != i + 1 || !c.is_computing_node {
                return Err(SharemindNetworkError__SHAREMIND_NETWORK_INVALID_ARGUMENT);
            }
        }

        // Client is always node 0, servers start from 1.
        // Client is not a computing node.
        // Servers are always computing nodes.
        let local_node_id = self.local_node_id();
        self.config = Some(Box::new(NetworkConfigurationWrapper::new(
            local_node_id,
            local_node_id != 0,
        )));

        Ok(self.config.as_ref().expect("network conf must exist"))
    }

    /// Creates a new network usable for either PD or PDPI communication.
    pub fn new_network(
        &mut self,
        network_configuration: &NetworkConfigurationWrapper,
    ) -> NetworkResult<&mut NetworkWrapper> {
        // Validate the API was used correctly:
        //  1) init_configuration() must have been called beforehand.
        //  2) The network conf object must be the one returned by the
        //     init_configuration() call.
        match self.config.as_ref() {
            Some(c) => {
                if !std::ptr::eq(c.as_ref(), network_configuration) {
                    // Invalid network configuration object provided.
                    return Err(SharemindNetworkError__SHAREMIND_NETWORK_INVALID_ARGUMENT);
                }
            }
            // Network configuration not initialized. Provided config cannot be correct.
            None => return Err(SharemindNetworkError__SHAREMIND_NETWORK_INVALID_ARGUMENT),
        }

        // Create network
        if let Some(network) = self.factory.new_network() {
            self.networks
                .push(NetworkWrapper::new(network_configuration, network));
            Ok(self.networks.last_mut().unwrap())
        } else {
            tracing::error!("Unable to create more networks");
            Err(SharemindNetworkError__SHAREMIND_NETWORK_CONFIGURATION_LIMITS_REACHED)
        }
    }
}
