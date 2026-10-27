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
#include <sharemind/mod_tabledb/tdbtypes.h>

#ifdef __cplusplus
#include <cstddef>
#include <cstdint>
#else
#include <stddef.h>
#include <stdint.h>
#endif

SHAREMIND_EXTERN_C_BEGIN

SharemindTdbValue ** SharemindTdbValue_new_ptrarray(size_t num_elements);
void SharemindTdbValue_delete_ptrarray(SharemindTdbValue ** ptr);

SharemindTdbString ** SharemindTdbString_new_ptrarray(size_t num_elements);
void SharemindTdbString_delete_ptrarray(SharemindTdbString ** ptr);

SharemindTdbType ** SharemindTdbType_new_ptrarray(size_t num_elements);
void SharemindTdbType_delete_ptrarray(SharemindTdbType ** ptr);


SHAREMIND_EXTERN_C_END
