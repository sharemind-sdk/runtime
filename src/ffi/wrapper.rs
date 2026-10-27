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

use crate::ffi::bindings::SharemindNetworkError;

pub mod network;
pub mod network_configuration;
pub mod node;
pub mod pdpi_network_facility;

/// A type that implements this trait must have C representation and the `inner` field of type T at offset 0.
pub trait WrappedC<T>: Sized {
    fn get_wrapper<'a>(inner: *const T) -> &'a Self {
        unsafe { &*(inner as *const _) }
    }

    fn get_mut_wrapper<'a>(inner: *mut T) -> &'a mut Self {
        unsafe { &mut *(inner as *mut _) }
    }
}

pub type NetworkResult<T> = Result<T, SharemindNetworkError>;
