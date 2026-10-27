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
 *  @brief Insert a row into a table
 *  @note This function can be used if all columns have the same type.
 *  @param datasource - name of the data source
 *  @param table - name of the table
 *  @param values - row to be inserted
 */
template <type T>
void tdbInsertRow (string datasource, string table, T[[1]] values, bool value_as_column) {
    T dummy;
    string t_dom = "public";
    uint64 t_size = sizeof(dummy);
    __syscall ("tdb_insert_row", __cref datasource, __cref table, __cref t_dom, __cref "$T", t_size, value_as_column, __cref values);
}

/**
 *  @brief Insert a row into a table
 *  @note This function can be used if there is one string column in the table.
 *  @param datasource - name of the data source
 *  @param table - name of the table
 *  @param value - row to be inserted
 */
void tdbInsertRow (string datasource, string table, string value, bool value_as_column) {
    string t_dom = "public";
    string t_name = "string";
    uint64 t_size = 0;
    __syscall ("tdb_insert_row", __cref datasource, __cref table, __cref t_dom, __cref t_name, t_size, value_as_column, __cref value);
}

void main() {
    string datasource_name = argument("datasource_name");
    string table_name = argument("table_name");
    string type_name = argument("type_name");
    bool value_as_column = argument("value_as_column");

    if (type_name == "bool") {
        bool[[1]] values = argument("data_values");
        tdbInsertRow(datasource_name, table_name, values, value_as_column);
    } else if (type_name == "uint8") {
        uint8[[1]] values = argument("data_values");
        tdbInsertRow(datasource_name, table_name, values, value_as_column);
    } else if (type_name == "uint16") {
        uint16[[1]] values = argument("data_values");
        tdbInsertRow(datasource_name, table_name, values, value_as_column);
    } else if (type_name == "uint32") {
        uint32[[1]] values = argument("data_values");
        tdbInsertRow(datasource_name, table_name, values, value_as_column);
    } else if (type_name == "uint64") {
        uint64[[1]] values = argument("data_values");
        tdbInsertRow(datasource_name, table_name, values, value_as_column);
    } else if (type_name == "int8") {
        int8[[1]] values = argument("data_values");
        tdbInsertRow(datasource_name, table_name, values, value_as_column);
    } else if (type_name == "int16") {
        int16[[1]] values = argument("data_values");
        tdbInsertRow(datasource_name, table_name, values, value_as_column);
    } else if (type_name == "int32") {
        int32[[1]] values = argument("data_values");
        tdbInsertRow(datasource_name, table_name, values, value_as_column);
    } else if (type_name == "int64") {
        int64[[1]] values = argument("data_values");
        tdbInsertRow(datasource_name, table_name, values, value_as_column);
    } else if (type_name == "float32") {
        float32[[1]] values = argument("data_values");
        tdbInsertRow(datasource_name, table_name, values, value_as_column);
    } else if (type_name == "float64") {
        float64[[1]] values = argument("data_values");
        tdbInsertRow(datasource_name, table_name, values, value_as_column);
    } else if (type_name == "string") {
        string value = argument("data_values");
        tdbInsertRow(datasource_name, table_name, value, value_as_column);
    } else {
        print("Unsupported column type name: ", type_name);
        return;
    }
}
