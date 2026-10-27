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

use std::{
    ffi::{c_void, CString},
    ptr::null,
};

use crate::{
    common::container::PointerVector,
    ffi::bindings::{SharemindTdbValue, SharemindTdbValue_delete, SharemindTdbValue_new},
    tabledb_capnp,
};

use super::{
    error::TdbError,
    rpc::{ColumnMetadata, TableMetadata},
};

/// A convenience wrapper for a tabledb value batch.
/// Validates all input value pointers and values upon batch creation.
/// May take ownership of the input value pointers if the inner TdbValuePtrVec is initialized accordingly.
#[derive(Debug)]
pub struct TdbValueBatch {
    values: TdbValuePtrVec,
    value_as_column: bool,
}

impl TdbValueBatch {
    pub fn from_ptr_vec(
        value_ptrs: TdbValuePtrVec,
        value_as_column: bool,
    ) -> Result<Self, TdbError> {
        if value_ptrs.is_empty() {
            return Err(TdbError::invalid_argument(
                "tabledb value batch must contain at least one value",
            ));
        }

        // Validate pointers and values.
        for &ptr in &value_ptrs {
            if ptr.is_null() {
                return Err(TdbError::invalid_argument("tabledb value pointer is null"));
            }

            if !(unsafe { *ptr }).is_valid() {
                return Err(TdbError::invalid_argument("invalid tabledb value"));
            }
        }

        Ok(Self {
            values: value_ptrs,
            value_as_column,
        })
    }

    pub fn from_ptr_slice(
        value_ptr_slice: &[*mut SharemindTdbValue],
        value_as_column: bool,
        take_ownership: bool,
    ) -> Result<Self, TdbError> {
        let ptr_vec = if take_ownership {
            TdbValuePtrVec::with_vec_owned(value_ptr_slice.to_vec())
        } else {
            TdbValuePtrVec::with_vec_unowned(value_ptr_slice.to_vec())
        };
        Self::from_ptr_vec(ptr_vec, value_as_column)
    }

    pub fn values_as_ptrs(&self) -> &[*mut SharemindTdbValue] {
        &self.values
    }

    pub fn values_as_refs<'a>(&self) -> Vec<&'a SharemindTdbValue> {
        // pointers are guaranteed to be non-null
        self.values.iter().map(|&v| unsafe { &*v }).collect()
    }

    pub fn is_value_as_column(&self) -> bool {
        self.value_as_column
    }

    // The number of rows this batch represents
    pub fn row_count(&self) -> usize {
        assert!(
            !self.values.is_empty(),
            "batch must contain at least one value"
        );

        if self.value_as_column {
            // pointers are guaranteed to be non-null
            let first_value = unsafe { &*self.values[0] };
            first_value.element_count()
        } else {
            1
        }
    }
}

pub type TdbValuePtrVec = PointerVector<SharemindTdbValue>;

impl TdbValuePtrVec {
    fn new_owned(capacity: usize) -> Self {
        Self::new(capacity, Some(SharemindTdbValue_delete))
    }

    #[allow(dead_code)]
    fn new_unowned(capacity: usize) -> Self {
        Self::new(capacity, None)
    }

    fn with_vec_owned(vec: Vec<*mut SharemindTdbValue>) -> Self {
        Self::from_vec(vec, Some(SharemindTdbValue_delete))
    }

    fn with_vec_unowned(vec: Vec<*mut SharemindTdbValue>) -> Self {
        Self::from_vec(vec, None)
    }
}

pub fn serialize_batches_to_write_request(
    table_metadata: &TableMetadata,
    value_batches: &[TdbValueBatch],
    mut request: tabledb_capnp::database::write_params::Builder<'_>,
) -> Result<(), TdbError> {
    // Compute the serialized segment size
    let table_col_count = capnp_to_u32(
        table_metadata.get_column_count(),
        "number of columns to serialize",
    )?;

    let inserted_row_count = capnp_to_u32(
        value_batches
            .iter()
            .map(|batch| batch.row_count())
            .sum::<usize>(),
        "number of rows to serialize",
    )?;

    request.set_rows_added_count(inserted_row_count as u64);

    // Get or initialize builder for the list of columns
    let mut columns = if request.has_columns() {
        request
            .reborrow()
            .get_columns()
            .map_err(|e| TdbError::general_error(format!("error getting the columns field: {e}")))?
    } else {
        request.reborrow().init_columns(table_col_count)
    };
    assert!(
        columns.len() == table_col_count,
        "number of columns in RPC request must match the table schema"
    );

    // Initialize the serializer
    let serializer =
        TableRpcSerializer::new(columns.reborrow(), table_col_count, inserted_row_count);

    serialize_batches(table_metadata, value_batches, serializer)
}

/// Serializes the value batches according to translation scheme described in
/// documentation for the tdb_insert_row2 syscall.
fn serialize_batches(
    table_metadata: &TableMetadata,
    value_batches: &[TdbValueBatch],
    mut serializer: impl TableSerializer,
) -> Result<(), TdbError> {
    // Validate the inputs
    if value_batches.is_empty() {
        return Err(TdbError::invalid_argument("no value batches given"));
    }

    // The individual batch values have been validated beforehand in `TdbValueBatch::new()`

    for batch in value_batches {
        let batch_row_count = batch.row_count();
        let mut batch_col_count: usize = 0;

        for (i, &value) in batch.values_as_refs().iter().enumerate() {
            let value_type = unsafe { value.type_as_ref() };

            if value_type.is_variable_length() {
                // Variable length values have element count 1

                if batch.is_value_as_column() && batch_row_count != 1 {
                    return Err(TdbError::invalid_argument(
                        "error serializing variable length value: inconsistent row count for a value batch"));
                }

                // Check that value type matches table schema
                if i >= table_metadata.get_column_count() {
                    return Err(TdbError::invalid_argument(
                        "error serializing variable length value: inconsistent column count for a value batch"));
                }

                let table_column_type = &table_metadata.get_column(i).unwrap().type_;
                let effective_table_column_type =
                    if table_column_type.name == "string" && value_type.name_str() == "uint8" {
                        // Strings are currently stored as byte arrays,
                        // so pretend the string column holds byte arrays
                        let mut t = table_column_type.clone();
                        t.name = "uint8".to_owned();
                        t
                    } else {
                        table_column_type.clone()
                    };

                if effective_table_column_type != *value_type {
                    return Err(TdbError::invalid_argument(
                        "error serializing variable length value: type does not match table schema",
                    ));
                }

                // Serialize
                serializer
                    .serialize_value_as_column(value, i)
                    .map_err(|e| {
                        TdbError::from_err(e, "error serializing variable length value")
                    })?;

                batch_col_count += 1;
            } else {
                assert!(value.size > 0);

                if batch.is_value_as_column() {
                    if value.element_count() != batch_row_count {
                        return Err(TdbError::invalid_argument(
                            "error serializing value as column: inconsistent row count for a value batch"));
                    }

                    // Check that value type matches table schema
                    if i >= table_metadata.get_column_count() {
                        return Err(TdbError::invalid_argument(
                            "error serializing value as column: too many columns in a value batch",
                        ));
                    }

                    if table_metadata.get_column(i).unwrap().type_ != *value_type {
                        return Err(TdbError::invalid_argument(
                            "error serializing value as column: table schema type mismatch",
                        ));
                    }

                    // Serialize
                    serializer
                        .serialize_value_as_column(value, i)
                        .map_err(|e| TdbError::from_err(e, "error serializing value as column"))?;

                    batch_col_count += 1;
                } else {
                    // Check that value type matches table schema
                    let value_column_count = value.element_count();
                    if batch_col_count + value_column_count > table_metadata.get_column_count() {
                        return Err(TdbError::invalid_argument(
                            "error serializing value as row: inconsistent column count for value elements"));
                    }

                    if !table_metadata
                        .get_schema()
                        .iter()
                        .skip(batch_col_count)
                        .take(value_column_count)
                        .all(|col| &col.type_ == value_type)
                    {
                        return Err(TdbError::invalid_argument(
                            "error serializing value as row: table schema type mismatch",
                        ));
                    }

                    // Serialize
                    serializer
                        .serialize_value_as_row(value)
                        .map_err(|e| TdbError::from_err(e, "error serializing value as row"))?;

                    // Assumption: batch has only one value representing one row that covers all table columns.
                    batch_col_count += value_column_count;
                }
            }
        }

        // Check that we have values for all the columns
        if batch_col_count != table_metadata.get_column_count() {
            return Err(TdbError::invalid_argument(
                "given number of values differs from the number of columns",
            ));
        }
    }

    Ok(())
}

/// Convert buffer to a buffer pointer suitable for SharemindTdbValue.
/// SharemindTdbValue_new expects nullptr if size = 0.
fn sharemind_tdb_value_buffer_ptr(buffer: &[u8]) -> *const c_void {
    if buffer.is_empty() {
        null()
    } else {
        buffer.as_ptr() as *const c_void
    }
}

// The reader contains a list of ColumnData structs that all belong to
// the same column. This is because the database backend treats the written
// ColumnData structs as opaque blobs (no deserialization or concatenation occurs),
// stores them in columns one after another and returns them this way when
// the column is read. On the runtime side capnp sees these blobs are formatted
// messages that we can work on directly or translate into desired representation.
pub fn deserialize_column_batch_from_reader(
    reader: ::capnp::struct_list::Reader<tabledb_capnp::column_data::Owned>,
    column_metadata: &ColumnMetadata,
) -> Result<TdbValuePtrVec, TdbError> {
    // Total number of values in all ColumnData structs combined
    let value_count = reader
        .iter()
        .map(|col| -> Result<usize, ::capnp::Error> { Ok(col.get_values()?.len() as usize) })
        .sum::<Result<usize, ::capnp::Error>>()?;

    if column_metadata.type_.is_variable_length() {
        let mut dest_values = TdbValuePtrVec::new_owned(value_count);

        // Deserialize ColumnData structures in order
        for column_data in reader.iter() {
            for src_value in column_data.get_values()?.iter() {
                let src_value_buffer = src_value.get_buffer()?;
                let dest_value_ptr = unsafe {
                    SharemindTdbValue_new(
                        CString::new(column_metadata.type_.domain.clone())?.as_ptr(),
                        CString::new(column_metadata.type_.name.clone())?.as_ptr(),
                        column_metadata.type_.size,
                        sharemind_tdb_value_buffer_ptr(src_value_buffer),
                        src_value_buffer.len() as u64,
                    )
                };

                if dest_value_ptr.is_null() {
                    return Err(TdbError::general_error(
                        "failed to allocate a tabledb value",
                    ));
                }

                dest_values.push(dest_value_ptr);
            }
        }

        Ok(dest_values)
    } else {
        let type_size = column_metadata.type_.size as usize;
        let buffer_size = value_count * type_size;
        let mut buffer: Vec<u8> = Vec::with_capacity(buffer_size);

        // Deserialize ColumnData structures in order by combining them into one buffer
        for column_data in reader.iter() {
            for src_value in column_data.get_values()?.iter() {
                let src_value_buffer = src_value.get_buffer()?;
                if src_value_buffer.len() != type_size {
                    return Err(TdbError::general_error(
                        "tabledb value size does not match column type size in schema",
                    ));
                }
                buffer.extend_from_slice(src_value_buffer)
            }
        }

        assert!(buffer.len() == buffer_size);

        let mut dest_values = TdbValuePtrVec::new_owned(1);

        let dest_value_ptr = unsafe {
            SharemindTdbValue_new(
                CString::new(column_metadata.type_.domain.clone())?.as_ptr(),
                CString::new(column_metadata.type_.name.clone())?.as_ptr(),
                column_metadata.type_.size,
                sharemind_tdb_value_buffer_ptr(&buffer),
                buffer.len() as u64,
            )
        };

        if dest_value_ptr.is_null() {
            return Err(TdbError::general_error(
                "failed to allocate a tabledb value",
            ));
        }

        dest_values.push(dest_value_ptr);
        Ok(dest_values)
    }
}

fn capnp_to_u32<T>(val: T, what: &str) -> Result<u32, TdbError>
where
    u32: std::convert::TryFrom<T>,
{
    val.try_into()
        .map_err(|_| TdbError::general_error(format!("{what} too large for RPC interface")))
}

pub struct TableRpcSerializer<'a> {
    columns_builder: ::capnp::struct_list::Builder<'a, tabledb_capnp::column_data::Owned>,
    column_write_offsets: Vec<u32>,
    max_row_count: u32, // cache for convenience
}

impl<'a> TableRpcSerializer<'a> {
    pub fn new(
        mut columns_builder: ::capnp::struct_list::Builder<'a, tabledb_capnp::column_data::Owned>,
        table_col_count: u32,
        inserted_row_count: u32,
    ) -> Self {
        assert!(
            columns_builder.len() == table_col_count,
            "provided table column count is inconsistent with columns builder"
        );

        // Initialize each column
        for i in 0..table_col_count {
            columns_builder
                .reborrow()
                .get(i)
                .init_values(inserted_row_count);
        }

        Self {
            columns_builder,
            column_write_offsets: vec![0; table_col_count as usize],
            max_row_count: inserted_row_count,
        }
    }
}

trait TableSerializer {
    fn serialize_value_as_column(
        &mut self,
        value: &SharemindTdbValue,
        column_idx: usize,
    ) -> Result<(), TdbError>;

    fn serialize_value_as_row(&mut self, value: &SharemindTdbValue) -> Result<(), TdbError>;
}

impl TableSerializer for TableRpcSerializer<'_> {
    // Works for both VAC and variable length values (basically 1-element VAC)
    fn serialize_value_as_column(
        &mut self,
        value: &SharemindTdbValue,
        column_idx: usize,
    ) -> Result<(), TdbError> {
        // capnp interfaces use u32 as a size type
        let column_idx = capnp_to_u32(column_idx, "column index")?;
        let element_count = capnp_to_u32(value.element_count(), "value element count")?;

        let columns = self.columns_builder.reborrow();
        if column_idx >= columns.len() {
            return Err(TdbError::invalid_argument("column index out of bounds"));
        }

        let mut column_values = columns
            .get(column_idx)
            .get_values()
            .map_err(|e| TdbError::io_error(format!("error getting column values builder: {e}")))?;

        // Enforce maximum row count in the columns
        let column_write_offset = self.column_write_offsets[column_idx as usize];
        if column_write_offset + element_count > self.max_row_count {
            return Err(TdbError::invalid_argument(
                "column cannot fit new value elements",
            ));
        }

        // Store the value elements in the column
        for i in 0..element_count {
            let element = value.get_element(i as usize);
            column_values
                .reborrow()
                .get(column_write_offset + i)
                .set_buffer(element)
        }

        // Commit the new elements in the column
        self.column_write_offsets[column_idx as usize] += element_count;

        Ok(())
    }

    fn serialize_value_as_row(&mut self, value: &SharemindTdbValue) -> Result<(), TdbError> {
        // capnp interfaces use u32 as a size type
        let element_count = capnp_to_u32(value.element_count(), "value element count")?;

        let first_row_offset = self
            .column_write_offsets
            .first()
            .cloned()
            .expect("at least one column must exist");

        // Column window to write row value elements to
        let start_col_idx = self
            .column_write_offsets
            .iter()
            .position(|&row_offset| row_offset != first_row_offset)
            .unwrap_or(0); // columns are uniform, restart at the first column
        let end_col_idx = start_col_idx + element_count as usize;

        // Can row fit the value elements?
        if end_col_idx > self.column_write_offsets.len() {
            return Err(TdbError::invalid_argument(
                "not enough columns available to fit the value elements",
            ));
        }

        // Validate the columns window
        let row_idx = self.column_write_offsets[start_col_idx];
        for &row_offset in &self.column_write_offsets[start_col_idx..end_col_idx] {
            if row_offset >= self.max_row_count {
                return Err(TdbError::invalid_argument("columns cannot fit another row"));
            }

            if row_offset != row_idx {
                return Err(TdbError::general_error(
                    "columns have unequal number of rows written",
                ));
            }
        }

        // Serialize to the columns
        for (i, col_idx) in (start_col_idx..end_col_idx).enumerate() {
            // Retrieve the column
            let col_idx = capnp_to_u32(col_idx, "column index")?;
            let column_values = self
                .columns_builder
                .reborrow()
                .get(col_idx)
                .get_values()
                .map_err(|e| {
                    TdbError::io_error(format!("error getting column values builder: {e}"))
                })?;

            // Store the element in the column
            let element = value.get_element(i);
            column_values.get(row_idx).set_buffer(element);
        }

        // Commit the row
        self.column_write_offsets[start_col_idx..end_col_idx]
            .iter_mut()
            .for_each(|offset| *offset += 1);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::syscalls::tabledb::types::{type_element_size, TdbType};

    use super::*;
    use rand::Rng;
    use tracing_test::traced_test;

    fn generate_test_table_metadata(
        col_count: usize,
        row_count: usize,
        variable_length: bool,
    ) -> TableMetadata {
        let type_size = if variable_length { 0 } else { 2 };
        let default_type = TdbType {
            domain: String::from("public"),
            name: String::from("uint16"),
            size: type_size,
        };

        let schema = (0..col_count)
            .map(|i| ColumnMetadata {
                name: format!("col_{i}"),
                type_: default_type.clone(),
            })
            .collect();

        TableMetadata {
            schema,
            row_count: row_count.try_into().unwrap(),
        }
    }

    fn generate_random_data(data_type: &TdbType) -> Vec<u8> {
        let mut rng = rand::rng();
        let bytes_size = if data_type.is_variable_length() {
            // size is 0 for variable length types, thus derive size from type name
            let element_size = type_element_size(&data_type.name);

            // Generate a random array size (between some min and max size, e.g., 1 to 10)
            let element_count = rng.random_range(1..=10);

            element_size * element_count
        } else {
            data_type.size as usize
        };

        (0..bytes_size).map(|_| rng.random()).collect()
    }

    fn generate_test_table_data(metadata: &TableMetadata) -> Vec<Vec<Vec<u8>>> {
        let column_count = metadata.get_column_count();
        let row_count = metadata.get_row_count();

        let mut columns = vec![Vec::with_capacity(row_count); column_count];
        for (i, column_metadata) in metadata.get_schema().iter().enumerate() {
            for _ in 0..row_count {
                columns[i].push(generate_random_data(&column_metadata.type_));
            }
        }

        columns
    }

    fn columns_to_batches(
        metadata: &TableMetadata,
        columns: &Vec<Vec<Vec<u8>>>,
        value_as_column: bool,
    ) -> Vec<TdbValueBatch> {
        let mut batches = Vec::with_capacity(1);

        if value_as_column {
            // One value per table column
            let mut value_ptrs = TdbValuePtrVec::new_owned(columns.len());

            for (i, column_metadata) in metadata.get_schema().iter().enumerate() {
                let type_domain_cstr = CString::new(column_metadata.type_.domain.clone()).unwrap();
                let type_name_cstr = CString::new(column_metadata.type_.name.clone()).unwrap();

                let column = &columns[i];
                assert_eq!(column.len(), metadata.get_row_count());
                let merged_bytes = column.iter().flatten().cloned().collect::<Vec<u8>>();

                let value_ptr = unsafe {
                    SharemindTdbValue_new(
                        type_domain_cstr.as_ptr(),
                        type_name_cstr.as_ptr(),
                        column_metadata.type_.size,
                        sharemind_tdb_value_buffer_ptr(&merged_bytes),
                        merged_bytes.len() as u64,
                    )
                };

                if value_ptr.is_null() {
                    panic!("failed to allocate a tabledb value");
                }

                value_ptrs.push(value_ptr);
            }

            batches.push(TdbValueBatch::from_ptr_vec(value_ptrs, value_as_column).unwrap());
        } else {
            let column_metadata = metadata.get_column(0).unwrap();
            let type_domain_cstr = CString::new(column_metadata.type_.domain.clone()).unwrap();
            let type_name_cstr = CString::new(column_metadata.type_.name.clone()).unwrap();

            for row_idx in 0..metadata.get_row_count() {
                let mut merged_bytes: Vec<u8> = Vec::new();

                for column in columns {
                    merged_bytes.extend(&column[row_idx]);
                }

                // One value per table row
                let mut value_ptrs = TdbValuePtrVec::new_owned(metadata.get_row_count());

                let value_ptr = unsafe {
                    SharemindTdbValue_new(
                        type_domain_cstr.as_ptr(),
                        type_name_cstr.as_ptr(),
                        column_metadata.type_.size,
                        sharemind_tdb_value_buffer_ptr(&merged_bytes),
                        merged_bytes.len() as u64,
                    )
                };

                if value_ptr.is_null() {
                    panic!("failed to allocate a tabledb value");
                }

                value_ptrs.push(value_ptr);
                batches.push(TdbValueBatch::from_ptr_vec(value_ptrs, value_as_column).unwrap());
            }
        }

        batches
    }

    // Each table row of vlen values will become a separate batch.
    // Reason: a vlen value takes up the whole value data buffer and cannot encode arrays of column values.
    fn columns_to_batches_vlen(
        metadata: &TableMetadata,
        columns: &[Vec<Vec<u8>>],
        value_as_column: bool,
    ) -> Vec<TdbValueBatch> {
        let mut batches = Vec::with_capacity(1);

        #[allow(clippy::needless_range_loop)]
        for row_idx in 0..metadata.get_row_count() {
            let mut value_ptrs = TdbValuePtrVec::new_owned(metadata.get_column_count());

            for (col_idx, column_metadata) in metadata.get_schema().iter().enumerate() {
                let type_domain_cstr = CString::new(column_metadata.type_.domain.clone()).unwrap();
                let type_name_cstr = CString::new(column_metadata.type_.name.clone()).unwrap();
                let value_data = &columns[col_idx][row_idx];

                let value_ptr = unsafe {
                    SharemindTdbValue_new(
                        type_domain_cstr.as_ptr(),
                        type_name_cstr.as_ptr(),
                        column_metadata.type_.size,
                        sharemind_tdb_value_buffer_ptr(value_data),
                        value_data.len() as u64,
                    )
                };

                if value_ptr.is_null() {
                    panic!("failed to allocate a tabledb value");
                }

                value_ptrs.push(value_ptr);
            }
            batches.push(TdbValueBatch::from_ptr_vec(value_ptrs, value_as_column).unwrap());
        }
        batches
    }

    fn validate_serialized_data(
        reference_columns: &[Vec<Vec<u8>>],
        serialized_columns: capnp::struct_list::Reader<'_, tabledb_capnp::column_data::Owned>,
    ) {
        assert_eq!(reference_columns.len(), serialized_columns.len() as usize);

        for (i, column) in serialized_columns.iter().enumerate() {
            assert!(column.has_values());
            let values = column.get_values().unwrap();
            assert_eq!(reference_columns[i].len(), values.len() as usize);
            for (j, value) in values.iter().enumerate() {
                assert!(value.has_buffer());
                let serialized_value_data = value.get_buffer().unwrap();
                if reference_columns[i][j] != serialized_value_data {
                    panic!("invalid data serialized at column {i} and row {j}: original {:?}, serialized {:?}", reference_columns[i][j], serialized_value_data);
                }
            }
        }
    }

    fn validate_deserialized_data(
        reference_column: Vec<Vec<u8>>,
        deserialized_values: TdbValuePtrVec,
    ) {
        let mut row_idx = 0;
        for &value_ptr in &deserialized_values {
            assert!(!value_ptr.is_null());
            let value = unsafe { &*value_ptr };
            let element_count = value.element_count();

            for i in 0..element_count {
                let deser_value_element = value.get_element(i);
                if reference_column[row_idx] != deser_value_element {
                    panic!(
                        "invalid data deserialized at row {row_idx}: reference {:?}, deserialized {:?}",
                        &reference_column[row_idx],
                        deser_value_element
                    );
                }
                row_idx += 1;
            }
        }
    }

    fn serialization_test(variable_length: bool, value_as_column: bool) {
        let col_count = 3;
        let row_count = 5;

        // Generate test data
        let table_metadata = generate_test_table_metadata(col_count, row_count, variable_length);
        let reference_columns = generate_test_table_data(&table_metadata);

        // Convert reference data to input value batches
        let value_batches = if variable_length {
            columns_to_batches_vlen(&table_metadata, &reference_columns, value_as_column)
        } else {
            columns_to_batches(&table_metadata, &reference_columns, value_as_column)
        };

        // Create output message serving as serialization target
        let mut message = capnp::message::Builder::new_default();
        let mut write_request =
            message.init_root::<tabledb_capnp::database::write_params::Builder<'_>>();

        // Serialize test data into the write request
        serialize_batches_to_write_request(
            &table_metadata,
            &value_batches,
            write_request.reborrow(),
        )
        .unwrap();

        // Check serialization result
        assert!(write_request.has_columns());
        let serialized_columns = write_request.reborrow_as_reader().get_columns().unwrap();
        validate_serialized_data(&reference_columns, serialized_columns);

        // Deserialize a list of column data.
        // Pretend all serialized columns data instances belong to the same column,
        // i.e. simulate multiple write requests.
        let deserialized_values = deserialize_column_batch_from_reader(
            serialized_columns,
            table_metadata.get_column(0).unwrap(),
        )
        .unwrap();

        // Check deserialization result.
        // Interpret all original columns as one (see note at deserialization step).
        let merged_column = reference_columns
            .iter()
            .flatten()
            .cloned()
            .collect::<Vec<Vec<u8>>>();
        validate_deserialized_data(merged_column, deserialized_values);
    }

    #[test]
    #[traced_test]
    fn batch_serialization_vac() {
        serialization_test(false, true);
    }

    #[test]
    #[traced_test]
    fn batch_serialization_novac() {
        serialization_test(false, false);
    }

    #[test]
    #[traced_test]
    fn batch_serialization_vlen_vac() {
        serialization_test(true, true);
    }

    #[test]
    #[traced_test]
    fn batch_serialization_vlen_novac() {
        serialization_test(true, false);
    }
}
