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
#include <sharemind/DataStoreFactory.h>

#include "libdatastoremanager.h"

using namespace sharemind;

extern "C" {

SharemindDataStoreFactory * SharemindDataStoreFactory_new() {
    auto factory = new DataStoreFactory();
    return &factory->wrapper();
}

void SharemindDataStoreFactory_free(SharemindDataStoreFactory * const factory) {
    assert(factory);
    delete &DataStoreFactory::fromWrapper(*factory);
}

} // extern "C"