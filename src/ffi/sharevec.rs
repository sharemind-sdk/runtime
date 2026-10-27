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

use std::os::raw::c_void;

unsafe extern "C" {
    /// Returns the size of a ShareVector **in bytes**.
    pub fn sharevec_size(vec: *mut c_void) -> usize;

    /// Returns a pointer to the underlying **byte** data of a ShareVector.
    pub fn sharevec_data(vec: *mut c_void) -> *mut u8;
}
