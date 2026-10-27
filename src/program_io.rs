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

use std::collections::HashMap;

use crate::ffi::primitive_types::{VmData, VmDataView, VmType};

pub mod streams;

#[derive(Clone)]
pub struct ProgramArgument {
    pub data: Vec<u8>,
}

impl From<VmData> for ProgramArgument {
    fn from(data: VmData) -> Self {
        Self {
            data: data.into_byte_vec(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgramResult {
    pub pd: String,
    pub type_: String,
    pub data: Vec<u8>,
}

impl ProgramResult {
    pub fn is_public(&self) -> bool {
        self.pd.is_empty() || self.pd == "public"
    }

    pub fn as_vm_data(&'_ self) -> Result<VmDataView<'_>, String> {
        VmDataView::try_from(self)
    }
}

/// Conversion of results into data of VM primitive types.
///
/// Data types in the public protection domain match the VM primitive types.
/// Other protection domains can have data types with different names and
/// data representations. For these we first try to match the result type with
/// the VM type and, if unsuccessful, then just treat it as an oblivious byte array.
impl<'a> TryFrom<&'a ProgramResult> for VmDataView<'a> {
    type Error = String;

    fn try_from(result: &'a ProgramResult) -> Result<Self, Self::Error> {
        let vm_type = if let Some(vm_type) = VmType::from_name(&result.type_) {
            vm_type
        } else if result.is_public() {
            return Err(format!(
                "Result type {} is incompatible with VmType",
                &result.type_
            ));
        } else {
            VmType::Uint8
        };

        VmDataView::new(vm_type, result.data.as_slice())
    }
}

pub type ProgramArgumentMap = HashMap<String, ProgramArgument>;

pub type ProgramResultMap = HashMap<String, ProgramResult>;
