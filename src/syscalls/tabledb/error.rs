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

use std::ffi::{c_void, CStr};

use crate::{
    ffi::{
        bindings::{
            SharemindModuleApi0x1SyscallContext, SharemindTdbError,
            SharemindTdbError__SHAREMIND_TDB_CONSENSUS_ERROR,
            SharemindTdbError__SHAREMIND_TDB_GENERAL_ERROR,
            SharemindTdbError__SHAREMIND_TDB_INVALID_ARGUMENT,
            SharemindTdbError__SHAREMIND_TDB_IO_ERROR,
            SharemindTdbError__SHAREMIND_TDB_MISSING_FACILITY,
            SharemindTdbError__SHAREMIND_TDB_TABLE_ALREADY_EXISTS,
            SharemindTdbError__SHAREMIND_TDB_TABLE_NOT_FOUND,
            SharemindTdbError__SHAREMIND_TDB_UNKNOWN_ERROR,
        },
        vm::syscall::{
            data_store::get_data_store,
            error::{SyscallError, SyscallErrorKind},
        },
    },
    syserr,
};

#[derive(Clone, Debug, PartialEq)]
pub enum TdbErrorKind {
    /** An unexpected exception occurred. */
    UnknownError,

    /** An general error occurred. */
    GeneralError,

    /** Could not reach consensus. */
    ConsensusError,

    /** An IO error occurred. */
    IoError,

    /** An invalid argument was given to the function. */
    InvalidArgument,

    /** No such table was found. */
    TableNotFound,

    /** Cannot create table, already exists. */
    TableAlreadyExists,

    /** Missing facility. */
    MissingFacility,
}

impl From<SharemindTdbError> for TdbErrorKind {
    fn from(value: SharemindTdbError) -> Self {
        match value {
            SharemindTdbError__SHAREMIND_TDB_UNKNOWN_ERROR => Self::UnknownError,
            SharemindTdbError__SHAREMIND_TDB_GENERAL_ERROR => Self::GeneralError,
            SharemindTdbError__SHAREMIND_TDB_CONSENSUS_ERROR => Self::ConsensusError,
            SharemindTdbError__SHAREMIND_TDB_IO_ERROR => Self::IoError,
            SharemindTdbError__SHAREMIND_TDB_INVALID_ARGUMENT => Self::InvalidArgument,
            SharemindTdbError__SHAREMIND_TDB_TABLE_NOT_FOUND => Self::TableNotFound,
            SharemindTdbError__SHAREMIND_TDB_TABLE_ALREADY_EXISTS => Self::TableAlreadyExists,
            SharemindTdbError__SHAREMIND_TDB_MISSING_FACILITY => Self::MissingFacility,
            _ => {
                tracing::warn!("Unsupported value of SharemindTdbError: {value}");
                Self::UnknownError
            }
        }
    }
}

impl From<TdbErrorKind> for SharemindTdbError {
    fn from(value: TdbErrorKind) -> Self {
        match value {
            TdbErrorKind::UnknownError => SharemindTdbError__SHAREMIND_TDB_UNKNOWN_ERROR,
            TdbErrorKind::GeneralError => SharemindTdbError__SHAREMIND_TDB_GENERAL_ERROR,
            TdbErrorKind::ConsensusError => SharemindTdbError__SHAREMIND_TDB_CONSENSUS_ERROR,
            TdbErrorKind::IoError => SharemindTdbError__SHAREMIND_TDB_IO_ERROR,
            TdbErrorKind::InvalidArgument => SharemindTdbError__SHAREMIND_TDB_INVALID_ARGUMENT,
            TdbErrorKind::TableNotFound => SharemindTdbError__SHAREMIND_TDB_TABLE_NOT_FOUND,
            TdbErrorKind::TableAlreadyExists => {
                SharemindTdbError__SHAREMIND_TDB_TABLE_ALREADY_EXISTS
            }
            TdbErrorKind::MissingFacility => SharemindTdbError__SHAREMIND_TDB_MISSING_FACILITY,
        }
    }
}

impl std::fmt::Display for TdbErrorKind {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
        match self {
            Self::UnknownError => write!(fmt, "Unknown error"),
            Self::GeneralError => write!(fmt, "General error"),
            Self::ConsensusError => write!(fmt, "Consensus error"),
            Self::IoError => write!(fmt, "I/O error"),
            Self::InvalidArgument => write!(fmt, "Invalid argument"),
            Self::TableNotFound => write!(fmt, "Table not found"),
            Self::TableAlreadyExists => write!(fmt, "Table already exists"),
            Self::MissingFacility => write!(fmt, "Missing facility"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TdbError {
    pub kind: TdbErrorKind,
    pub reason: String,
}

impl TdbError {
    pub fn new<S: Into<String>>(kind: TdbErrorKind, reason: S) -> Self {
        TdbError {
            kind,
            reason: reason.into(),
        }
    }

    pub fn from_err<S: Into<String>>(other: TdbError, reason: S) -> Self {
        TdbError {
            kind: other.kind,
            reason: format!("{}: {}", reason.into(), other.reason),
        }
    }

    pub fn general_error<S: Into<String>>(reason: S) -> Self {
        TdbError {
            kind: TdbErrorKind::GeneralError,
            reason: reason.into(),
        }
    }

    pub fn invalid_argument<S: Into<String>>(reason: S) -> Self {
        TdbError {
            kind: TdbErrorKind::InvalidArgument,
            reason: reason.into(),
        }
    }

    pub fn io_error<S: Into<String>>(reason: S) -> Self {
        TdbError {
            kind: TdbErrorKind::IoError,
            reason: reason.into(),
        }
    }
}

impl std::error::Error for TdbError {}

impl std::fmt::Display for TdbError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.reason.is_empty() {
            write!(fmt, "{}", self.kind)
        } else {
            write!(fmt, "{}: {}", self.kind, self.reason)
        }
    }
}

impl From<::std::io::Error> for TdbError {
    fn from(err: ::std::io::Error) -> Self {
        Self {
            kind: TdbErrorKind::IoError,
            reason: err.to_string(),
        }
    }
}

impl From<::std::ffi::NulError> for TdbError {
    fn from(err: ::std::ffi::NulError) -> Self {
        Self {
            kind: TdbErrorKind::GeneralError,
            reason: err.to_string(),
        }
    }
}

impl From<::capnp::Error> for TdbError {
    fn from(err: ::capnp::Error) -> Self {
        Self {
            kind: TdbErrorKind::IoError,
            reason: format!("RPC error: {}", err),
        }
    }
}

impl From<TdbErrorKind> for SyscallErrorKind {
    fn from(kind: TdbErrorKind) -> Self {
        match kind {
            TdbErrorKind::InvalidArgument => Self::InvalidCall,
            _ => Self::GeneralError,
        }
    }
}

impl From<TdbError> for SyscallError {
    fn from(err: TdbError) -> Self {
        syserr!(err.kind.clone().into(), format!("tabledb error: {err}"))
    }
}

unsafe extern "C" fn tabledb_error_code_deleter(ptr: *mut c_void) {
    if !ptr.is_null() {
        let _ = Box::from_raw(ptr as *mut SharemindTdbError); // dropped
    }
}

// Communicates the error code to tabledb module by setting the error code in tabledb `errors` datastore.
pub fn set_tabledb_module_error_code(
    ctx: &SharemindModuleApi0x1SyscallContext,
    ds_name: &CStr,
    ecode: SharemindTdbError,
) -> Result<(), SyscallError> {
    // Get error data store
    let errors_ds = get_data_store(ctx, c"mod_tabledb/errors")?;

    // Remove existing error code, if any
    errors_ds.remove(ds_name);

    // Set the new error code
    let err = Box::<SharemindTdbError>::new(ecode);
    let err_ptr = Box::into_raw(err);
    let r = unsafe {
        errors_ds.set(
            ds_name,
            err_ptr as *mut c_void,
            Some(tabledb_error_code_deleter),
        )
    };

    if !r {
        unsafe {
            let _ = Box::from_raw(err_ptr); // drop if setting the error failed
        }

        return Err(syserr!(
            SyscallErrorKind::GeneralError,
            format!(
                "unable to set error code for data source '{}' (probably already set)",
                ds_name.to_string_lossy()
            )
        ));
    }

    Ok(())
}
