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

use crate::ffi::ctrl::cmodapi::module::ControllerModule;
use crate::ffi::ctrl::cmodapi::pd::{ProtectionDomain, ValueMap};
use crate::ffi::ctrl::cmodapi::ControllerModuleApi;
use crate::ffi::facility::network::NetworkFacility;
use crate::ffi::facility::random::RandomFacility;
use crate::ffi::loghard::LogHardLogger;
use anyhow::Context;
use clap::Subcommand;
use std::cell::RefCell;
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use tracing_subscriber::fmt::time::LocalTime;

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    /// Convert data from plaintext into protection domain representation.
    Classify,
    /// Convert data from protection domain representation into plaintext.
    Declassify,
}

/// A service for using a Sharemind client-side protection domain module (also
/// referred to as Controller module) to classify and declassify data.
///
/// A controller module defines a set of Protection Domain Kinds and their
/// supported data types for the 'classify' and 'declassify' operations.
/// Each PDK implements some cryptographic multi-party computation scheme that
/// can be instantiated as a protection domain for a fixed number of parties
/// (MPC nodes). The PD service loads one controller module and instantiates one
/// protection domain kind from the module.
///
/// The classify operation essentially converts data values from plaintext into
/// the internal representation of the protection domain to which the value belongs.
/// The resulting classified values consist of one component per each PD party.
/// The declassify operation reverses this process by converting data back to
/// plaintext.
/// The actual operation depends on the scheme implemented by the PDK, e.g.:
///
/// ----------------------------------------------------------------------------
/// PDK scheme type             Classify                Declassify
/// ----------------------------------------------------------------------------
/// Secret Sharing              Sharing                 Reconstructing
/// Garbled Circuits            Garbling / Encoding     Evaluating / Decoding
/// Homomorphic Encryption      Encrypting              Decrypting
/// ----------------------------------------------------------------------------
///
/// Alternatively to the protection domain instantiated from a PDK, the values
/// can also belong to the implicitly defined public domain that exists on all
/// computing nodes. Public values stay in plaintext form at all times and are
/// replicated among the nodes as such.
pub struct ProtectionDomainService {
    // General structures
    cmod_api: Rc<ControllerModuleApi>,
    _module: Rc<ControllerModule>,
    pd: ProtectionDomain,
    _network_facility: Option<Rc<RefCell<NetworkFacility>>>,

    // Module facilities
    logger_facility: Box<LogHardLogger>,
    random_facility: Box<RandomFacility>,
}

impl ProtectionDomainService {
    pub fn new(
        logger: &LogHardLogger,
        module_path: &Path,
        pdk_name: &str,
        pd_name: &str,
        pd_conf: Option<&Path>,
        network_facility: Option<Rc<RefCell<NetworkFacility>>>,
    ) -> Result<Self, Box<dyn Error>> {
        let cmod_api = Rc::new(ControllerModuleApi::new()?);

        // Module facilities
        let mut sublogger = Box::new(logger.sublogger(c"[PDService]"));
        cmod_api.set_module_facility(sublogger.as_mut())?;

        let mut random_facility = Box::new(RandomFacility::new());
        cmod_api.set_module_facility(random_facility.as_mut())?;

        // Load controller module
        let module = Rc::new(ControllerModule::new(cmod_api.clone(), module_path)?);

        // Load protection domain (only one, as support for simultaneous PDs is phased out)
        let pd = ProtectionDomain::new(
            module.clone(),
            pdk_name,
            pd_name,
            pd_conf,
            network_facility.clone(),
        )
        .map_err(|e| format!("Failed to create protection domain '{}': {}", pd_name, e))?;

        pd.start()
            .map_err(|e| format!("Failed to start protection domain '{}': {}", pd_name, e))?;

        Ok(Self {
            cmod_api,
            _module: module,
            pd,
            _network_facility: network_facility,
            logger_facility: sublogger,
            random_facility,
        })
    }

    pub fn node_count(&self) -> usize {
        self.pd.node_count()
    }

    /// Classify the values locally based on their protection domain.
    ///
    /// Returns one value set per MPC node in the loaded protection domain.
    /// Each output value set contains the components of classified values that
    /// correspond to the original input values and are intended for one specific
    /// MPC node. Values in the public domain are replicated in plaintext to
    /// each value set.
    pub fn classify(&self, plaintext_values: &ValueMap) -> Result<Vec<ValueMap>, String> {
        let mut node_value_sets = vec![ValueMap::default(); self.pd.node_count()];

        for (name, value) in plaintext_values {
            if value.is_public() {
                // Put the same plaintext value in each output value set
                for value_set in &mut node_value_sets {
                    let old_value = value_set.insert(name.clone(), value.clone());
                    assert_eq!(old_value, None); // input must have unique value names
                }
            } else if value.pd_name == self.pd.pd_name() {
                // Put each classified value component to a different node value set
                let classified_components = self.pd.classify(value)?;
                assert_eq!(classified_components.len(), node_value_sets.len());
                for (value_set, classified_component) in
                    node_value_sets.iter_mut().zip(classified_components)
                {
                    let old_value = value_set.insert(name.clone(), classified_component);
                    assert_eq!(old_value, None);
                }
            } else {
                return Err(format!(
                    "Value '{}' has unexpected protection domain: {}",
                    name, value.pd_name,
                ));
            }
        }

        Ok(node_value_sets)
    }

    /// Declassify the values locally based on their protection domain.
    ///
    /// This reconstructs the original values from the input value sets containing
    /// the components of classified values in the protection domain data
    /// representation.
    /// Values in the public domain are validated for equality and returned in
    /// plaintext as is.
    pub fn declassify(&self, value_sets: &[ValueMap]) -> Result<ValueMap, String> {
        if value_sets.len() != self.pd.node_count() {
            return Err("Invalid number of input value sets".to_owned());
        }

        let mut results = ValueMap::new();

        // Take the first value set as reference
        if let Some((first, rest)) = value_sets.split_first() {
            // Check that all value sets have equal keys in them
            if !rest.iter().all(|m| {
                m.keys()
                    .collect::<HashSet<_>>()
                    .eq(&first.keys().collect::<HashSet<_>>())
            }) {
                return Err("Input value sets do not have equal sets of keys".to_owned());
            }

            // Process the value sets
            for (k, v) in first {
                if v.is_public() {
                    // Is public value equal across the value sets?
                    if !rest.iter().all(|m| &m[k] == v) {
                        return Err(format!(
                            "Public value '{k}' is not equal across the input value sets"
                        ));
                    }

                    let old_value = results.insert(k.clone(), v.clone());
                    assert_eq!(old_value, None);
                } else if v.pd_name == self.pd.pd_name() {
                    let value_components = value_sets.iter().map(|m| &m[k]).collect::<Vec<_>>();
                    let value = self.pd.declassify(&value_components)?;
                    let old_value = results.insert(k.clone(), value);
                    assert_eq!(old_value, None);
                } else {
                    return Err(format!(
                        "Protection domain '{}' specified by value '{}' does not exist",
                        v.pd_name, k,
                    ));
                }
            }
        }

        Ok(results)
    }

    /// Classify one value in the protocol mode.
    ///
    /// Protocol mode involves an interactive online protocol between the client
    /// and a set of MPC nodes to upload the input data in protection domain
    /// representation to the MPC nodes. The protocol is executed concurrently by
    ///  1) the client-side PD classify syscall for the given data type, and
    ///  2) its server-side counterpart PD `upload_vec` syscall on each MPC node.
    ///
    /// Returns a result indicating success.
    pub fn classify_protocol(
        &self,
        plaintext_data: &[u8],
        pd_typename: &str,
    ) -> Result<(), String> {
        self.pd.classify_protocol(pd_typename, plaintext_data)
    }

    /// Declassify one value in the protocol mode.
    ///
    /// Similarly to classify_protocol, this involves an interactive online
    /// protocol between the client and a set of MPC nodes to download the data
    /// in protection domain representation from the MPC nodes and reconstruct
    /// it as plaintext. The protocol is executed concurrently by
    ///  1) the client-side PD declassify syscall for the given data type, and
    ///  2) its server-side counterpart PD `download_vec` syscall on each MPC node.
    ///
    /// The reconstructed data is written to the provided writer.
    /// Returns a result indicating success.
    pub fn declassify_protocol<W: Write>(
        &self,
        pd_typename: &str,
        writer: &mut W,
    ) -> Result<(), String> {
        self.pd.declassify_protocol(pd_typename, writer)
    }
}

impl Drop for ProtectionDomainService {
    fn drop(&mut self) {
        self.pd.stop();
        self.cmod_api
            .unset_module_facility(self.random_facility.as_mut());
        self.cmod_api
            .unset_module_facility(self.logger_facility.as_mut());
    }
}

/// Initialize tracing for structured logging.
/// Environment variable RUST_LOG can be used to customize the logging level.
pub fn setup_logging() -> Result<(), Box<dyn Error>> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or("info".into());
    let timer = LocalTime::new(time::format_description::parse(
        "[year]-[month padding:zero]-[day padding:zero] [hour]:[minute]:[second]",
    )?);
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_timer(timer)
        .with_target(false)
        .init();
    Ok(())
}

pub fn create_protection_domain(
    pd_module_path: Option<PathBuf>,
    pdk_name: String,
    pd_conf_path: Option<PathBuf>,
    network_facility: Option<Rc<RefCell<NetworkFacility>>>,
) -> Result<(String, ProtectionDomainService), Box<dyn Error>> {
    let pd_module_path = pd_module_path
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("libsharemind_mod_{}_ctrl.so", pdk_name)));
    let pd_name = format!("pd_{}", pdk_name);
    let logger = LogHardLogger::default();
    let pd = ProtectionDomainService::new(
        &logger,
        &pd_module_path,
        &pdk_name,
        &pd_name,
        pd_conf_path.as_deref(),
        network_facility,
    )?;
    Ok((pd_name, pd))
}

pub fn read_input_value_data(plaintext_path: PathBuf) -> Result<Vec<u8>, Box<dyn Error>> {
    let plaintext_data = fs::read(&plaintext_path)
        .with_context(|| format!("Failed to read input: {}", plaintext_path.display()))?;
    Ok(plaintext_data)
}

/// Writer for the reconstructed plaintext output data.
/// If path is a file: truncate if exists, create if missing.
/// If path is a pipe: append if exists, error if missing.
pub fn prepare_output_data_writer(
    pipe: bool,
    plaintext_path: PathBuf,
) -> Result<File, Box<dyn Error>> {
    let plaintext_writer = OpenOptions::new()
        .write(true)
        .create(!pipe)
        .truncate(!pipe)
        .open(&plaintext_path)
        .with_context(|| format!("Failed to open output path: {}", plaintext_path.display()))?;
    Ok(plaintext_writer)
}
