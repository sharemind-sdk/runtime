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

// Sharemind error code constants in match expressions generate warnings
#![allow(non_upper_case_globals)]

use std::{
    ffi::CStr,
    marker::PhantomData,
    ptr::{null, null_mut, NonNull},
};

use crate::ffi::ctrl::bindings::{
    SharemindCodeBlock, SharemindControllerCReference, SharemindControllerDataType,
    SharemindControllerDataTypeOperationWrapper, SharemindControllerDataType_classifyWrapper,
    SharemindControllerDataType_declassifyWrapper, SharemindControllerDataType_module,
    SharemindControllerDataType_privateSize, SharemindControllerDataType_publicSize,
    SharemindControllerDataType_signature, SharemindControllerModuleApi0x1Error,
    SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_GENERAL_ERROR,
    SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_IMPLEMENTATION_LIMITS_REACHED,
    SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_INVALID_CALL,
    SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_INVALID_PD_CONFIGURATION,
    SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_MISSING_FACILITY,
    SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_MODULE_ERROR,
    SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_OK,
    SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_OUT_OF_MEMORY,
    SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_SHAREMIND_ERROR,
    SharemindControllerModule_handle, SharemindControllerPd_handle, SharemindControllerReference,
    SharemindDataTypeContext,
};

use super::{module::ControllerModule, pd::ProtectionDomain};

pub struct ControllerDataType<'a> {
    inner: NonNull<SharemindControllerDataType>,
    _owner_marker: std::marker::PhantomData<&'a ControllerModule>,
}

impl<'a> ControllerDataType<'a> {
    pub fn from_ptr(
        dt: NonNull<SharemindControllerDataType>,
        _owner: &'a ControllerModule,
    ) -> Self {
        Self {
            inner: dt,
            _owner_marker: PhantomData,
        }
    }

    fn handle(&self) -> *mut SharemindControllerDataType {
        self.inner.as_ptr()
    }

    pub fn public_size(&self) -> usize {
        unsafe { SharemindControllerDataType_publicSize(self.handle()) }
    }

    pub fn private_size(&self) -> usize {
        unsafe { SharemindControllerDataType_privateSize(self.handle()) }
    }

    fn signature(&self) -> &str {
        let dt_signature_cstr = unsafe {
            let dt_signature_cbuf = SharemindControllerDataType_signature(self.handle());
            assert!(!dt_signature_cbuf.is_null());
            CStr::from_ptr(dt_signature_cbuf)
        };
        dt_signature_cstr.to_str().unwrap()
    }

    pub fn classify(
        &self,
        pd: &ProtectionDomain,
        refs: Option<&[SharemindControllerReference]>,
        crefs: &[SharemindControllerCReference],
    ) -> Result<(), ControllerDataTypeOperationError> {
        tracing::trace!("Entering classify syscall");
        let op_wrapper = unsafe { SharemindControllerDataType_classifyWrapper(self.handle()) };
        self.operation(op_wrapper, pd, None, refs, Some(crefs))
    }

    pub fn declassify(
        &self,
        pd: &ProtectionDomain,
        args: Option<&[SharemindCodeBlock]>,
        refs: Option<&[SharemindControllerReference]>,
        crefs: Option<&[SharemindControllerCReference]>,
    ) -> Result<(), ControllerDataTypeOperationError> {
        tracing::trace!("Entering declassify syscall");
        let op_wrapper = unsafe { SharemindControllerDataType_declassifyWrapper(self.handle()) };
        self.operation(op_wrapper, pd, args, refs, crefs)
    }

    fn operation(
        &self,
        op_wrapper: SharemindControllerDataTypeOperationWrapper,
        pd: &ProtectionDomain,
        args: Option<&[SharemindCodeBlock]>,
        refs: Option<&[SharemindControllerReference]>,
        crefs: Option<&[SharemindControllerCReference]>,
    ) -> Result<(), ControllerDataTypeOperationError> {
        // Validate the PD and data type are related
        if !self
            .signature()
            .starts_with(&format!("{}::", pd.pdk_name()))
        {
            return Err(ControllerDataTypeOperationError::invalid_call());
        }

        let module = NonNull::new(unsafe { SharemindControllerDataType_module(self.handle()) })
            .ok_or_else(ControllerDataTypeOperationError::sharemind_error)?;

        // Single-use context for the operation syscall
        let mut context = SharemindDataTypeContext {
            moduleHandle: unsafe { SharemindControllerModule_handle(module.as_ptr()) },
            pdHandle: unsafe { SharemindControllerPd_handle(pd.handle()) },
        };

        let syscall = op_wrapper
            .callable
            .ok_or_else(ControllerDataTypeOperationError::sharemind_error)?;

        let (args_ptr, num_args) =
            args.map_or((null_mut(), 0), |s| (s.as_ptr() as *mut _, s.len()));
        let refs_ptr = refs.map_or(null(), |s| s.as_ptr());
        let crefs_ptr = crefs.map_or(null(), |s| s.as_ptr());

        let syscall_status = unsafe {
            syscall(
                args_ptr,
                num_args,
                refs_ptr,
                crefs_ptr,
                null_mut(),
                &mut context,
            )
        };

        match syscall_status {
            SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_OK => Ok(()),
            err => Err(ControllerDataTypeOperationError::from(err)),
        }
    }
}

pub struct ControllerDataTypeOperationError {
    error_code: SharemindControllerModuleApi0x1Error,
}

impl ControllerDataTypeOperationError {
    pub fn sharemind_error() -> Self {
        Self::from(SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_SHAREMIND_ERROR)
    }

    pub fn invalid_call() -> Self {
        Self::from(
            SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_INVALID_CALL,
        )
    }
}

impl From<SharemindControllerModuleApi0x1Error> for ControllerDataTypeOperationError {
    fn from(error: SharemindControllerModuleApi0x1Error) -> Self {
        Self { error_code: error }
    }
}

impl std::fmt::Display for ControllerDataTypeOperationError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        let error_str = match self.error_code {
            SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_OK => {
                "Success"
            }
            SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_OUT_OF_MEMORY => {
                "Out of memory"
            }
            SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_IMPLEMENTATION_LIMITS_REACHED => {
                "Implementation limits reached"
            }
            SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_SHAREMIND_ERROR => {
                "Sharemind acted in an unspecified manner"
            }
            SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_MODULE_ERROR => {
                "Programming fault in the module"
            }
            SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_GENERAL_ERROR => {
                "A general runtime error"
            }
            SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_INVALID_CALL => {
                "The system call was called improperly"
            }
            SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_MISSING_FACILITY => {
                "A required facility was not provided"
            }
            SharemindControllerModuleApi0x1Error_SHAREMIND_CONTROLLER_MODULE_API_0x1_INVALID_PD_CONFIGURATION => {
                "The protection domain configuration given was invalid or erroneous"
            }
            _ => "Unknown error",
        };

        write!(fmt, "{}", error_str)
    }
}
