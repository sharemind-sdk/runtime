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



/**
 *  @brief Create a table where all columns have the same type
 *  @note Supported types - \ref bool "bool" / \ref uint8 "uint8" / \ref uint16 "uint16" / \ref uint32 "uint32" / \ref uint64 "uint" / \ref int8 "int8" / \ref int16 "int16" / \ref int32 "int32" / \ref int64 "int" / \ref float32 "float32" / \ref float64 "float64"
 *  @param datasource - name of the data source
 *  @param table - name of the table
 *  @param vtype - the type of this value will be the type of the columns
 *  @param ncols - number of columns in the created table
 */
template <domain D, type T>
void tdbTableCreate (string datasource, string table, D T vtype, uint64 ncols) {
    uint64 t_size = sizeof(vtype);
    __syscall ("tdb_tbl_create", __cref datasource, __cref table, __cref "$D", __cref "$T", t_size, ncols);
}

void main() {
    string datasource_name = argument("datasource_name");
    string table_name = argument("table_name");
    uint64 column_count = argument("column_count");
    string type_name = argument("type_name");
    string type_domain = argument("type_domain");

    if (type_domain == "public") {
        if (type_name == "bool") {
            bool data_type;
            tdbTableCreate(datasource_name, table_name, data_type, column_count);
        } else if (type_name == "uint8") {
            uint8 data_type;
            tdbTableCreate(datasource_name, table_name, data_type, column_count);
        } else if (type_name == "uint16") {
            uint16 data_type;
            tdbTableCreate(datasource_name, table_name, data_type, column_count);
        } else if (type_name == "uint32") {
            uint32 data_type;
            tdbTableCreate(datasource_name, table_name, data_type, column_count);
        } else if (type_name == "uint64") {
            uint64 data_type;
            tdbTableCreate(datasource_name, table_name, data_type, column_count);
        } else if (type_name == "int8") {
            int8 data_type;
            tdbTableCreate(datasource_name, table_name, data_type, column_count);
        } else if (type_name == "int16") {
            int16 data_type;
            tdbTableCreate(datasource_name, table_name, data_type, column_count);
        } else if (type_name == "int32") {
            int32 data_type;
            tdbTableCreate(datasource_name, table_name, data_type, column_count);
        } else if (type_name == "int64") {
            int64 data_type;
            tdbTableCreate(datasource_name, table_name, data_type, column_count);
        } else if (type_name == "float32") {
            float32 data_type;
            tdbTableCreate(datasource_name, table_name, data_type, column_count);
        } else if (type_name == "float64") {
            float64 data_type;
            tdbTableCreate(datasource_name, table_name, data_type, column_count);
        } else {
            print("Unsupported column type name: ", type_name);
            return;
        }
    } else {
        print("Unsupported column type domain: ", type_domain);
        return;
    }
}
