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

use super::bindings::{
    LogHardBackend, LogHardBackend_addStdAppender, LogHardBackend_new, LogHardBackend_newLogger,
    LogHardBackend_setPriority, LogHardLogger as CLoghardLogger, LogHardLogger_log,
    LogHardLogger_newLogger, LogHardPriority__LOGHARD_PRIORITY_DEBUG,
    LogHardPriority__LOGHARD_PRIORITY_ERROR, LogHardPriority__LOGHARD_PRIORITY_FATAL,
    LogHardPriority__LOGHARD_PRIORITY_FULLDEBUG, LogHardPriority__LOGHARD_PRIORITY_NORMAL,
    LogHardPriority__LOGHARD_PRIORITY_WARNING,
};
use log::{Log, Metadata, Record};
use std::{
    ffi::{c_char, CStr, CString},
    ptr::null_mut,
};

pub struct LogHardLogger {
    backend: *mut LogHardBackend,
    pub inner: *mut CLoghardLogger,
}

impl LogHardLogger {
    pub fn new(level: &str, prefix: &CStr) -> Self {
        let mut log_hard_err = 0;
        let mut log_hard_err_str: *const c_char = null_mut();
        let backend = unsafe { LogHardBackend_new(&mut log_hard_err, &mut log_hard_err_str) };
        // TODO error handling
        let level_loghard = match level {
            "off" => LogHardPriority__LOGHARD_PRIORITY_FATAL,
            "error" => LogHardPriority__LOGHARD_PRIORITY_ERROR,
            "warn" => LogHardPriority__LOGHARD_PRIORITY_WARNING,
            "info" => LogHardPriority__LOGHARD_PRIORITY_NORMAL,
            "debug" => LogHardPriority__LOGHARD_PRIORITY_DEBUG,
            "trace" => LogHardPriority__LOGHARD_PRIORITY_FULLDEBUG,
            _ => panic!("Unexpected log hard priority value"),
        }; // these are actually `tracing` LevelFilter levels (Display)
        unsafe { LogHardBackend_setPriority(backend, level_loghard) };
        unsafe { LogHardBackend_addStdAppender(backend) };
        let logger = unsafe { LogHardBackend_newLogger(backend, prefix.as_ptr()) };
        Self {
            backend,
            inner: logger,
        }
    }

    pub fn sublogger(&self, prefix: &CStr) -> Self {
        let logger = unsafe { LogHardLogger_newLogger(self.inner, prefix.as_ptr()) };
        Self {
            backend: self.backend,
            inner: logger,
        }
    }
}

impl Default for LogHardLogger {
    fn default() -> Self {
        Self::new("info", c"")
    }
}

unsafe impl Sync for LogHardLogger {}
unsafe impl Send for LogHardLogger {}

impl Log for LogHardLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let logstring = CString::new(record.args().to_string()).expect("Invalid C string");
        unsafe { LogHardLogger_log(self.inner, record.level() as u32, logstring.as_ptr()) }
    }

    fn flush(&self) {}
}
