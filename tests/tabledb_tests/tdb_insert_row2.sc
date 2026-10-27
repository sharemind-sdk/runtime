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
void addValueToVmap(uint64 vmap, T value) {
    tdbVmapAddValue(vmap, "values", value);
}

void addValueToVmap(uint64 vmap, string str) {
    tdbVmapAddVlenValue(vmap, "values", __bytes_from_string(str));
}

template<type T>
void addValueToVmap(uint64 vmap, T[[1]] values, bool is_vlen) {
    if (is_vlen) {
        tdbVmapAddVlenValue(vmap, "values", values);
    } else {
        tdbVmapAddValue(vmap, "values", values);
    }
}

void main() {
    string datasource_name = argument("datasource_name");
    string table_name = argument("table_name");
    uint64 value_object_count = argument("value_object_count");

    // Table should have been created by the test beforehand
    assert(tdbTableExists(datasource_name, table_name));

    // Create a new vector map for storing arguments to the table row insertion call.
    uint64 params_vmap = tdbVmapNew();

    // Input arguments:
    //  batch_id_i      // batch id for the value object
    //  batch_vac_i     // use for tdbVmapSetValueAsColumn(params_vmap) with every new batch
    //  value_type_i    // typename for the value object
    //  value_data_i    // raw byte buffer for the value object
    //  value_vlen_i    // use tdbVmapAddValue() or tdbVmapAddVlenValue()

    // Process input arguments
    uint64 prev_batch_id = 0;
    for (uint i = 0; i < value_object_count; ++i) {
        uint64 batch_id = argument("batch_id_" + tostring(i));
        bool batch_vac = argument("batch_vac_" + tostring(i));
        string type_name = argument("value_type_" + tostring(i));
        bool is_vlen = argument("value_vlen_" + tostring(i));

        // init first batch
        if (i == 0) {
            if (batch_vac) {
                tdbVmapSetValueAsColumn(params_vmap);
            }
        }

        // new batch detected
        if (prev_batch_id != batch_id) {
            assert(prev_batch_id < batch_id);
            tdbVmapAddBatch(params_vmap);
            if (batch_vac) {
                tdbVmapSetValueAsColumn(params_vmap);
            }
        }

        // Add value to params_vmap based on type info
        if (type_name == "bool") {
            bool[[1]] values = argument("value_data_" + tostring(i));
            addValueToVmap(params_vmap, values, is_vlen);
        } else if (type_name == "uint8") {
            uint8[[1]] values = argument("value_data_" + tostring(i));
            addValueToVmap(params_vmap, values, is_vlen);
        } else if (type_name == "uint16") {
            uint16[[1]] values = argument("value_data_" + tostring(i));
            addValueToVmap(params_vmap, values, is_vlen);
        } else if (type_name == "uint32") {
            uint32[[1]] values = argument("value_data_" + tostring(i));
            addValueToVmap(params_vmap, values, is_vlen);
        } else if (type_name == "uint64") {
            uint64[[1]] values = argument("value_data_" + tostring(i));
            addValueToVmap(params_vmap, values, is_vlen);
        } else if (type_name == "int8") {
            int8[[1]] values = argument("value_data_" + tostring(i));
            addValueToVmap(params_vmap, values, is_vlen);
        } else if (type_name == "int16") {
            int16[[1]] values = argument("value_data_" + tostring(i));
            addValueToVmap(params_vmap, values, is_vlen);
        } else if (type_name == "int32") {
            int32[[1]] values = argument("value_data_" + tostring(i));
            addValueToVmap(params_vmap, values, is_vlen);
        } else if (type_name == "int64") {
            int64[[1]] values = argument("value_data_" + tostring(i));
            addValueToVmap(params_vmap, values, is_vlen);
        } else if (type_name == "float32") {
            float32[[1]] values = argument("value_data_" + tostring(i));
            addValueToVmap(params_vmap, values, is_vlen);
        } else if (type_name == "float64") {
            float64[[1]] values = argument("value_data_" + tostring(i));
            addValueToVmap(params_vmap, values, is_vlen);
        } else if (type_name == "string") {
            string value = argument("value_data_" + tostring(i));
            addValueToVmap(params_vmap, value);
        } else {
            print("Unsupported type name: ", type_name);
            return;
        }

        prev_batch_id = batch_id;
    }

    tdbInsertRow(datasource_name, table_name, params_vmap);
}
