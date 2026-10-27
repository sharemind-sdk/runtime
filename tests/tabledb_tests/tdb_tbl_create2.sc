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

template <domain D, type T>
void vmapAddColumn (uint params_vmap, D T vtype, string name) {
    tdbVmapAddType(params_vmap, "types", vtype);
    tdbVmapAddString(params_vmap, "names", name);
}

void main() {
    string datasource_name = argument("datasource_name");
    string table_name = argument("table_name");
    uint64 column_count = argument("column_count");

    // Create a new "vector map" for storing arguments to the table creation call.
    uint64 params_vmap = tdbVmapNew();

    for (uint i = 0; i < column_count; ++i) {
        string column_name = argument("column_name_" + tostring(i));
        string type_name = argument("type_name_" + tostring(i));
        string type_domain = argument("type_domain_" + tostring(i));

        if (type_domain == "public") {
            if (type_name == "bool") {
                bool data_type;
                vmapAddColumn(params_vmap, data_type, column_name);
            } else if (type_name == "uint8") {
                uint8 data_type;
                vmapAddColumn(params_vmap, data_type, column_name);
            } else if (type_name == "uint16") {
                uint16 data_type;
                vmapAddColumn(params_vmap, data_type, column_name);
            } else if (type_name == "uint32") {
                uint32 data_type;
                vmapAddColumn(params_vmap, data_type, column_name);
            } else if (type_name == "uint64") {
                uint64 data_type;
                vmapAddColumn(params_vmap, data_type, column_name);
            } else if (type_name == "int8") {
                int8 data_type;
                vmapAddColumn(params_vmap, data_type, column_name);
            } else if (type_name == "int16") {
                int16 data_type;
                vmapAddColumn(params_vmap, data_type, column_name);
            } else if (type_name == "int32") {
                int32 data_type;
                vmapAddColumn(params_vmap, data_type, column_name);
            } else if (type_name == "int64") {
                int64 data_type;
                vmapAddColumn(params_vmap, data_type, column_name);
            } else if (type_name == "float32") {
                float32 data_type;
                vmapAddColumn(params_vmap, data_type, column_name);
            } else if (type_name == "float64") {
                float64 data_type;
                vmapAddColumn(params_vmap, data_type, column_name);
            } else {
                print("Unsupported column type name: ", type_name);
                return;
            }
        } else {
            print("Unsupported column type domain: ", type_domain);
            return;
        }
    }

    tdbTableCreate(datasource_name, table_name, params_vmap);

    // Free the parameter map
    tdbVmapDelete(params_vmap);
}
