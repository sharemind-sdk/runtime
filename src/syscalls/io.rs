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

use std::{cmp, mem, ptr};

use crate::{
    ffi::{
        bindings::{RustProcess, SharemindModuleApi0x1SyscallContext},
        vm::{
            syscall::{
                error::{SyscallError, SyscallErrorKind},
                interface::{validate_syscall, ModuleApiReference, SyscallRule},
            },
            RustProcessWrapper,
        },
        wrapper::WrappedC,
    },
    program_io::ProgramResult,
    ref_to_str_or_err, sharemind_module_api_0x1_syscall, syserr,
};

fn get_rust_process_wrapper(
    ctx: &SharemindModuleApi0x1SyscallContext,
) -> Result<&RustProcessWrapper, SyscallError> {
    let facility = ctx.get_process_facility::<RustProcess>(c"RustProcess")?;
    Ok(RustProcessWrapper::get_wrapper(facility))
}

fn get_rust_process_wrapper_mut(
    ctx: &SharemindModuleApi0x1SyscallContext,
) -> Result<&mut RustProcessWrapper, SyscallError> {
    let facility = ctx.get_process_facility::<RustProcess>(c"RustProcess")?;
    Ok(RustProcessWrapper::get_mut_wrapper(facility))
}

sharemind_module_api_0x1_syscall! {
/// Print a string to the runtime log
///
/// Stack arguments:
///  N/A
///
/// Reference parameters:
///  N/A
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: the string to print (string)
///
/// No return value
log_string_syscall, |args, refs, crefs, return_value, _c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 0, num_refs: 0, num_crefs: 1, need_return_value: false}
    ])?;

    let msg = ref_to_str_or_err!(crefs[0]);
    tracing::info!("[SecreC] {}", msg);
    Ok(())
}}

sharemind_module_api_0x1_syscall! {
/// Read a program argument by name.
///
/// Stack arguments:
///  N/A
///
/// Reference parameters:
///  Optional `refs[0]`: buffer for storing the argument data (byte array).
///                    The buffer is expected to have enough space for the data.
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the argument (string)
///
/// Return value: length of the argument data (uint64)
argument_syscall, |args, refs, crefs, return_value, c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 0, num_refs: 0, num_crefs: 1, need_return_value: true},
        SyscallRule {num_args: 0, num_refs: 1, num_crefs: 1, need_return_value: true}
    ])?;

    let argument_name = ref_to_str_or_err!(crefs[0]);

    let process = get_rust_process_wrapper(c)?;
    let argument = process.program_arguments
        .get(argument_name)
        .ok_or_else(|| syserr!(SyscallErrorKind::InvalidCall, format!("argument not found: {argument_name}")))?;

    const_assert!(mem::size_of::<usize>() <= mem::size_of::<u64>());
    (*return_value).uint64[0] = argument.data.len() as u64;
    if !refs.is_empty() {
        assert!(refs.len() == 1);
        let r = refs[0];
        assert!(r.size > 0);
        let to_copy: usize = cmp::min(r.size, argument.data.len());

        ptr::copy_nonoverlapping(argument.data.as_ptr(), r.pData as *mut u8, to_copy);
    }

    Ok(())
}}

sharemind_module_api_0x1_syscall! {
/// Set the program result by name.
///
/// Stack arguments:
///  Mandatory `args[0]`: first index in data buffer to read result from (uint64)
///  Mandatory `args[1]`: last index in data buffer to read result from (uint64)
///
/// Reference parameters:
///  N/A
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the result (string)
///  Mandatory `crefs[1]`: name of the result protection domain (string)
///  Mandatory `crefs[2]`: name of the result data type (string)
///  Mandatory `crefs[3]`: data buffer to read the result from (byte array)
///
/// No return value
set_result_syscall, |args, refs, crefs, return_value, c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 2, num_refs: 0, num_crefs: 4, need_return_value: false}
    ])?;

    let result_name = ref_to_str_or_err!(crefs[0]);
    let pd_name = ref_to_str_or_err!(crefs[1]);
    let type_name = ref_to_str_or_err!(crefs[2]);
    let data = crefs[3].as_bytes();

    const_assert!(mem::size_of::<usize>() >= mem::size_of::<u64>());
    let begin = args[0].uint64[0] as usize;
    let end = args[1].uint64[0] as usize;

    if begin > end || end > data.len() {
        return Err(syserr!(SyscallErrorKind::InvalidCall, "invalid data bounds"));
    }

    let process = get_rust_process_wrapper_mut(c)?;
    process.program_results.insert(
        result_name.to_owned(),
        ProgramResult {
            pd: pd_name.to_owned(),
            type_: type_name.to_owned(),
            data: data[begin..end].to_vec(),
        },
    );

    Ok(())
}}
