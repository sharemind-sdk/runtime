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
    string table_name = argument("table_name");

    uint64 rv = tdbGetColumnTypes(datasource_name, table_name);

    string vec_name = "types";
    uint64 vec_count = tdbVmapCount(rv, vec_name);

    if (vec_count == 1) {
        // There is no tdbVmapTypeVectorSize method, so use the column count
        uint64 col_count = tdbGetColumnCount(datasource_name, table_name);
        publish("names_count", col_count);
        for (uint i = 0; i < col_count; ++i) {
            string type_name = tdbVmapGetTypeName(rv, vec_name, i);
            publish("name_" + tostring(i), type_name);
        }
    } else {
        print("vmap expected to contain 1 vector '", vec_name,"', but contains ", vec_count);
    }
}
