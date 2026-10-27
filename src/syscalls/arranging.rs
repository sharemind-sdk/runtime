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

use crate::ffi::sharevec::{sharevec_data, sharevec_size};
use crate::ffi::vm::syscall::error::SyscallErrorKind;
use crate::ffi::vm::syscall::interface::{validate_syscall, SyscallRule};
use crate::{sharemind_module_api_0x1_syscall, syserr};
use std::{ptr, slice};

unsafe fn data_range_arg(arg: &crate::ffi::bindings::SharemindCodeBlock) -> (*mut u8, usize) {
    let arg_p = arg.p[0];
    (sharevec_data(arg_p), sharevec_size(arg_p))
}

unsafe fn data_range_rf(
    reference: &crate::ffi::bindings::SharemindModuleApi0x1Reference,
) -> (*mut u8, usize) {
    (reference.pData as *mut u8, reference.size)
}

sharemind_module_api_0x1_syscall! {
    /// Gather the specific array slice from src into dest.
    /// Note that supplying element size is not necessary as it is derived from the provided input.
    ///
    /// Stack arguments (`args`, ShareVec) *or* Reference parameters (`refs`):
    ///  Mandatory 0: source data buffer.
    ///  Mandatory 1: destination data buffer.
    ///  Source and destination must be different.
    ///
    /// Constant reference parameters:
    ///  Mandatory `crefs[0]`: dimensions of the source (as uint64).
    ///  Mandatory `crefs[1]`: slice begin (element) index in every dimension (as uint64).
    ///  Mandatory `crefs[2]`: slice end index in every dimension (as uint64).
    ///  Optional  `crefs[3]`: slice step value in every dimension (as uint64).
    ///
    /// No return value
    arrgather_syscall, |args, refs, crefs, return_value, _c| {
        validate_syscall(args, refs, crefs, return_value, &[
            SyscallRule {num_args: 2, num_refs: 0, num_crefs: 3, need_return_value: false},
            SyscallRule {num_args: 2, num_refs: 0, num_crefs: 4, need_return_value: false},
            SyscallRule {num_args: 0, num_refs: 2, num_crefs: 3, need_return_value: false},
            SyscallRule {num_args: 0, num_refs: 2, num_crefs: 4, need_return_value: false},
        ])?;
        if crefs.len() == 4 {
            return Err(syserr!(SyscallErrorKind::ImplementationLimitsReached,
                "Step for array slicing is not implemented".to_string()
            ));
        }

        let (src_data, src_size) = if args.len() == 2 { data_range_arg(&args[0]) } else { data_range_rf(&refs[0]) };
        let (dest_data, dest_size) = if args.len() == 2 { data_range_arg(&args[1]) } else { data_range_rf(&refs[1]) };

        let num_dims = crefs[0].size / (64 / 8);
        assert!(crefs[0].size > 0);
        assert_eq!(crefs[0].size, crefs[1].size);
        assert_eq!(crefs[0].size, crefs[2].size);
        let dims = unsafe { slice::from_raw_parts(crefs[0].pData as *const u64, num_dims) };
        let begins = unsafe { slice::from_raw_parts(crefs[1].pData as *const u64, num_dims) };
        let ends = unsafe { slice::from_raw_parts(crefs[2].pData as *const u64, num_dims) };

        for i in 0..num_dims {
            assert!(begins[i] <= ends[i]); // support code that requires zero slices, simply do nothing
            assert!(ends[i] <= dims[i]);
        }

        let mut diffs = vec![0u64; num_dims];
        for dim in 0..num_dims {
            diffs[dim] = ends[dim] - begins[dim];
        }

        let src_elements: u64 = dims.iter().product();
        if src_elements == 0 {
            // Prevents division by zero
            assert!(diffs.iter().all(|x| *x == 0u64));
            return Ok(());
        }
        let element_size: u64 = src_size as u64 / src_elements;

        // stride[dim_i] is a number that specifies by how many bytes the offsets of elements A and B (in src) differ,
        // if B has all the same dimension indexes that A does, except that index at dim_i is one higher.
        let mut strides = vec![element_size; num_dims];
        for dim in (0..num_dims - 1).rev() {
            strides[dim] = strides[dim + 1] * dims[dim + 1];
        }

        // copying multiple elements at one, which is faster
        let region_size = element_size * diffs.last().unwrap();
        let mut selected_regions = 1u64;
        for dim in 0..num_dims - 1 {
            selected_regions *= diffs[dim];
        }
        let region_offset = element_size * begins.last().unwrap();

        if selected_regions * region_size > dest_size as u64 {
            return Err(syserr!(SyscallErrorKind::GeneralError, "Output buffer too small".to_string()));
        }

        for region_idx in 0..selected_regions {
            let mut src_offset = 0u64;
            let mut region_idx_rem = region_idx;
            for thisdim in (0..num_dims - 1).rev() {
                let el = region_idx_rem % diffs[thisdim];
                region_idx_rem /= diffs[thisdim];
                src_offset += (begins[thisdim] + el) * strides[thisdim];
            }
            src_offset += region_offset;

            let dest_offset = region_idx * region_size;

            ptr::copy_nonoverlapping(
                src_data.offset(src_offset as isize),
                dest_data.offset(dest_offset as isize),
                region_size as usize,
            );
        }

        Ok(())
    }
}

sharemind_module_api_0x1_syscall! {
    /// Scatter from src into the specific array slice of dest.
    /// Elements will be scattered either 1-to-1 (if src is large), or 1-to-all (if src size is one element).
    /// Note that supplying element size is not necessary as it is derived from the provided input.
    ///
    /// Stack arguments (`args`, ShareVec) *or* Reference parameters (`refs`):
    ///  Mandatory 0: source data buffer.
    ///  Mandatory 1: destination data buffer.
    ///  Source and destination must be different.
    ///
    /// Constant reference parameters:
    ///  Mandatory `crefs[0]`: dimensions of the destination (as uint64).
    ///  Mandatory `crefs[1]`: slice begin (element) index in every dimension (as uint64).
    ///  Mandatory `crefs[2]`: slice end index in every dimension (as uint64).
    ///  Optional  `crefs[3]`: slice step value in every dimension (as uint64).
    ///
    /// No return value
    arrscatter_syscall, |args, refs, crefs, return_value, _c| {
        validate_syscall(args, refs, crefs, return_value, &[
            SyscallRule {num_args: 2, num_refs: 0, num_crefs: 3, need_return_value: false},
            SyscallRule {num_args: 2, num_refs: 0, num_crefs: 4, need_return_value: false},
            SyscallRule {num_args: 0, num_refs: 2, num_crefs: 3, need_return_value: false},
            SyscallRule {num_args: 0, num_refs: 2, num_crefs: 4, need_return_value: false},
        ])?;
        if crefs.len() == 4 {
            return Err(syserr!(SyscallErrorKind::ImplementationLimitsReached,
                "Step for array slicing is not implemented".to_string()
            ));
        }

        let (src_data, src_size) = if args.len() == 2 { data_range_arg(&args[0]) } else { data_range_rf(&refs[0]) };
        let (dest_data, dest_size) = if args.len() == 2 { data_range_arg(&args[1]) } else { data_range_rf(&refs[1]) };

        let num_dims = crefs[0].size / (64 / 8);
        assert!(crefs[0].size > 0);
        assert_eq!(crefs[0].size, crefs[1].size);
        assert_eq!(crefs[0].size, crefs[2].size);
        let dims = unsafe { slice::from_raw_parts(crefs[0].pData as *const u64, num_dims) };
        let begins = unsafe { slice::from_raw_parts(crefs[1].pData as *const u64, num_dims) };
        let ends = unsafe { slice::from_raw_parts(crefs[2].pData as *const u64, num_dims) };

        for i in 0..num_dims {
            assert!(begins[i] <= ends[i]); // support code that requires zero slices, simply do nothing
            assert!(ends[i] <= dims[i]);
        }

        let mut diffs = vec![0u64; num_dims];
        for dim in 0..num_dims {
            diffs[dim] = ends[dim] - begins[dim];
        }

        let dest_elements: u64 = dims.iter().product();
        if dest_elements == 0 {
            // Prevents division by zero
            assert!(diffs.iter().all(|x| *x == 0u64));
            return Ok(());
        }
        let element_size: u64 = dest_size as u64 / dest_elements;

        // stride[dim_i] is a number that specifies by how many bytes the offsets of elements A and B (in dest) differ,
        // if B has all the same dimension indexes that A does, except that index at dim_i is one higher.
        let mut strides = vec![element_size; num_dims];
        for dim in (0..num_dims - 1).rev() {
            strides[dim] = strides[dim + 1] * dims[dim + 1];
        }

        // copying multiple elements at one, which is faster
        let region_size = element_size * diffs.last().unwrap();
        let mut selected_regions = 1u64;
        for dim in 0..num_dims - 1 {
            selected_regions *= diffs[dim];
        }
        let region_offset = element_size * begins.last().unwrap();

        if element_size != src_size as u64 && selected_regions * region_size != src_size as u64 {
            return Err(syserr!(SyscallErrorKind::GeneralError, "Input buffer too small".to_string()));
        }

        for region_idx in 0..selected_regions {
            let mut dest_offset = 0u64;
            let mut region_idx_rem = region_idx;
            for thisdim in (0..num_dims - 1).rev() {
                let el = region_idx_rem % diffs[thisdim];
                region_idx_rem /= diffs[thisdim];
                dest_offset += (begins[thisdim] + el) * strides[thisdim];
            }
            dest_offset += region_offset;

            let src_offset = region_idx * region_size;

            if element_size != src_size as u64 {
                ptr::copy_nonoverlapping(
                    src_data.offset(src_offset as isize),
                    dest_data.offset(dest_offset as isize),
                    region_size as usize,
                );
            } else {
                for i in 0u64..diffs[num_dims - 1] {
                    ptr::copy_nonoverlapping(
                        src_data,
                        dest_data.offset((dest_offset + i * element_size) as isize),
                        element_size as usize,
                    );
                }
            }
        }

        Ok(())
    }
}
