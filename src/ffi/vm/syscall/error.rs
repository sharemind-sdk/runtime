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

use std::{ffi::CString, str::Utf8Error};

use crate::ffi::bindings::{
    SharemindModuleApi0x1Error, SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_ACCESS_DENIED,
    SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_GENERAL_ERROR,
    SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_IMPLEMENTATION_LIMITS_REACHED,
    SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_INVALID_CALL,
    SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_INVALID_MODULE_CONFIGURATION,
    SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_INVALID_PD_CONFIGURATION,
    SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_MISSING_FACILITY,
    SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_MODULE_ERROR,
    SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_OUT_OF_MEMORY,
};

#[derive(Clone, Debug)]
#[repr(u32)]
pub enum SyscallErrorKind {
    OutOfMemory,
    ImplementationLimitsReached,
    ModuleError,
    GeneralError,
    InvalidCall,
    MissingFacility,
    InvalidPdConfiguration,
    InvalidModuleConfiguration,
    AccessDenied,
}

impl std::fmt::Display for SyscallErrorKind {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::OutOfMemory => write!(fmt, "Out of memory"),
            Self::ImplementationLimitsReached => write!(fmt, "Implementation limits reached"),
            Self::ModuleError => write!(fmt, "Module error"),
            Self::GeneralError => write!(fmt, "General error"),
            Self::InvalidCall => write!(fmt, "Invalid call"),
            Self::MissingFacility => write!(fmt, "Missing facility"),
            Self::InvalidPdConfiguration => write!(fmt, "Invalid pd configuration"),
            Self::InvalidModuleConfiguration => write!(fmt, "Invalid module configuration"),
            Self::AccessDenied => write!(fmt, "Access denied"),
        }
    }
}

impl From<SyscallErrorKind> for SharemindModuleApi0x1Error {
    fn from(kind: SyscallErrorKind) -> SharemindModuleApi0x1Error {
        use SyscallErrorKind::*;
        match kind {
            OutOfMemory => SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_OUT_OF_MEMORY,
            ImplementationLimitsReached => {
                SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_IMPLEMENTATION_LIMITS_REACHED
            }
            ModuleError => SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_MODULE_ERROR,
            GeneralError => SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_GENERAL_ERROR,
            InvalidCall => SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_INVALID_CALL,
            MissingFacility => SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_MISSING_FACILITY,
            InvalidPdConfiguration => {
                SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_INVALID_PD_CONFIGURATION
            }
            InvalidModuleConfiguration => {
                SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_INVALID_MODULE_CONFIGURATION
            }
            AccessDenied => SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_ACCESS_DENIED,
        }
    }
}

impl From<SharemindModuleApi0x1Error> for SyscallErrorKind {
    fn from(value: SharemindModuleApi0x1Error) -> SyscallErrorKind {
        use SyscallErrorKind::*;
        match value {
            SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_OUT_OF_MEMORY => OutOfMemory,
            SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_IMPLEMENTATION_LIMITS_REACHED => {
                ImplementationLimitsReached
            }
            SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_MODULE_ERROR => ModuleError,
            SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_GENERAL_ERROR => GeneralError,
            SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_INVALID_CALL => InvalidCall,
            SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_MISSING_FACILITY => MissingFacility,
            SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_INVALID_PD_CONFIGURATION => {
                InvalidPdConfiguration
            }
            SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_INVALID_MODULE_CONFIGURATION => {
                InvalidModuleConfiguration
            }
            SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_ACCESS_DENIED => AccessDenied,
            _ => {
                panic!("Unsupported value of SharemindModuleApi0x1Error: {value}");
            }
        }
    }
}

#[derive(Debug)]
pub struct SyscallError {
    pub kind: SyscallErrorKind,
    reason: Option<String>,
    source: Option<Box<dyn std::error::Error>>,
}

impl SyscallError {
    pub fn new(kind: SyscallErrorKind) -> Self {
        SyscallError {
            kind,
            reason: None,
            source: None,
        }
    }

    pub fn with_reason<T: AsRef<str>>(mut self, reason: T) -> Self {
        self.reason = Some(reason.as_ref().to_string());
        self
    }

    pub fn with_source<E>(mut self, err: E) -> Self
    where
        E: Into<Box<dyn std::error::Error>>,
    {
        self.source = Some(err.into());
        self
    }
}

impl std::fmt::Display for SyscallError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.kind)?;

        if let Some(reason) = &self.reason {
            write!(f, ": {}", reason)?
        }

        if let Some(source) = &self.source {
            write!(f, " (source: {})", source)?
        }

        Ok(())
    }
}

impl std::error::Error for SyscallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_deref()
    }
}

#[macro_export]
macro_rules! syserr {
    ($kind:expr) => {
        $crate::ffi::vm::syscall::error::SyscallError::new($kind)
    };
    ($kind:expr, $reason:expr) => {
        $crate::ffi::vm::syscall::error::SyscallError::new($kind).with_reason($reason)
    };
    ($kind:expr, $reason:expr, $source:expr) => {
        $crate::ffi::vm::syscall::error::SyscallError::new($kind)
            .with_reason($reason)
            .with_source($source)
    };
}

#[derive(Debug)]
pub enum ModuleApiFinderError {
    NullFunctionPointer(&'static str),
    CStringUtf8Error(&'static str, Utf8Error),
    NotFound(&'static str, CString),
}

impl std::error::Error for ModuleApiFinderError {}

impl std::fmt::Display for ModuleApiFinderError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ModuleApiFinderError::NullFunctionPointer(what) => {
                write!(f, "{} function is None", what)
            }
            ModuleApiFinderError::CStringUtf8Error(what, err) => {
                write!(f, "{} name contains invalid UTF-8: {}", what, err)
            }
            ModuleApiFinderError::NotFound(what, name) => match name.to_str() {
                Ok(s) => write!(f, "{} not found: {}", what, s),
                Err(_) => write!(
                    f,
                    "{} not found: {} (name contains invalid UTF-8)",
                    what,
                    name.to_string_lossy()
                ),
            },
        }
    }
}

impl From<ModuleApiFinderError> for SyscallError {
    fn from(value: ModuleApiFinderError) -> Self {
        match value {
            ModuleApiFinderError::NotFound(_, _) => {
                syserr!(SyscallErrorKind::MissingFacility, value.to_string())
            }
            _ => syserr!(SyscallErrorKind::GeneralError, value.to_string()),
        }
    }
}
