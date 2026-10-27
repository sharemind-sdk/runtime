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

#pragma once

#include <sharemind/extern_c.h>

SHAREMIND_EXTERN_C_BEGIN

/** Forward declarations: */

struct LogHardLogger;
typedef struct LogHardLogger LogHardLogger;
struct SharemindExecutionProfiler;
typedef struct SharemindExecutionProfiler SharemindExecutionProfiler;


SharemindExecutionProfiler * SharemindExecutionProfiler_new(LogHardLogger * logger);
void SharemindExecutionProfiler_free(SharemindExecutionProfiler * profiler);

bool SharemindExecutionProfiler_start_log(
        SharemindExecutionProfiler * profiler,
        const char * filename);

void SharemindExecutionProfiler_process_log(
        SharemindExecutionProfiler * profiler);

uint32_t SharemindExecutionProfiler_new_section_type(
        SharemindExecutionProfiler * profiler,
        const char * name);

uint32_t SharemindExecutionProfiler_start_section(
        SharemindExecutionProfiler * profiler,
        uint32_t section_type,
        uint64_t section_complexity);

void SharemindExecutionProfiler_end_section(
        SharemindExecutionProfiler * profiler,
        uint32_t section_id,
        uint_fast64_t end_time);

void SharemindExecutionProfiler_push_parent_section(
        SharemindExecutionProfiler * profiler,
        uint32_t section_id);

void SharemindExecutionProfiler_pop_parent_section(
        SharemindExecutionProfiler * profiler);

SHAREMIND_EXTERN_C_END
