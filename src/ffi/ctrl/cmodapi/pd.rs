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

use super::{error::ControllerModuleApiError, module::ControllerModule};
use crate::{
    controller_module_api_err,
    ffi::{
        ctrl::{
            bindings::{
                SharemindCodeBlock, SharemindControllerCReference, SharemindControllerPd,
                SharemindControllerPd_free, SharemindControllerPd_isStarted,
                SharemindControllerPd_lastErrorString, SharemindControllerPd_setFacility,
                SharemindControllerPd_start, SharemindControllerPd_stop,
                SharemindControllerPdk_lastError, SharemindControllerPdk_lastErrorString,
                SharemindControllerPdk_newPd, SharemindControllerReference,
            },
            facility::pd_data::PdDataFacility,
        },
        facility::{network::NetworkFacility, pd_network::PdNetworkFacilityWrapper, Facility},
        io::{AsFfiCallback, WriteCallbackCtx},
    },
};
use std::collections::HashMap;
use std::{
    cell::RefCell,
    error::Error,
    ffi::{c_void, CString},
    io::Write,
    ops::Deref,
    os::unix::ffi::OsStrExt,
    path::Path,
    ptr::{null, null_mut, NonNull},
    rc::Rc,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Value {
    pub pd_name: String,
    pub type_name: String,
    pub data: Vec<u8>,
}

impl Value {
    pub fn is_public(&self) -> bool {
        self.pd_name.is_empty()
    }
    pub fn byte_len(&self) -> usize {
        self.data.len()
    }
}

pub type ValueMap = HashMap<String, Value>;

/// A Rust wrapper for a protection domain.
///
/// The wrapper facilitates the creation of a protection domain and the method
/// calls to classify and declassify one value object.
///
/// Steps to classify or declassify data:
///  1. Choose a PDK
///  2. Instantiate PDK into a PD.
///  3. Find the PDK::datatype and call its classify/declassify syscalls with
///     the input and output references while also passing in the PD handle.
///
/// Regarding the network feature:
///  1. During PD startup, PD reports its node configuration to the PD service
///     via the "PdData" facility (mandatory/expected).
///     This mainly covers the logical-to-physical node mapping (incl. node count).
///     The physical node config is responsibility of the application layer.
///     If physical network exists, then validate reported node count vs network node count.
///  2. PD can create a new PD network, that is mapped to physical server connections.
///  3. Classify/Declassify may use the PD network for communication with server-side PD nodes.
///  4. Classify/Declassify work with a number of value components equal to the
///     number of nodes reported by the PD via the "PdData" facility during startup.
///     This is regardless of whether or not PD uses the network or works offline.
pub struct ProtectionDomain {
    inner: NonNull<SharemindControllerPd>,
    module: Rc<ControllerModule>,

    pdk_name: String,
    pd_name: String,

    // PD-level facilities
    pd_data_facility: Box<PdDataFacility>,
    pd_network_facility: Option<Box<PdNetworkFacilityWrapper>>,
}

impl ProtectionDomain {
    pub fn new(
        module: Rc<ControllerModule>,
        pdk_name: &str,
        pd_name: &str,
        pd_conf: Option<&Path>,
        network_facility: Option<Rc<RefCell<NetworkFacility>>>,
    ) -> Result<Self, Box<dyn Error>> {
        let pdk = module
            .find_pdk(pdk_name)
            .ok_or_else(|| format!("Protecton domain kind not found: {}", pdk_name))?;

        // Create Protection Domain
        let pd_name_c = CString::new(pd_name).map_err(|_| "PD name has internal nul byte(s)")?;
        let pd_conf_c = pd_conf
            .map(|p| {
                CString::new(p.as_os_str().as_bytes())
                    .map_err(|_| "PD conf path has internal nul byte(s)")
            })
            .transpose()?;

        let pd = NonNull::new(unsafe {
            SharemindControllerPdk_newPd(
                pdk,
                pd_name_c.as_ptr(),
                pd_conf_c.as_ref().map_or(null(), |s| s.as_ptr()),
            )
        })
        .ok_or_else(|| {
            ControllerModuleApiError::with_ffi_reason(
                unsafe { SharemindControllerPdk_lastError(pdk) },
                unsafe { SharemindControllerPdk_lastErrorString(pdk) },
            )
        })?;

        let pd_guard =
            scopeguard::guard(pd, |pd| unsafe { SharemindControllerPd_free(pd.as_ptr()) });

        unsafe {
            controller_module_api_err!(
                SharemindControllerPdk_lastError(pdk),
                SharemindControllerPdk_lastErrorString(pdk)
            )?;
        }

        // Create PdData facility specifically for this PD instance
        let expected_node_count = network_facility
            .as_ref()
            .map(|f| f.borrow().total_node_count());
        let mut pd_data_facility = Box::new(PdDataFacility::new(expected_node_count));
        set_pd_facility(pd_guard.deref(), pd_data_facility.as_mut())?;

        // Create PD network facility specifically for this PD instance
        let pd_network_facility = match network_facility {
            Some(net_facility) => {
                let mut pd_net_facility =
                    Box::new(PdNetworkFacilityWrapper::new(net_facility.clone()));
                set_pd_facility(pd_guard.deref(), pd_net_facility.as_mut())?;
                Some(pd_net_facility)
            }
            None => None,
        };

        Ok(Self {
            inner: scopeguard::ScopeGuard::into_inner(pd_guard), // disarm the guard
            module,
            pdk_name: pdk_name.to_owned(),
            pd_name: pd_name.to_owned(),
            pd_data_facility,
            pd_network_facility,
        })
    }

    pub fn handle(&self) -> *mut SharemindControllerPd {
        self.inner.as_ptr()
    }

    pub fn pd_name(&self) -> &str {
        &self.pd_name
    }

    pub fn pdk_name(&self) -> &str {
        &self.pdk_name
    }

    fn is_started(&self) -> bool {
        unsafe { SharemindControllerPd_isStarted(self.handle()) }
    }

    pub fn start(&self) -> Result<(), Box<dyn Error>> {
        controller_module_api_err!(
            unsafe { SharemindControllerPd_start(self.handle()) },
            unsafe { SharemindControllerPd_lastErrorString(self.handle()) }
        )?;

        // Check that PD has registered its nodes.
        // PdData facility has already validated the registered nodes.
        if self.node_count() == 0 {
            self.stop();
            return Err("Protection domain started without registering any nodes".into());
        }

        Ok(())
    }

    pub fn stop(&self) {
        unsafe {
            if self.is_started() {
                SharemindControllerPd_stop(self.handle());
            }
        }
    }

    pub fn node_count(&self) -> usize {
        assert!(self.is_started());
        self.pd_data_facility.node_configurations().len()
    }

    fn construct_data_type_signature(&self, type_name: &str) -> String {
        format!("{}::{type_name}", self.pdk_name)
    }

    pub fn classify(&self, value: &Value) -> Result<Vec<Value>, String> {
        if !self.is_started() {
            return Err("Protection domain not started yet".to_owned());
        }

        // Data type calculations and validations
        let dt_signature = self.construct_data_type_signature(&value.type_name);
        let dt = self
            .module
            .find_datatype(&dt_signature)
            .ok_or_else(|| format!("Data type not found: {}", &dt_signature))?;

        let dt_public_size = dt.public_size();
        if !value.byte_len().is_multiple_of(dt_public_size) {
            return Err(format!(
                "The value cannot be split into elements of specified type '{}'. Type size {}, data size {}.",
                dt_signature, dt_public_size, value.byte_len()));
        }
        let element_count = value.byte_len() / dt_public_size;
        let total_private_bytes = dt.private_size() * element_count;

        let node_count = self.node_count();
        assert_eq!(
            node_count,
            self.pd_data_facility.node_configurations().len()
        );

        // Construct output buffers for the classified value components
        let mut classified_value_components = vec![None; node_count];
        let mut refs = vec![
            SharemindControllerReference {
                internal: null(),
                pData: null_mut(),
                size: 0,
            };
            node_count + 1 // last one is null-terminator
        ];

        for (ref_, node_conf) in refs
            .iter_mut()
            .take(node_count)
            .zip(self.pd_data_facility.node_configurations())
        {
            // Miner numbers have been validated in PdDataFacility::register_pd_nodes()
            assert!(node_conf.miner_number > 0 && node_conf.miner_number <= node_count);
            let miner_index = node_conf.miner_number - 1;

            let mut cvalue = Value {
                pd_name: value.pd_name.clone(),
                type_name: value.type_name.clone(),
                data: vec![0; total_private_bytes], // internal buffer address remains stable
            };

            ref_.pData = cvalue.data.as_mut_ptr() as *mut c_void;
            ref_.size = total_private_bytes;
            classified_value_components[miner_index] = Some(cvalue);
        }

        // Classify public data, if any
        if element_count > 0 {
            let mut crefs = vec![
                SharemindControllerCReference {
                    internal: null(),
                    pData: null(),
                    size: 0
                };
                2 // last one is null-terminator
            ];

            crefs[0].pData = value.data.as_ptr() as *const c_void;
            crefs[0].size = value.byte_len();

            dt.classify(self, Some(&refs), &crefs)
                .map_err(|e| format!("Failed to classify the value: {}", e))?;
        }

        Ok(classified_value_components
            .into_iter()
            .map(Option::unwrap)
            .collect())
    }

    pub fn declassify(&self, value_components: &[&Value]) -> Result<Value, String> {
        if !self.is_started() {
            return Err("Protection domain not started yet".to_owned());
        }

        let node_count = self.node_count();
        if value_components.len() != node_count {
            return Err(
                "Number of input value components does not match the number of nodes".to_owned(),
            );
        }

        let &first = value_components.first().expect("Input must not be empty");

        // Data type calculations and validations
        let dt_signature = self.construct_data_type_signature(&first.type_name);
        let dt = self
            .module
            .find_datatype(&dt_signature)
            .ok_or_else(|| format!("Data type not found: {}", &dt_signature))?;

        let total_private_bytes = first.byte_len();
        let dt_private_size = dt.private_size();
        if total_private_bytes % dt_private_size != 0 {
            return Err(format!(
                "The first value cannot be split into elements of specified type '{}'. Type private size {}, data size {}.",
                dt_signature, dt_private_size, total_private_bytes));
        }
        let element_count = total_private_bytes / dt_private_size;
        let total_public_bytes = element_count * dt.public_size();

        assert_eq!(
            node_count,
            self.pd_data_facility.node_configurations().len()
        );

        let mut crefs = vec![
            SharemindControllerCReference {
                internal: null(),
                pData: null_mut(),
                size: 0,
            };
            node_count + 1 // last one is null-terminator
        ];

        for (i, (cref, node_conf)) in crefs
            .iter_mut()
            .take(node_count)
            .zip(self.pd_data_facility.node_configurations())
            .enumerate()
        {
            let miner_index: usize = node_conf.miner_number - 1;
            assert!(miner_index < node_count);
            let cvalue = value_components[miner_index];

            // Validate the value components by comparing them to the first one
            if i > 0 {
                if first.pd_name != cvalue.pd_name {
                    return Err(
                        "The classified value components belong to different protection domains"
                            .to_owned(),
                    );
                }

                if first.type_name != cvalue.type_name {
                    return Err("The classified value components are not the same type".to_owned());
                }

                if total_private_bytes != cvalue.byte_len() {
                    return Err("The classified value components are not the same size".to_owned());
                }
            }

            // Set the constant references for the declassify syscall
            cref.pData = cvalue.data.as_ptr() as *mut c_void;
            cref.size = cvalue.byte_len();
        }

        // Declassify private data, if any
        let mut value = Value {
            pd_name: first.pd_name.clone(),
            type_name: first.type_name.clone(),
            data: vec![0; total_public_bytes],
        };

        if element_count > 0 {
            let mut refs = vec![
                SharemindControllerReference {
                    internal: null(),
                    pData: null_mut(),
                    size: 0
                };
                2 // last one is null-terminator
            ];

            refs[0].pData = value.data.as_mut_ptr() as *mut c_void;
            refs[0].size = value.byte_len();

            dt.declassify(self, None, Some(&refs), Some(&crefs))
                .map_err(|e| format!("Failed to declassify the value: {}", e))?;
        }

        Ok(value)
    }

    pub fn classify_protocol(
        &self,
        pd_typename: &str,
        plaintext_data: &[u8],
    ) -> Result<(), String> {
        if !self.is_started() {
            return Err("Protection domain not started yet".to_owned());
        }
        if self.pd_network_facility.is_none() {
            return Err("Network facility not found in the protocol mode".to_owned());
        }

        // Data type calculations and validations
        let dt_signature = self.construct_data_type_signature(pd_typename);
        let dt = self
            .module
            .find_datatype(&dt_signature)
            .ok_or_else(|| format!("Data type not found: {}", &dt_signature))?;

        let dt_public_size = dt.public_size();
        if !plaintext_data.len().is_multiple_of(dt_public_size) {
            return Err(format!(
                "The value cannot be split into elements of specified type '{}'. Type size {}, data size {}.",
                dt_signature, dt_public_size, plaintext_data.len()));
        }
        let element_count = plaintext_data.len() / dt_public_size;

        // Classify public data, if any
        if element_count > 0 {
            let mut crefs = vec![
                SharemindControllerCReference {
                    internal: null(),
                    pData: null(),
                    size: 0
                };
                2 // last one is null-terminator
            ];

            crefs[0].pData = plaintext_data.as_ptr() as *const c_void;
            crefs[0].size = plaintext_data.len();

            tracing::debug!("Entering the classify call");
            dt.classify(self, None, &crefs)
                .map_err(|e| format!("Failed to classify the value: {}", e))?;
        }

        Ok(())
    }

    pub fn declassify_protocol<W: Write>(
        &self,
        pd_typename: &str,
        writer: &mut W,
    ) -> Result<(), String> {
        if !self.is_started() {
            return Err("Protection domain not started yet".to_owned());
        }
        if self.pd_network_facility.is_none() {
            return Err("Network facility not found in the protocol mode".to_owned());
        }

        // Data type calculations and validations
        let dt_signature = self.construct_data_type_signature(pd_typename);
        let dt = self
            .module
            .find_datatype(&dt_signature)
            .ok_or_else(|| format!("Data type not found: {}", &dt_signature))?;

        // We don't know how much data the protocol will receive to allocate big
        // enough output buffer.
        // Therefore we let the protocol write the output to the provided callback.
        let mut callback_ctx = WriteCallbackCtx::new(writer);
        let (write_cb, write_ctx) = callback_ctx.as_ffi_callback();

        let args = [
            SharemindCodeBlock { cp: [write_cb] },
            SharemindCodeBlock { p: [write_ctx] },
        ];

        tracing::debug!("Entering the declassify call");
        dt.declassify(self, Some(&args), None, None)
            .map_err(|e| format!("Failed to declassify the value: {}", e))?;

        Ok(())
    }
}

impl Drop for ProtectionDomain {
    fn drop(&mut self) {
        self.stop();
        unsafe { SharemindControllerPd_free(self.handle()) };
    }
}

fn set_pd_facility<T, F: Facility<T>>(
    pd: &NonNull<SharemindControllerPd>,
    facility: &mut F,
) -> Result<(), String> {
    controller_module_api_err!(
        unsafe {
            SharemindControllerPd_setFacility(
                pd.as_ptr(),
                facility.name().as_ptr(),
                facility.inner() as *mut c_void,
                null_mut(),
            )
        },
        unsafe { SharemindControllerPd_lastErrorString(pd.as_ptr()) }
    )
    .map_err(|e| {
        format!(
            "Failed to set {} facility: {}",
            facility.name().to_string_lossy(),
            e
        )
    })
}
