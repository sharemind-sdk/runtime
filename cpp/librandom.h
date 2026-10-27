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

#ifndef SHAREMIND_LIBRANDOM_C
#define SHAREMIND_LIBRANDOM_C

#include <sharemind/extern_c.h>
#include <sharemind/Random/librandom.h>

SHAREMIND_EXTERN_C_BEGIN

SharemindRandomFacility * SharemindRandomFacility_new(
    SharemindRandomEngineConf const * const conf);

void SharemindRandomFacility_free(
    SharemindRandomFacility * const facility);

SHAREMIND_EXTERN_C_END

#endif /* SHAREMIND_LIBRANDOM_C */
