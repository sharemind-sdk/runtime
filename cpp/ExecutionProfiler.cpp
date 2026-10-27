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

#include <cassert>
#include <LogHard/Logger.h>
#include <LogHard/LoggerC.h>
#include <sharemind/ExecutionProfiler.h>

#include "libexecutionprofiler.h"

using namespace sharemind;

extern "C" {

struct SharemindExecutionProfiler {
    SharemindExecutionProfiler(LogHardLogger * logger)
        : inner(*static_cast<LogHard::Logger *>(LogHardLogger_cxx(logger)))
    {}

    ExecutionProfiler inner;
};

ExecutionProfiler & getInstance(SharemindExecutionProfiler * profiler) {
    assert(profiler);
    return profiler->inner;
}


SharemindExecutionProfiler * SharemindExecutionProfiler_new(LogHardLogger * logger) {
    assert(logger);
    return new SharemindExecutionProfiler(logger);
}

void SharemindExecutionProfiler_free(SharemindExecutionProfiler * profiler) {
    delete profiler;
}

bool SharemindExecutionProfiler_start_log(
        SharemindExecutionProfiler * profiler,
        const char * filename)
{
    assert(profiler);
    return getInstance(profiler).startLog(filename);
}

void SharemindExecutionProfiler_process_log(
        SharemindExecutionProfiler * profiler)
{
    assert(profiler);
    return getInstance(profiler).processLog();
}

uint32_t SharemindExecutionProfiler_new_section_type(
        SharemindExecutionProfiler * profiler,
        const char * name)
{
    assert(profiler);
    return getInstance(profiler).newSectionType(name);
}

uint32_t SharemindExecutionProfiler_start_section(
        SharemindExecutionProfiler * profiler,
        uint32_t sectionType,
        uint64_t sectionComplexity)
{
    assert(profiler);
    return getInstance(profiler).startSection(sectionType, sectionComplexity);
}

void SharemindExecutionProfiler_end_section(
        SharemindExecutionProfiler * profiler,
        uint32_t sectionId,
        uint_fast64_t endTime)
{
    assert(profiler);
    return getInstance(profiler).endSection(sectionId, endTime);
}

void SharemindExecutionProfiler_push_parent_section(
        SharemindExecutionProfiler * profiler,
        uint32_t sectionId)
{
    assert(profiler);
    return getInstance(profiler).pushParentSection(sectionId);
}

void SharemindExecutionProfiler_pop_parent_section(
        SharemindExecutionProfiler * profiler)
{
    assert(profiler);
    return getInstance(profiler).popParentSection();
}

} // extern "C"