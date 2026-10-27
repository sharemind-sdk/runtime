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

#ifndef SHAREMIND_VM_SINGLEPROCESS_H
#define SHAREMIND_VM_SINGLEPROCESS_H

#include <stdint.h>
#include <sharemind/extern_c.h>
#include <sharemind/module-apis/api_0x1.h>

SHAREMIND_EXTERN_C_BEGIN

typedef struct FinderMut_ FinderMut;
typedef struct FinderConst_ FinderConst;

struct FinderMut_ {
    void *(* const find)(const FinderMut *, const char *);
};

struct FinderConst_ {
    void const *(* const find)(const FinderConst *, const char *);
};

typedef enum {
    PROCESS_EXIT_OK,
    PROCESS_EXIT_TRAP_ERROR,
    PROCESS_EXIT_SYSCALL_ERROR,
    PROCESS_EXIT_RUNTIME_ERROR,
    PROCESS_EXIT_UNKNOWN_ERROR,
    PROCESS_EXIT_SPAWN_ERROR
} ProcessExitStatus;

//void spawn(char const * filename, void * (*facilityfinder)(char const *));
ProcessExitStatus spawn(char const * filename, const FinderMut * facilityfinder, const FinderConst * syscallfinder);

struct RustProcess_;
typedef struct RustProcess_ RustProcess;

struct RustProcess_ {
    const SharemindModuleApi0x1PdpiInfo * (* get_pdpi_info)(RustProcess * facility, uint64_t pd_index);
};

SHAREMIND_EXTERN_C_END

#endif /* SHAREMIND_VM_SINGLEPROCESS_H */
