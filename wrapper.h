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

#include <sharemind/datastoreapi.h>
#include <sharemind/libmodapi/libmodapi.h>
#include <sharemind/libprocessfacility.h>
#ifdef ENABLE_TABLEDB
#include <sharemind/mod_tabledb/tdberror.h>
#include <sharemind/mod_tabledb/TdbTypesUtil.h>
#include <sharemind/mod_tabledb/tdbvectormapapi.h>
#include "TdbTypesUtil2.h"
#endif
#ifdef ENABLE_PROFILER
#include "libexecutionprofiler.h"
#endif
#include "libdatastoremanager.h"
#include "singleprocess.h"


// Some fixes for Clang complaints
struct LogHardBackend;
typedef struct LogHardBackend LogHardBackend;
struct LogHardLogger;
typedef struct LogHardLogger LogHardLogger;
struct SharemindCryptographicRandom;
typedef struct SharemindCryptographicRandom SharemindCryptographicRandom;

#include <LogHard/BackendC.h>
#include <LogHard/LoggerC.h>
#include <sharemind/libpd.h>

#include <librandom.h>

