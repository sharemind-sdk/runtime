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

#include "TdbTypesUtil2.h"

extern "C" {

SharemindTdbValue ** SharemindTdbValue_new_ptrarray(size_t num_elements) {
    return new SharemindTdbValue * [num_elements];
}

void SharemindTdbValue_delete_ptrarray(SharemindTdbValue ** ptr) {
    delete[] ptr;
}

SharemindTdbString ** SharemindTdbString_new_ptrarray(size_t num_elements) {
    return new SharemindTdbString * [num_elements];
}

void SharemindTdbString_delete_ptrarray(SharemindTdbString ** ptr) {
    delete[] ptr;
}

SharemindTdbType ** SharemindTdbType_new_ptrarray(size_t num_elements) {
    return new SharemindTdbType * [num_elements];
}

void SharemindTdbType_delete_ptrarray(SharemindTdbType ** ptr) {
    delete[] ptr;
}


} // extern "C"