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

pub(crate) mod data_store;
pub mod logger;
pub mod network;
pub mod pd_network;
pub(crate) mod process;
#[cfg(feature = "profiler")]
pub mod profiler;
pub mod random;

use std::ffi::{c_char, c_void, CStr};

pub trait Facility<T> {
    fn inner(&mut self) -> *mut T;
    fn name(&self) -> &CStr;
}

pub type SetFacilityFn<T, E> = unsafe extern "C" fn(
    c: *mut T,
    name: *const c_char,
    facility: *mut c_void,
    context: *mut c_void,
) -> E;
