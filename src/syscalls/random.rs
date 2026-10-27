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

use crate::ffi::vm::syscall::interface::{validate_syscall, SyscallRule};
use crate::sharemind_module_api_0x1_syscall;
use rand::RngCore;

// Simply fill with pseudo-random data.
sharemind_module_api_0x1_syscall! {
blocking_randomize, |args, refs, crefs, return_value, _c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 0, num_refs: 1, num_crefs: 0, need_return_value: false}
    ])?;

    let buffer = refs[0];
    if buffer.size == 0 {
        return Ok(());
    }

    let slice = unsafe {
        std::slice::from_raw_parts_mut(buffer.pData as *mut u8, buffer.size)
    };
    rand::rng().fill_bytes(slice);

    Ok(())
}}
