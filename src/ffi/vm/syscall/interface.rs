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

use std::{
    ffi::{CStr, FromBytesWithNulError},
    slice,
    str::from_utf8,
};

use crate::{
    ffi::{
        bindings::{
            SharemindCodeBlock, SharemindModuleApi0x1CReference, SharemindModuleApi0x1Reference,
            SharemindModuleApi0x1SyscallContext,
        },
        vm::syscall::error::ModuleApiFinderError,
    },
    syserr,
};

use super::error::{SyscallError, SyscallErrorKind};

#[derive(Debug)]
pub enum ModuleApiReferenceError {
    FromBytesWithNulError(FromBytesWithNulError),
    NotNullTerminated,
    Utf8Error(std::str::Utf8Error),
}

impl std::error::Error for ModuleApiReferenceError {}

impl std::fmt::Display for ModuleApiReferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleApiReferenceError::FromBytesWithNulError(e) => write!(f, "{e}"),
            ModuleApiReferenceError::NotNullTerminated => {
                write!(f, "Referenced data is not null terminated")
            }
            ModuleApiReferenceError::Utf8Error(e) => write!(f, "{e}"),
        }
    }
}

impl From<FromBytesWithNulError> for ModuleApiReferenceError {
    fn from(value: FromBytesWithNulError) -> Self {
        ModuleApiReferenceError::FromBytesWithNulError(value)
    }
}

impl From<std::str::Utf8Error> for ModuleApiReferenceError {
    fn from(value: std::str::Utf8Error) -> Self {
        ModuleApiReferenceError::Utf8Error(value)
    }
}

pub trait ModuleApiReference {
    // Interpret the referenced data as a byte slice
    fn as_bytes(&self) -> &[u8];

    // Returns the referenced data as a C compatible string.
    // Checks that the string contains exactly one null terminator at the end.
    // Fails if the underlying byte array is empty, i.e. referenced data size is 0.
    #[allow(dead_code)]
    fn to_cstr(&self) -> Result<&CStr, ModuleApiReferenceError> {
        Ok(CStr::from_bytes_with_nul(self.as_bytes())?)
    }

    // Returns the referenced data as an UTF-8 string slice.
    // Fails if the underlying byte array is not null terminated (or is empty).
    fn to_str(&self) -> Result<&str, ModuleApiReferenceError> {
        let bytes = self.as_bytes();
        if bytes.last() != Some(&b'\0') {
            return Err(ModuleApiReferenceError::NotNullTerminated);
        }

        // Rust strings are not null-terminated.
        Ok(from_utf8(&bytes[..bytes.len() - 1])?)
    }

    // Checks if the reference does not point to any data, i.e. is a terminator.
    fn is_terminator(&self) -> bool;
}

impl ModuleApiReference for SharemindModuleApi0x1Reference {
    fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.pData as *const u8, self.size) }
    }

    fn is_terminator(&self) -> bool {
        self.pData.is_null()
    }
}

impl ModuleApiReference for SharemindModuleApi0x1CReference {
    fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.pData as *const u8, self.size) }
    }

    fn is_terminator(&self) -> bool {
        self.pData.is_null()
    }
}

fn length_of_refs_array<T>(refs_ptr: *const T) -> usize
where
    T: ModuleApiReference,
{
    let mut current_idx: usize = 0;
    loop {
        unsafe {
            if (*refs_ptr.add(current_idx)).is_terminator() {
                break;
            }
        }
        current_idx += 1;
    }
    current_idx
}

// The final terminating element is not included in the returned slice.
pub fn slice_from_refs_ptr<'a, T>(refs_ptr: *const T) -> &'a [T]
where
    T: ModuleApiReference,
{
    if refs_ptr.is_null() {
        return &[];
    }
    unsafe { slice::from_raw_parts(refs_ptr, length_of_refs_array(refs_ptr)) }
}

pub fn slice_from_args_ptr<'a>(
    args_ptr: *const SharemindCodeBlock,
    args_count: usize,
) -> &'a [SharemindCodeBlock] {
    if args_ptr.is_null() {
        &[]
    } else {
        unsafe { slice::from_raw_parts(args_ptr, args_count) }
    }
}

impl SharemindModuleApi0x1SyscallContext {
    /// TODO:
    /// Linter complains that we return mutable outputs from immutable inputs.
    /// We call a function pointer that returns a mutable object owned by C code.
    /// Probably returning &mut is a too strong guarantee.
    /// Rust cannot be sure about the behavior of C code regarding that object.
    /// Option 1: Return NonNull<T> and let users decide about safety of dereferencing.
    /// Option 2: Return a T wrapper that encapsulates unsafe operations and exposes a safe API.
    /// Option 3: Possibly could return UnsafeCell<T>, not sure if that is good.
    /// Ignore the lint for now until we have time to properly refactor this.
    #[allow(clippy::mut_from_ref)]
    pub fn get_process_facility<T>(&self, name: &CStr) -> Result<&mut T, SyscallError> {
        const context: &str = "Process facility";
        let process_facility_fn = self
            .processFacility
            .ok_or(ModuleApiFinderError::NullFunctionPointer(context))?;
        let facility_ptr = unsafe { process_facility_fn(self, name.as_ptr()) } as *mut T;

        Ok(unsafe { facility_ptr.as_mut() }
            .ok_or_else(|| ModuleApiFinderError::NotFound(context, name.to_owned()))?)
    }
}

pub struct SyscallRule {
    pub num_args: usize,
    pub num_refs: usize,
    pub num_crefs: usize,
    pub need_return_value: bool,
}

impl SyscallRule {
    pub fn applies(
        &self,
        args: &[SharemindCodeBlock],
        refs: &[SharemindModuleApi0x1Reference],
        crefs: &[SharemindModuleApi0x1CReference],
        return_value: *mut SharemindCodeBlock,
    ) -> bool {
        if self.num_args != args.len() {
            return false;
        }

        if self.num_refs != refs.len() {
            return false;
        }

        if self.num_crefs != crefs.len() {
            return false;
        }

        if self.need_return_value && return_value.is_null() {
            return false;
        }

        true
    }
}

pub fn validate_syscall(
    args: &[SharemindCodeBlock],
    refs: &[SharemindModuleApi0x1Reference],
    crefs: &[SharemindModuleApi0x1CReference],
    return_value: *mut SharemindCodeBlock,
    valid_syscall_rules: &[SyscallRule],
) -> Result<(), SyscallError> {
    let any_rule_applies = valid_syscall_rules
        .iter()
        .any(|rule| rule.applies(args, refs, crefs, return_value));

    if any_rule_applies {
        Ok(())
    } else {
        Err(syserr!(
            SyscallErrorKind::InvalidCall,
            "syscall validation failed"
        ))
    }
}

#[macro_export]
macro_rules! ref_to_cstr_or_err {
    ($ref:expr) => {
        $crate::ffi::vm::syscall::interface::ModuleApiReference::to_cstr(&$ref).map_err(|e| {
            $crate::syserr!(
                $crate::ffi::vm::syscall::error::SyscallErrorKind::InvalidCall,
                format!("invalid C string in {}", std::stringify!($ref)),
                e
            )
        })?
    };
}

#[macro_export]
macro_rules! ref_to_str_or_err {
    ($ref:expr) => {
        $crate::ffi::vm::syscall::interface::ModuleApiReference::to_str(&$ref).map_err(|e| {
            $crate::syserr!(
                $crate::ffi::vm::syscall::error::SyscallErrorKind::InvalidCall,
                format!("invalid UTF-8 string in {}", std::stringify!($ref)),
                e
            )
        })?
    };
}
