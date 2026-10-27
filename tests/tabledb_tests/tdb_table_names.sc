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

import stdlib;
import table_database;

void main() {
    string datasource_name = argument("datasource_name");
    uint64 rv = tdbGetTableNames(datasource_name);

    string vecname = "names";
    uint64 vec_count = tdbVmapCount(rv, vecname);

    if (vec_count == 1) {
        uint64 names_count = tdbVmapStringVectorSize(rv, vecname);
        publish("names_count", names_count);
        for (uint i = 0; i < names_count; ++i) {
            string name = tdbVmapGetString(rv, vecname, i);
            publish("name_" + tostring(i), name);
        }
    } else {
        print("vmap expected to contain 1 vector '", vecname,"', but contains ", vec_count);
    }
}
