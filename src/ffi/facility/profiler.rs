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
    ffi::{c_void, CStr},
    ptr::NonNull,
};

use crate::ffi::{
    bindings::{
        uint_fast64_t, SharemindExecutionProfiler, SharemindExecutionProfiler_end_section,
        SharemindExecutionProfiler_free, SharemindExecutionProfiler_new,
        SharemindExecutionProfiler_new_section_type, SharemindExecutionProfiler_pop_parent_section,
        SharemindExecutionProfiler_process_log, SharemindExecutionProfiler_push_parent_section,
        SharemindExecutionProfiler_start_log, SharemindExecutionProfiler_start_section,
    },
    facility::Facility,
    loghard::LogHardLogger,
};

pub type CxxSharemindExecutionProfiler = c_void;

pub struct ProfilerFacility {
    inner: NonNull<SharemindExecutionProfiler>,
}

pub type SectionType = u32;
pub type SectionComplexity = u64;
pub type SectionId = u32;
pub type EndTime = uint_fast64_t;

impl ProfilerFacility {
    pub fn new(logger: &mut LogHardLogger) -> Result<Self, String> {
        let profiler = NonNull::new(unsafe { SharemindExecutionProfiler_new(logger.inner()) })
            .ok_or("Failed to create execution profiler object")?;
        Ok(Self { inner: profiler })
    }

    pub fn start_log(&self, filename: &CStr) -> bool {
        unsafe { SharemindExecutionProfiler_start_log(self.inner.as_ptr(), filename.as_ptr()) }
    }

    pub fn process_log(&self) {
        unsafe { SharemindExecutionProfiler_process_log(self.inner.as_ptr()) }
    }

    pub fn new_section_type(&self, name: &CStr) -> SectionType {
        unsafe { SharemindExecutionProfiler_new_section_type(self.inner.as_ptr(), name.as_ptr()) }
    }

    pub fn start_section(
        &self,
        section_type: SectionType,
        section_complexity: SectionComplexity,
    ) -> SectionId {
        unsafe {
            SharemindExecutionProfiler_start_section(
                self.inner.as_ptr(),
                section_type,
                section_complexity,
            )
        }
    }

    pub fn end_section(&self, section_id: SectionId, end_time: EndTime) {
        unsafe { SharemindExecutionProfiler_end_section(self.inner.as_ptr(), section_id, end_time) }
    }

    pub fn push_parent_section(&self, section_id: SectionId) {
        unsafe { SharemindExecutionProfiler_push_parent_section(self.inner.as_ptr(), section_id) }
    }

    pub fn pop_parent_section(&self) {
        unsafe { SharemindExecutionProfiler_pop_parent_section(self.inner.as_ptr()) }
    }
}

// For use with mod_executionprofiler, which expects a C++ handle.
impl Facility<c_void> for ProfilerFacility {
    fn inner(&mut self) -> *mut c_void {
        self.inner.as_ptr() as *mut _
    }

    fn name(&self) -> &CStr {
        c"Profiler"
    }
}

impl Drop for ProfilerFacility {
    fn drop(&mut self) {
        unsafe { SharemindExecutionProfiler_free(self.inner.as_ptr()) };
    }
}
