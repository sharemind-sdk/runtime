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

use std::ffi::CStr;

use crate::ffi::bindings::{SharemindTdbString, SharemindTdbType, SharemindTdbValue};

impl SharemindTdbType {
    pub fn is_variable_length(&self) -> bool {
        self.size == 0
    }

    pub fn domain_str(&self) -> &str {
        assert!(!self.domain.is_null(), "type domain must not be null");
        unsafe { CStr::from_ptr(self.domain).to_str().unwrap() }
    }

    pub fn name_str(&self) -> &str {
        assert!(!self.name.is_null(), "type name must not be null");
        unsafe { CStr::from_ptr(self.name).to_str().unwrap() }
    }

    pub fn is_valid(&self) -> Result<(), &'static str> {
        if self.domain.is_null() {
            return Err("type domain is null");
        }

        if self.name.is_null() {
            return Err("type name is null");
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TdbType {
    pub domain: String,
    pub name: String,
    pub size: u64,
}

impl TdbType {
    pub fn is_variable_length(&self) -> bool {
        self.size == 0
    }
}

impl PartialEq<SharemindTdbType> for TdbType {
    fn eq(&self, other: &SharemindTdbType) -> bool {
        self.domain == other.domain_str()
            && self.name == other.name_str()
            && self.size == other.size
    }
}

#[allow(dead_code)]
pub(crate) fn type_element_size(name: &str) -> usize {
    match name {
        "int8" => 1,
        "int16" => 2,
        "int32" => 4,
        "int64" => 8,
        "uint8" => 1,
        "uint16" => 2,
        "uint32" => 4,
        "uint64" => 8,
        "float32" => 4,
        "float64" => 8,
        "string" => 1,
        _ => panic!("Unsupported data type: {name}. Possibly need to add the missing type in `type_element_size()`."),
    }
}

impl SharemindTdbValue {
    /// Returns a slice of bytes representing the value.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the `buffer` and `size` fields of the
    /// `SharemindTdbValue` are valid and not null before calling this method.
    /// The returned slice will point to memory that `buffer` points to.
    /// While the method checks for a null-pointer, rust is unable to guarantee
    /// that the memory pointed to by `buffer` is actually valid.
    pub unsafe fn as_bytes(&self) -> &[u8] {
        if self.type_as_ref().is_variable_length() && self.buffer.is_null() {
            assert_eq!(
                self.size, 0,
                "variable-length buffer is null but size not 0"
            );
            &[]
        } else {
            assert!(!self.buffer.is_null(), "buffer pointer must not be null");
            std::slice::from_raw_parts(self.buffer as *const u8, self.size as usize)
        }
    }

    /// Returns a reference to the type of this value.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the `type_` field of the `SharemindTdbValue`
    /// is valid and not null before calling this method.
    /// While the method checks for a null-pointer, rust is unable to guarantee
    /// that the memory pointed to by `type_` is actually valid.
    pub unsafe fn type_as_ref(&self) -> &SharemindTdbType {
        self.type_
            .as_ref()
            .expect("value type pointer must not be null")
    }

    pub fn is_valid(&self) -> bool {
        // Check that the type exists
        if self.type_.is_null() {
            tracing::error!("Invalid value: Type must not be null.");
            return false;
        }

        let value_type = unsafe { self.type_as_ref() };

        // Validate value type
        if let Err(e) = value_type.is_valid() {
            tracing::error!("Invalid value type: {}.", e);
            return false;
        }

        if value_type.is_variable_length() {
            // Variable length types are handled differently
            // e.g. data can be empty
            return true;
        } else {
            // Fixed-size values must have at least one element

            // Check that the value is non-empty
            if self.size == 0 {
                tracing::error!(
                    "Invalid value of type \"{}::{}\": Value size must be greater than zero.",
                    value_type.domain_str(),
                    value_type.name_str(),
                );
                return false;
            }

            // Check that the value buffer length is a multiple of the type size length
            if !self.size.is_multiple_of(value_type.size) {
                tracing::error!(
                "Invalid value of type \"{}::{}\": Value size must be a multiple of its type size.",
                value_type.domain_str(),
                value_type.name_str(),
            );
                return false;
            }

            // Check that buffer exists
            if self.buffer.is_null() {
                tracing::error!(
                    "Invalid value of type \"{}::{}\": Buffer must not be null.",
                    value_type.domain_str(),
                    value_type.name_str(),
                );
                return false;
            }
        }

        true
    }

    // Number of elements the value buffer represents given the value type.
    pub fn element_count(&self) -> usize {
        let value_type = unsafe { self.type_as_ref() };
        if value_type.is_variable_length() {
            // For variable length types we do not support arrays.
            // The buffer is the value.
            1
        } else {
            assert_ne!(value_type.size, 0, "value type size must not be zero");
            assert_eq!(
                self.size % value_type.size,
                0,
                "value size must be a multiple of its type size"
            );
            const_assert_eq!(size_of::<u64>(), size_of::<usize>());
            (self.size / value_type.size) as usize
        }
    }

    // Returns a byte array representing an element in the buffer at given index
    pub fn get_element(&self, idx: usize) -> &[u8] {
        let value_type = unsafe { self.type_as_ref() };
        let buffer = unsafe { self.as_bytes() };

        if value_type.is_variable_length() {
            assert_eq!(idx, 0);
            buffer
        } else {
            const_assert_eq!(size_of::<u64>(), size_of::<usize>());
            let element_size = value_type.size as usize;

            assert!(idx < self.element_count());
            let start = idx * element_size;
            let end = start + element_size;
            &buffer[start..end]
        }
    }
}

impl SharemindTdbString {
    pub fn as_str(&self) -> &str {
        assert!(!self.str_.is_null(), "string must not be null");
        unsafe { CStr::from_ptr(self.str_).to_str().unwrap() }
    }
}
