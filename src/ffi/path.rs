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
    ffi::{CString, NulError},
    path::Path,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathToCStringError {
    #[cfg(not(unix))]
    #[error("path contains invalid UTF-8")]
    InvalidUtf8, // Mostly relevant on non-unix systems

    #[error("path contains an interior NUL byte at position {}", source.nul_position())]
    InteriorNul {
        #[source]
        source: NulError,
    },
}

// Portable implementation
#[cfg(not(unix))]
pub fn path_to_cstring(path: &Path) -> Result<CString, PathToCStringError> {
    let path_str = path.to_str().ok_or(PathToCStringError::InvalidUtf8)?;
    CString::new(path_str).map_err(|e: NulError| PathToCStringError::InteriorNul { source: e })
}

// Unix specific implementation
#[cfg(unix)]
pub fn path_to_cstring(path: &Path) -> Result<CString, PathToCStringError> {
    use std::os::unix::ffi::OsStrExt;

    let bytes = path.as_os_str().as_bytes();
    CString::new(bytes).map_err(|e: NulError| PathToCStringError::InteriorNul { source: e })
}

/// Extension trait to add convenient conversion method to Path
pub trait PathToCStringExt {
    /// Convert this path to a CString
    fn to_cstring(&self) -> Result<CString, PathToCStringError>;
}

impl PathToCStringExt for Path {
    fn to_cstring(&self) -> Result<CString, PathToCStringError> {
        path_to_cstring(self)
    }
}
