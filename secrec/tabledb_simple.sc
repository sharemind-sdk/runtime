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

import shared3p;
import shared3p_table_database;
import stdlib;
import table_database;

domain pd_shared3p shared3p;

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
    string ds = "DS1"; // Data source name
    string tbl = "table"; // Table name

    // Open database before running operations on it
    tdbOpenConnection(ds);

    // Check if a table exists
    if (tdbTableExists(ds, tbl)) {
        // Delete existing table
        print("Deleting existing table: ", tbl);
        tdbTableDelete(ds, tbl);
    }

    // Create a new table with homogeneous data
    print("Creating new table: ", tbl);
    uint ncols = 10;
    uint vtype;
    tdbTableCreate(ds, tbl, vtype, ncols);
    print("Created a table");

    // Insert data rows
    uint nrows = 5;
    uint [[1]] row(ncols);
    for (uint i = 0; i < nrows; ++i) {
        row = i;
        tdbInsertRow(ds, tbl, row, false);
        print("Inserting row [", i, "]: ", _vectorToString(row));
    }

    // Read table info
    print("Table name: ", tbl);
    print("Columns: ", tdbGetColumnCount(ds, tbl));
    print("Rows: ", tdbGetRowCount(ds, tbl));

    for (uint i = 0; i < tdbGetColumnCount(ds, tbl); ++i) {
        uint vmap_id = tdbReadColumn(ds, tbl, i);
        uint64 value_count = tdbVmapValueVectorSize(vmap_id, "values");
        for (uint64 j = 0; j < value_count; ++j) {
            uint64[[1]] values = tdbVmapGetValue(vmap_id, "values", j);
            print("values_" + tostring(j) + "=", _vectorToString(values));
        }
    }

    // Close the database. All databases will also be closed automatically on
    // exit.
    tdbCloseConnection(ds);
}
