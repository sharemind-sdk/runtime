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

pub mod cache;
pub(crate) mod data_store;
pub(crate) mod error;
pub(crate) mod interface;

use error::SyscallError;
use interface::{slice_from_args_ptr, slice_from_refs_ptr};

use crate::ffi::bindings::{
    FinderMut, SharemindCodeBlock, SharemindModuleApi0x1CReference, SharemindModuleApi0x1Error,
    SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_INVALID_CALL,
    SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_OK, SharemindModuleApi0x1Reference,
    SharemindModuleApi0x1SyscallContext, SharemindSyscallWrapper,
};

pub type SharemindModuleApi0x1SyscallImplFn = fn(
    args: &[SharemindCodeBlock],
    refs: &[SharemindModuleApi0x1Reference],
    crefs: &[SharemindModuleApi0x1CReference],
    ret_val: *mut SharemindCodeBlock,
    c: &SharemindModuleApi0x1SyscallContext,
) -> Result<(), SyscallError>;

/// # Safety
/// This function is unsafe because it works on raw pointers that come from VM.
/// See: https://rust-lang.github.io/rust-clippy/master/index.html#not_unsafe_ptr_arg_deref
pub unsafe fn syscall_adapter(
    args: *mut SharemindCodeBlock,
    num_args: usize,
    refs: *const SharemindModuleApi0x1Reference,
    crefs: *const SharemindModuleApi0x1CReference,
    ret_val: *mut SharemindCodeBlock,
    c: *mut SharemindModuleApi0x1SyscallContext,
    syscall_impl: SharemindModuleApi0x1SyscallImplFn,
) -> SharemindModuleApi0x1Error {
    // Shadow original pointers with slices
    let args = slice_from_args_ptr(args, num_args);
    let refs = slice_from_refs_ptr(refs);
    let crefs = slice_from_refs_ptr(crefs);

    // Shadow original context pointer with a reference
    let c = match c.as_ref() {
        Some(c) => c,
        None => {
            tracing::error!("Syscall error: context pointer is null");
            return SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_INVALID_CALL;
        }
    };

    match syscall_impl(args, refs, crefs, ret_val, c) {
        Ok(_) => SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_OK,
        Err(e) => {
            tracing::error!("Syscall error: {e}");
            e.kind.clone().into()
        }
    }
}

/// Generate a syscall method using the given method name and implementation.
/// The attributes are also captured and associated with the method. This allows
/// properly capturing doc comments for the syscall.
#[macro_export]
macro_rules! sharemind_module_api_0x1_syscall {
    ($(#[$attr:meta])* $name:ident, $syscall_impl:expr) => {
        $(#[$attr])*
        /// # Safety
        /// This function is unsafe because it works on raw pointers that come from VM.
        pub unsafe extern "C" fn $name(
            args: *mut $crate::ffi::bindings::SharemindCodeBlock,
            num_args: usize,
            refs: *const $crate::ffi::bindings::SharemindModuleApi0x1Reference,
            crefs: *const $crate::ffi::bindings::SharemindModuleApi0x1CReference,
            ret_val: *mut $crate::ffi::bindings::SharemindCodeBlock,
            c: *mut $crate::ffi::bindings::SharemindModuleApi0x1SyscallContext,
        ) -> $crate::ffi::bindings::SharemindModuleApi0x1Error {
            $crate::ffi::vm::syscall::syscall_adapter(
                args,
                num_args,
                refs,
                crefs,
                ret_val,
                c,
                $syscall_impl,
            )
        }
    };
}

pub fn find_syscall<'a>(
    ctx: &'a SharemindModuleApi0x1SyscallContext,
    name: &'a std::ffi::CStr,
) -> Result<&'a SharemindSyscallWrapper, SyscallError> {
    let syscall_finder = ctx.get_process_facility::<FinderMut>(c"SyscallFinder")?;
    let syscall_finder_fn =
        syscall_finder
            .find
            .ok_or(error::ModuleApiFinderError::NullFunctionPointer(
                "Syscall finder",
            ))?;

    let syscall_ptr = unsafe {
        syscall_finder_fn(syscall_finder as *const _, name.as_ptr()) as *mut SharemindSyscallWrapper
    };
    let syscall = unsafe { syscall_ptr.as_ref() }
        .ok_or_else(|| error::ModuleApiFinderError::NotFound("Syscall", name.to_owned()))?;
    Ok(syscall)
}
