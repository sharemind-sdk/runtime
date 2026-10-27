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

use rand::{distr::Alphanumeric, Rng};
use strum::EnumIter;

// -----------------------------------------------------------------------------
// VM Type Definition
// -----------------------------------------------------------------------------

/// Representation of primitive data types supported by the Virtual Machine
#[derive(Clone, Copy, Debug, EnumIter, PartialEq)]
pub enum VmType {
    // Boolean (stored as 1 byte: 0 or 1)
    Bool,
    // Fixed-size signed integers
    Int8,
    Int16,
    Int32,
    Int64,
    // Fixed-size unsigned integers
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    // Fixed-size floating point numbers
    Float32,
    Float64,
    // Variable-length types
    String,
}

impl VmType {
    /// Creates the primitive type based on its name in VM
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "bool" => Some(Self::Bool),
            "int8" => Some(Self::Int8),
            "int16" => Some(Self::Int16),
            "int32" => Some(Self::Int32),
            "int64" | "int" => Some(Self::Int64),
            "uint8" => Some(Self::Uint8),
            "uint16" => Some(Self::Uint16),
            "uint32" => Some(Self::Uint32),
            "uint64" | "uint" => Some(Self::Uint64),
            "float32" | "float" => Some(Self::Float32),
            "float64" => Some(Self::Float64),
            "string" => Some(Self::String),
            _ => None,
        }
    }

    /// Returns the name of the primitive type in VM
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Int8 => "int8",
            Self::Int16 => "int16",
            Self::Int32 => "int32",
            Self::Int64 => "int64",
            Self::Uint8 => "uint8",
            Self::Uint16 => "uint16",
            Self::Uint32 => "uint32",
            Self::Uint64 => "uint64",
            Self::Float32 => "float32",
            Self::Float64 => "float64",
            Self::String => "string",
        }
    }

    /// For fixed-size types, returns the number of bytes required to store one value.
    /// For variable-length types returns None.
    pub fn type_size(&self) -> Option<usize> {
        match self {
            Self::Int8 | Self::Uint8 | Self::Bool => Some(1),
            Self::Int16 | Self::Uint16 => Some(2),
            Self::Int32 | Self::Uint32 | Self::Float32 => Some(4),
            Self::Int64 | Self::Uint64 | Self::Float64 => Some(8),
            Self::String => None, // variable-length type
        }
    }

    /// The base size in bytes (i.e. the size of the most basic element of the type).
    /// For fixed-size types, it is the same as type_size.
    /// For String, the base element is a byte (one ASCII character)
    pub fn base_size(&self) -> usize {
        match self {
            // Variable-length types
            Self::String => 1,
            // Fixed-size types
            _ => self.type_size().expect("Fixed-size type must have size"),
        }
    }

    pub fn is_variable_length(&self) -> bool {
        match self {
            // Variable-length types
            VmType::String => true,
            // Fixed-size types
            _ => false,
        }
    }
}

impl std::fmt::Display for VmType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.type_name())
    }
}

// -----------------------------------------------------------------------------
// Rust Type metadata and mappings into VM Type
// -----------------------------------------------------------------------------

/// A marker trait for fixed-size Rust types
pub trait VmFixedSize: Sized {
    const SIZE: usize = std::mem::size_of::<Self>(); // For the fixed-size representation
}

impl VmFixedSize for bool {}
impl VmFixedSize for i8 {}
impl VmFixedSize for i16 {}
impl VmFixedSize for i32 {}
impl VmFixedSize for i64 {}
impl VmFixedSize for u8 {}
impl VmFixedSize for u16 {}
impl VmFixedSize for u32 {}
impl VmFixedSize for u64 {}
impl VmFixedSize for f32 {}
impl VmFixedSize for f64 {}

/// A marker trait for variable-length Rust types
pub trait VmVariableLength {}

impl VmVariableLength for String {}

/// This trait links a Rust type to its corresponding VM type.
pub trait VmTyped {
    fn vm_type() -> VmType;
}

macro_rules! impl_vmtype_mapping {
    ($rust_type:ty, $vm_type:expr) => {
        impl VmTyped for $rust_type {
            fn vm_type() -> VmType {
                $vm_type
            }
        }
    };
}

impl_vmtype_mapping!(bool, VmType::Bool);
impl_vmtype_mapping!(i8, VmType::Int8);
impl_vmtype_mapping!(i16, VmType::Int16);
impl_vmtype_mapping!(i32, VmType::Int32);
impl_vmtype_mapping!(i64, VmType::Int64);
impl_vmtype_mapping!(u8, VmType::Uint8);
impl_vmtype_mapping!(u16, VmType::Uint16);
impl_vmtype_mapping!(u32, VmType::Uint32);
impl_vmtype_mapping!(u64, VmType::Uint64);
impl_vmtype_mapping!(f32, VmType::Float32);
impl_vmtype_mapping!(f64, VmType::Float64);
impl_vmtype_mapping!(String, VmType::String);

// -----------------------------------------------------------------------------
// VM Data Representation (Owned and Borrowed)
// -----------------------------------------------------------------------------

/// Trait with common functionality for owned and borrowed VM data
pub trait VmDataRef: Sized {
    /// Returns the VM type of the data
    fn vm_type(&self) -> VmType;
    /// Returns a reference to the underlying byte slice
    fn as_bytes(&self) -> &[u8];

    /// Returns true if no data is stored
    fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }

    /// Returns the number of VM type values stored.
    /// For fixed-size types this is the number of values of VM type size.
    /// For variable-length types (e.g. String) we consider the entire data as one value.
    fn len(&self) -> usize {
        let vm_type = self.vm_type();

        if vm_type.is_variable_length() {
            1
        } else {
            let type_size = vm_type.type_size().expect("Fixed-size type must have size");
            let byte_len = self.as_bytes().len();
            assert_eq!(byte_len % type_size, 0);
            byte_len / type_size
        }
    }

    /// Returns the raw byte slice representing the value stored at the given index.
    /// For variable-length types a single value is stored and returned.
    fn get_raw_at(&self, index: usize) -> Option<&[u8]> {
        let vm_type = self.vm_type();
        let bytes = self.as_bytes();

        if vm_type.is_variable_length() {
            if index == 0 {
                Some(self.as_bytes())
            } else {
                None
            }
        } else {
            let type_size = vm_type.type_size().expect("Fixed-size type must have size");
            let start = index.checked_mul(type_size)?;
            let end = start.checked_add(type_size)?;
            if end <= bytes.len() {
                Some(&bytes[start..end])
            } else {
                None
            }
        }
    }

    //----------------------------------------------
    // Helpers for converting VM data to Rust types
    //----------------------------------------------

    /// Returns the value at the given index converted into a Rust type.
    /// Delegates to the indexed conversion in FromVmDataRef.
    fn get_value<T>(&self, index: usize) -> Result<T, String>
    where
        T: FromVmDataRef,
    {
        T::from_vm_data_indexed(self, index)
    }

    /// Converts the entire VM data into a single Rust value.
    /// Delegates to the single-value conversion in FromVmDataRef.
    /// This method is applicable when the underlying buffer is expected to hold exactly one value.
    fn to_value<T>(&self) -> Result<T, String>
    where
        T: FromVmDataRef,
    {
        T::from_vm_data(self)
    }

    /// Converts the entire VM data into a vector of Rust values for fixed-size types.
    /// Delegates to the indexed conversion in FromVmDataRef.
    /// Note: Vector conversion for strings is not needed. Thus conversion limited to fixed-size types.
    fn to_value_vec<T>(&self) -> Result<Vec<T>, String>
    where
        T: FromVmDataRef + VmFixedSize,
    {
        assert!(!self.vm_type().is_variable_length()); // method is bound by VmFixedSize

        let count = self.len();
        let vec = (0..count)
            .map(|i| T::from_vm_data_indexed(self, i))
            .collect::<Result<Vec<_>, String>>()?;
        Ok(vec)
    }
}

/// The owned version of VM data
#[derive(Clone, Debug, PartialEq)]
pub struct VmData {
    vm_type: VmType,
    raw_data: Vec<u8>,
}

impl VmData {
    /// Construct an owned VmData from a VM type and a byte buffer.
    pub fn new(vm_type: VmType, raw_data: Vec<u8>) -> Result<Self, String> {
        if raw_data.len().is_multiple_of(vm_type.base_size()) {
            Ok(Self { vm_type, raw_data })
        } else {
            Err(format!(
                "Input data length {} is incompatible with data type {}",
                raw_data.len(),
                vm_type.type_name()
            ))
        }
    }

    /// Generates random VM data of the given type and the specified number of values.
    /// For String the count is the number of base_type elements, i.e. length of the string.
    pub fn random(vm_type: VmType, count: usize) -> Self {
        let data = match vm_type {
            VmType::Bool => convert_into_vmdata(random_fixed::<bool>(count)),
            VmType::Int8 => convert_into_vmdata(random_fixed::<i8>(count)),
            VmType::Int16 => convert_into_vmdata(random_fixed::<i16>(count)),
            VmType::Int32 => convert_into_vmdata(random_fixed::<i32>(count)),
            VmType::Int64 => convert_into_vmdata(random_fixed::<i64>(count)),
            VmType::Uint8 => convert_into_vmdata(random_fixed::<u8>(count)),
            VmType::Uint16 => convert_into_vmdata(random_fixed::<u16>(count)),
            VmType::Uint32 => convert_into_vmdata(random_fixed::<u32>(count)),
            VmType::Uint64 => convert_into_vmdata(random_fixed::<u64>(count)),
            VmType::Float32 => convert_into_vmdata(random_fixed::<f32>(count)),
            VmType::Float64 => convert_into_vmdata(random_fixed::<f64>(count)),
            VmType::String => {
                let s: String = rand::rng()
                    .sample_iter(&Alphanumeric)
                    .take(count)
                    .map(char::from)
                    .collect();
                convert_into_vmdata(s)
            }
        }
        .expect("Generated random data shall be compatible with VmData");

        assert_eq!(data.vm_type, vm_type);
        data
    }

    /// Convert VM data into a byte vector, consuming the original.
    pub fn into_byte_vec(self) -> Vec<u8> {
        self.raw_data
    }

    /// Extend the VM data with input byte buffer
    pub fn extend_from_bytes(&mut self, raw_data: &[u8]) -> Result<(), String> {
        if raw_data.len().is_multiple_of(self.vm_type.base_size()) {
            self.raw_data.extend_from_slice(raw_data);
            Ok(())
        } else {
            Err(format!(
                "Input data length {} is incompatible with data type {}",
                raw_data.len(),
                self.vm_type.type_name()
            ))
        }
    }
}

/// Helper function for random data generation for fixed-size types.
fn random_fixed<T>(count: usize) -> Vec<T>
where
    rand::distr::StandardUniform: rand::distr::Distribution<T>,
{
    let mut rng = rand::rng();
    (0..count).map(|_| rng.random::<T>()).collect::<Vec<_>>()
}

impl VmDataRef for VmData {
    fn vm_type(&self) -> VmType {
        self.vm_type
    }

    fn as_bytes(&self) -> &[u8] {
        &self.raw_data
    }
}

/// The borrowed version of VM data
#[derive(Debug, PartialEq)]
pub struct VmDataView<'a> {
    vm_type: VmType,
    raw_data: &'a [u8],
}

impl<'a> VmDataView<'a> {
    /// Constructs a new VmDataView from a VM type and a borrowed byte slice
    pub fn new(vm_type: VmType, raw_data: &'a [u8]) -> Result<Self, String> {
        if raw_data.len().is_multiple_of(vm_type.base_size()) {
            Ok(Self { vm_type, raw_data })
        } else {
            Err(format!(
                "Data length {} is incompatible with data type {}",
                raw_data.len(),
                vm_type.type_name()
            ))
        }
    }
}

impl VmDataRef for VmDataView<'_> {
    fn vm_type(&self) -> VmType {
        self.vm_type
    }

    fn as_bytes(&self) -> &[u8] {
        self.raw_data
    }
}

// -----------------------------------------------------------------------------
// Cross-comparison of Owned and Borrowed VM data
// -----------------------------------------------------------------------------

/// Compare two types that implement VmDataRef based on their VM type and the underlying bytes.
fn vm_data_eq<A: VmDataRef, B: VmDataRef>(a: &A, b: &B) -> bool {
    a.vm_type() == b.vm_type() && a.as_bytes() == b.as_bytes()
}

impl<'a> PartialEq<VmDataView<'a>> for VmData {
    fn eq(&self, other: &VmDataView<'a>) -> bool {
        vm_data_eq(self, other)
    }
}
impl PartialEq<VmData> for VmDataView<'_> {
    fn eq(&self, other: &VmData) -> bool {
        vm_data_eq(self, other)
    }
}

// -----------------------------------------------------------------------------
// Byte conversion abstractions the Rust types
// -----------------------------------------------------------------------------

/// This trait abstracts the conversion of a Rust type to/from its byte representation.
/// Motivation: Byte conversion APIs differ across the types.
pub trait ByteConvertible: Sized {
    type ByteArray: AsRef<[u8]>; // Allows both `[u8; N]` and `Vec<u8>`

    fn to_bytes(self) -> Self::ByteArray;
    fn from_bytes(bytes: Self::ByteArray) -> Result<Self, String>;
}

/// Macro to implement ByteConvertible for fixed-size numeric types that have `from_le_bytes` and `to_le_bytes`.
macro_rules! impl_byte_convertible_fixed {
    ($($t:ty),*) => {
        $(
            impl ByteConvertible for $t {
                type ByteArray = [u8; Self::SIZE];

                fn to_bytes(self) -> Self::ByteArray {
                    self.to_le_bytes()
                }

                fn from_bytes(bytes: Self::ByteArray) -> Result<Self, String> {
                    Ok(Self::from_le_bytes(bytes))
                }
            }
        )*
    };
}
impl_byte_convertible_fixed!(i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);

/// Special-case implementation for bool.
impl ByteConvertible for bool {
    type ByteArray = [u8; 1];

    fn to_bytes(self) -> Self::ByteArray {
        [self as u8]
    }

    fn from_bytes(bytes: Self::ByteArray) -> Result<Self, String> {
        match bytes[0] {
            0 => Ok(false),
            1 => Ok(true),
            b => Err(format!(
                "Invalid byte for bool: expected 0 or 1, but got {b}"
            )),
        }
    }
}

/// Special-case implementation for variable-size `String`.
/// For String, we use `Vec<u8>` as its byte representation.
///
/// Keep strings without null-terminator, becuase in SecreC stdlib:
///  - argument() adds a null-terminator to the incoming string bytes.
///  - publish() outputs strings bytes without a null-terminator.
impl ByteConvertible for String {
    type ByteArray = Vec<u8>;

    fn to_bytes(self) -> Self::ByteArray {
        self.into_bytes()
    }

    fn from_bytes(bytes: Self::ByteArray) -> Result<Self, String> {
        String::from_utf8(bytes).map_err(|e| e.to_string())
    }
}

// -----------------------------------------------------------------------------
// Conversions from a VM Data (owned and borrowed) into Rust Types
// -----------------------------------------------------------------------------

/// This trait defines how to convert an entire VM data or an indexed element
/// into a Rust type.
pub trait FromVmDataRef: Sized {
    fn from_vm_data<D: VmDataRef>(data: &D) -> Result<Self, String>;
    fn from_vm_data_indexed<D: VmDataRef>(data: &D, index: usize) -> Result<Self, String>;
}

/// Blanket implementation of conversions for fixed-size types.
impl<T> FromVmDataRef for T
where
    T: ByteConvertible + VmTyped + VmFixedSize,
    <T as ByteConvertible>::ByteArray:
        for<'a> TryFrom<&'a [u8], Error = std::array::TryFromSliceError>,
{
    fn from_vm_data<D: VmDataRef>(data: &D) -> Result<Self, String> {
        if data.vm_type() != T::vm_type() {
            return Err(format!(
                "Type mismatch: source {}, destination {}",
                data.vm_type().type_name(),
                T::vm_type().type_name()
            ));
        }

        if data.as_bytes().len() != T::SIZE {
            return Err(format!(
                "Byte length mismatch: source {}, destination {}",
                data.as_bytes().len(),
                T::SIZE
            ));
        }

        let byte_arr = data.as_bytes().try_into().map_err(|e| {
            format!(
                "Error converting from {} data to {} value: {}",
                data.vm_type().type_name(),
                T::vm_type().type_name(),
                e
            )
        })?;

        T::from_bytes(byte_arr)
    }

    fn from_vm_data_indexed<D: VmDataRef>(data: &D, index: usize) -> Result<Self, String> {
        if data.vm_type() != T::vm_type() {
            return Err(format!(
                "Type mismatch: source {}, destination {}",
                data.vm_type().type_name(),
                T::vm_type().type_name()
            ));
        }

        let slice = data
            .get_raw_at(index)
            .ok_or("Index out of bounds".to_owned())?;

        let byte_arr = slice.try_into().map_err(|e| {
            format!(
                "Error converting from {} data at index {} to {} value: {}",
                data.vm_type().type_name(),
                index,
                T::vm_type().type_name(),
                e
            )
        })?;

        T::from_bytes(byte_arr)
    }
}

/// Specialized implementation of conversion for the String type.
impl FromVmDataRef for String {
    fn from_vm_data<D: VmDataRef>(data: &D) -> Result<Self, String> {
        if data.vm_type() != VmType::String {
            return Err(format!(
                "Type mismatch: source {}, destination String",
                data.vm_type().type_name()
            ));
        }
        String::from_utf8(data.as_bytes().to_vec()).map_err(|e| {
            format!(
                "Error converting from {} data to {} value: {}",
                data.vm_type().type_name(),
                VmType::String.type_name(),
                e
            )
        })
    }
    fn from_vm_data_indexed<D: VmDataRef>(_data: &D, _index: usize) -> Result<Self, String> {
        Err("Indexed access not supported for string".to_owned())
    }
}

// -----------------------------------------------------------------------------
// Conversions from Rust Types into DSL Data
// -----------------------------------------------------------------------------

/// This trait converts a Rust value (or vector of values) into owned VmData.
pub trait IntoVmData {
    fn into_vm_data(self) -> Result<VmData, String>;
}

/// Blanket implementation for fixed-size types.
impl<T> IntoVmData for T
where
    T: ByteConvertible + VmTyped + VmFixedSize,
{
    fn into_vm_data(self) -> Result<VmData, String> {
        VmData::new(T::vm_type(), self.to_bytes().as_ref().to_vec())
    }
}

/// Converter for String
impl IntoVmData for String {
    fn into_vm_data(self) -> Result<VmData, String> {
        VmData::new(VmType::String, self.into_bytes())
    }
}

/// Converter for &str
impl IntoVmData for &str {
    fn into_vm_data(self) -> Result<VmData, String> {
        VmData::new(VmType::String, self.as_bytes().to_vec())
    }
}

/// Blanket implementation for vectors of fixed-size types.
impl<T> IntoVmData for Vec<T>
where
    T: ByteConvertible + VmTyped + VmFixedSize,
{
    fn into_vm_data(self) -> Result<VmData, String> {
        let mut raw_data = Vec::with_capacity(self.len() * T::SIZE);
        for value in self {
            raw_data.extend_from_slice(value.to_bytes().as_ref());
        }
        VmData::new(T::vm_type(), raw_data)
    }
}

/// Helper function for converting a Rust value (or vector) into VmData.
pub fn convert_into_vmdata<T: IntoVmData>(value: T) -> Result<VmData, String> {
    value.into_vm_data()
}
