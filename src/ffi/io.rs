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
    ffi::c_void,
    io::{Read, Write},
};

pub struct ReadCallbackCtx<'a, R: Read> {
    reader: &'a mut R,
    bytes_read: usize,
}

impl<'a, R: Read> ReadCallbackCtx<'a, R> {
    pub fn new(reader: &'a mut R) -> Self {
        Self {
            reader,
            bytes_read: 0,
        }
    }

    /// FFI-compatible callback for reading data from the reader.
    /// Returns the number of bytes read in the `bytes_read` parameter.
    extern "C" fn ffi_read_callback(
        ctx: *mut c_void,
        data_out: *mut u8,
        len: usize,
        bytes_read: *mut usize,
    ) -> bool {
        if ctx.is_null() || data_out.is_null() || bytes_read.is_null() {
            tracing::error!("Invalid inputs to read callback");
            return false;
        }

        let read_ctx = unsafe { &mut *(ctx as *mut ReadCallbackCtx<R>) };
        let buf = unsafe { std::slice::from_raw_parts_mut(data_out, len) };

        match read_ctx.reader.read(buf) {
            Ok(n) => {
                read_ctx.bytes_read += n;
                unsafe {
                    *bytes_read = n;
                }
                true
            }
            Err(e) => {
                tracing::error!("Failed to read data: {e}");
                false
            }
        }
    }
}

pub struct WriteCallbackCtx<'a, W: Write> {
    writer: &'a mut W,
    bytes_written: usize,
}

impl<'a, W: Write> WriteCallbackCtx<'a, W> {
    pub fn new(writer: &'a mut W) -> Self {
        Self {
            writer,
            bytes_written: 0,
        }
    }

    /// FFI-compatible callback for writing data to the writer.
    extern "C" fn ffi_write_callback(ctx: *mut c_void, data: *const u8, len: usize) -> bool {
        if ctx.is_null() || data.is_null() {
            tracing::error!("Invalid inputs to write callback");
            return false;
        }

        let write_ctx = unsafe { &mut *(ctx as *mut WriteCallbackCtx<W>) };
        let buf = unsafe { std::slice::from_raw_parts(data, len) };

        match write_ctx.writer.write_all(buf) {
            Ok(_) => {
                write_ctx.bytes_written += buf.len();
                true
            }
            Err(e) => {
                tracing::error!("Failed to write data: {e}");
                false
            }
        }
    }
}

// Trait for retrieving the callback and its context as pointers for use with FFI.
pub trait AsFfiCallback {
    fn as_ffi_callback(&mut self) -> (*const c_void, *mut c_void);
}

impl<R: Read> AsFfiCallback for ReadCallbackCtx<'_, R> {
    fn as_ffi_callback(&mut self) -> (*const c_void, *mut c_void) {
        let cb_ptr = Self::ffi_read_callback as *const c_void;
        let ctx_ptr = self as *mut _ as *mut c_void;
        (cb_ptr, ctx_ptr)
    }
}

impl<W: Write> AsFfiCallback for WriteCallbackCtx<'_, W> {
    fn as_ffi_callback(&mut self) -> (*const c_void, *mut c_void) {
        let cb_ptr = Self::ffi_write_callback as *const c_void;
        let ctx_ptr = self as *mut _ as *mut c_void;
        (cb_ptr, ctx_ptr)
    }
}
