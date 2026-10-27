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

template<type T>
void publishNumericColumn(string ds, string tbl, uint64 index, bool vlen, T dummy) {
    if (vlen) {
        // publish variable-length values individually
        uint vmap_id = tdbReadColumn(ds, tbl, index);
        uint64 value_count = tdbVmapValueVectorSize(vmap_id, "values");
        publish("value_count", value_count);

        for (uint64 i = 0; i < value_count; ++i) {
            T[[1]] value = tdbVmapGetVlenValue(vmap_id, "values", i);
            publish("value_" + tostring(i), value);
        }
        tdbVmapDelete(vmap_id);
    } else {
        T[[1]] values = tdbReadColumn(ds, tbl, index);
        publish("values", values);
    }
}

void publishStringColumn(string ds, string tbl, uint64 index) {
    // Strings are variable-lenght, publish them individually
    uint vmap_id = tdbReadColumn(ds, tbl, index);
    uint64 value_count = tdbVmapValueVectorSize(vmap_id, "values");
    publish("value_count", value_count);
    for (uint64 i = 0; i < value_count; ++i) {
        string value = tdbVmapGetValue(vmap_id, "values", i);
        publish("value_" + tostring(i), value);
    }
    tdbVmapDelete(vmap_id);
}

void main() {
    string datasource_name = argument("datasource_name");
    string table_name = argument("table_name");
    uint64 column_idx = argument("column_idx");
    string column_type = argument("column_type");
    bool column_vlen = argument("column_vlen");

    if (column_type == "bool") {
        publishNumericColumn(datasource_name, table_name, column_idx, column_vlen, false);
    } else if (column_type == "uint8") {
        publishNumericColumn(datasource_name, table_name, column_idx, column_vlen, 0::uint8);
    } else if (column_type == "uint16") {
        publishNumericColumn(datasource_name, table_name, column_idx, column_vlen, 0::uint16);
    } else if (column_type == "uint32") {
        publishNumericColumn(datasource_name, table_name, column_idx, column_vlen, 0::uint32);
    } else if (column_type == "uint64") {
        publishNumericColumn(datasource_name, table_name, column_idx, column_vlen, 0::uint64);
    } else if (column_type == "int8") {
        publishNumericColumn(datasource_name, table_name, column_idx, column_vlen, 0::int8);
    } else if (column_type == "int16") {
        publishNumericColumn(datasource_name, table_name, column_idx, column_vlen, 0::int16);
    } else if (column_type == "int32") {
        publishNumericColumn(datasource_name, table_name, column_idx, column_vlen, 0::int32);
    } else if (column_type == "int64") {
        publishNumericColumn(datasource_name, table_name, column_idx, column_vlen, 0::int64);
    } else if (column_type == "float32") {
        publishNumericColumn(datasource_name, table_name, column_idx, column_vlen, 0::float32);
    } else if (column_type == "float64") {
        publishNumericColumn(datasource_name, table_name, column_idx, column_vlen, 0::float64);
    } else if (column_type == "string") {
        assert(column_vlen);
        publishStringColumn(datasource_name, table_name, column_idx);
    } else {
        print("Unsupported column type: ", column_type);
        return;
    }
}
