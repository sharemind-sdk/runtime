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

use crate::{syscalls::tabledb::types::TdbType, tabledb_capnp};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub name: String,
    pub type_: TdbType,
}

impl Column {
    pub fn new(name: &str, type_: TdbType) -> Self {
        Self {
            name: name.to_owned(),
            type_,
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct ColumnData {
    blobs: Vec<Vec<u8>>,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Table {
    pub schema: Vec<Column>,
    pub column_data: Vec<ColumnData>,
    // mapping from column name to column index
    column_lookup_index: HashMap<String, usize>,
    pub row_count: u64,
}

impl Table {
    pub fn with_capacity(column_count: usize) -> Self {
        Self {
            schema: Vec::with_capacity(column_count),
            column_data: Vec::with_capacity(column_count),
            column_lookup_index: HashMap::with_capacity(column_count),
            row_count: 0,
        }
    }

    pub fn from_vec(columns: Vec<Column>) -> Result<Self, MockDatabaseError> {
        let mut table = Table::with_capacity(columns.len());
        for column in columns {
            table.add_column(column)?;
        }
        Ok(table)
    }

    pub fn column_count(&self) -> usize {
        self.schema.len()
    }

    pub fn add_column(&mut self, column: Column) -> Result<(), MockDatabaseError> {
        if self.column_lookup_index.contains_key(&column.name) {
            return Err(MockDatabaseError::OperationError(format!(
                "column {} already exists",
                column.name
            )));
        }

        let column_name = column.name.clone();
        self.schema.push(column);
        self.column_data.push(ColumnData::default());
        self.column_lookup_index
            .insert(column_name, self.schema.len() - 1);

        Ok(())
    }
}

pub type DatasourceTableMap = HashMap<String, Table>;

#[derive(Debug)]
pub struct MockDatabaseState {
    // mapping from datasource name to a datasource tables map
    datasources: HashMap<String, DatasourceTableMap>,
}

impl MockDatabaseState {
    pub fn new() -> Self {
        Self {
            datasources: HashMap::new(),
        }
    }

    pub fn read_datasource_tables<F>(
        &self,
        datasource_name: &str,
        read_fn: F,
    ) -> Result<(), MockDatabaseError>
    where
        F: FnOnce(&DatasourceTableMap) -> Result<(), MockDatabaseError>,
    {
        match self.datasources.get(datasource_name) {
            Some(ds_tables) => read_fn(ds_tables),
            None => Err(MockDatabaseError::OperationError(format!(
                "data source {} not found",
                datasource_name
            ))),
        }
    }

    pub fn modify_datasource_tables<F>(
        &mut self,
        datasource_name: &str,
        create_if_missing: bool,
        modify_fn: F,
    ) -> Result<(), MockDatabaseError>
    where
        F: FnOnce(&mut DatasourceTableMap) -> Result<(), MockDatabaseError>,
    {
        let ds_tables = if create_if_missing {
            self.datasources
                .entry(datasource_name.to_owned())
                .or_default()
        } else {
            self.datasources.get_mut(datasource_name).ok_or_else(|| {
                MockDatabaseError::OperationError(format!(
                    "data source {} not found",
                    datasource_name
                ))
            })?
        };

        modify_fn(ds_tables)
    }

    pub fn read_table<F>(
        &self,
        datasource_name: &str,
        table_name: &str,
        read_fn: F,
    ) -> Result<(), MockDatabaseError>
    where
        F: FnOnce(&Table) -> Result<(), MockDatabaseError>,
    {
        self.read_datasource_tables(datasource_name, |ds_tables| {
            match ds_tables.get(table_name) {
                Some(table) => read_fn(table),
                None => Err(MockDatabaseError::OperationError(format!(
                    "table {} not found in data source {}",
                    table_name, datasource_name
                ))),
            }
        })
    }

    pub fn modify_table<F>(
        &mut self,
        datasource_name: &str,
        table_name: &str,
        modify_fn: F,
    ) -> Result<(), MockDatabaseError>
    where
        F: FnOnce(&mut Table) -> Result<(), MockDatabaseError>,
    {
        self.modify_datasource_tables(datasource_name, false, |ds_tables| {
            match ds_tables.get_mut(table_name) {
                Some(table) => modify_fn(table),
                None => Err(MockDatabaseError::OperationError(format!(
                    "table {} not found in data source {}",
                    table_name, datasource_name
                ))),
            }
        })
    }
}

pub struct MockDatabaseServer {
    /// The state is shared with external code to enable state observability.
    state: Rc<RefCell<MockDatabaseState>>,
}

impl MockDatabaseServer {
    /// Create a mock database with a shared state.
    pub fn new(state: Rc<RefCell<MockDatabaseState>>) -> Self {
        Self { state }
    }

    pub fn into_client(self) -> tabledb_capnp::database::Client {
        capnp_rpc::new_client(self)
    }
}

#[derive(Debug)]
pub enum MockDatabaseError {
    OperationError(String),
    CapnpError(::capnp::Error),
}

impl From<::capnp::Error> for MockDatabaseError {
    fn from(err: ::capnp::Error) -> Self {
        MockDatabaseError::CapnpError(err)
    }
}

// DB communication contained invalid string data, probably not capnp fault.
impl From<std::str::Utf8Error> for MockDatabaseError {
    fn from(err: std::str::Utf8Error) -> Self {
        MockDatabaseError::OperationError(err.to_string())
    }
}

struct TableParam {
    table_name: String,
    datasource_name: String,
}

impl TableParam {
    fn from_reader(reader: tabledb_capnp::table::Reader<'_>) -> ::capnp::Result<TableParam> {
        Ok(TableParam {
            table_name: reader.get_name()?.to_string()?,
            datasource_name: reader.get_datasource()?.to_string()?,
        })
    }
}

fn set_default_status_ok_value(
    status_builder: &mut tabledb_capnp::database::result::status::Builder<
        ::capnp::any_pointer::Owned,
    >,
) -> ::capnp::Result<()> {
    let mut msg = ::capnp::message::Builder::new_default();
    let mut temp_root = msg.initn_root::<::capnp::primitive_list::Builder<i32>>(1);
    temp_root.set(0, 42);

    status_builder.set_status_ok(msg.get_root_as_reader::<::capnp::any_pointer::Reader>()?)
}

fn serialize_column_data_to_bytes(
    column_reader: tabledb_capnp::column_data::Reader,
) -> ::capnp::Result<Vec<u8>> {
    // We need a Builder to utilize capnp's serialization.
    // I could not find a better way than just copy data from Reader to Builer.
    let mut message = ::capnp::message::Builder::new_default();
    let mut builder = message.init_root::<tabledb_capnp::column_data::Builder>();
    builder.set_values(column_reader.get_values()?)?;
    Ok(::capnp::serialize::write_message_to_words(&message))
}

fn deserialize_column_data_from_bytes(
    bytes: &[u8],
    column_data_builder: &mut tabledb_capnp::column_data::Builder,
) -> ::capnp::Result<()> {
    // Deserialize blob into column data message
    let message = ::capnp::serialize::read_message(bytes, ::capnp::message::ReaderOptions::new())?;
    let column_data_reader = message.get_root::<tabledb_capnp::column_data::Reader>()?;
    column_data_builder.set_values(column_data_reader.get_values()?)
}

fn report_error<S: capnp::traits::Owned>(
    status_builder: &mut tabledb_capnp::database::result::status::Builder<S>,
    error_msg: String,
) {
    tracing::error!("Mock database error: {}", error_msg);
    status_builder.set_status_err(());
}

impl tabledb_capnp::database::Server for MockDatabaseServer {
    async fn write(
        self: Rc<Self>,
        params: tabledb_capnp::database::WriteParams,
        mut results: tabledb_capnp::database::WriteResults,
    ) -> ::capnp::Result<()> {
        let params = params.get()?;
        let mut status_builder = results.get().init_result().init_status();

        let TableParam {
            table_name,
            datasource_name,
        } = TableParam::from_reader(params.get_table()?)?;

        let result = self
            .state
            .borrow_mut()
            .modify_table(&datasource_name, &table_name, |table| {
                // Validate number of columns
                let columns_reader = params.get_columns()?;
                if table.column_count() != columns_reader.len() as usize {
                    return Err(MockDatabaseError::OperationError(format!(
                        "table {} has a different number of columns than provided",
                        &table_name
                    )));
                }

                // Serialize column datas as blobs
                let mut column_blobs = Vec::with_capacity(table.column_count());
                for column in columns_reader.iter() {
                    let blob = serialize_column_data_to_bytes(column)?;
                    column_blobs.push(blob);
                }

                // Commit the blobs into the table
                for (table_column, column_blob) in table.column_data.iter_mut().zip(column_blobs) {
                    table_column.blobs.push(column_blob);
                }
                table.row_count += params.get_rows_added_count();
                Ok(())
            });

        match result {
            Ok(_) => set_default_status_ok_value(&mut status_builder),
            Err(MockDatabaseError::OperationError(e)) => {
                report_error(&mut status_builder, e);
                Ok(())
            }
            Err(MockDatabaseError::CapnpError(e)) => Err(e),
        }
    }

    async fn read(
        self: Rc<Self>,
        params: tabledb_capnp::database::ReadParams,
        mut results: tabledb_capnp::database::ReadResults,
    ) -> ::capnp::Result<()> {
        let params = params.get()?;
        let mut status_builder = results.get().init_result().init_status();

        let TableParam {
            table_name,
            datasource_name,
        } = TableParam::from_reader(params.get_table()?)?;

        let result = self
            .state
            .borrow()
            .read_table(&datasource_name, &table_name, |table| {
                // Get column blobs
                let column_idx = params.get_column_idx() as usize;
                if column_idx + 1 > table.column_count() {
                    return Err(MockDatabaseError::OperationError(
                        "invalid column index provided".to_owned(),
                    ));
                }
                let column_blobs = &table.column_data[column_idx].blobs;

                // Write deserialized column blobs in result
                let mut column_datas_builder = status_builder
                    .reborrow()
                    .initn_status_ok(column_blobs.len() as u32);
                for (index, blob) in column_blobs.iter().enumerate() {
                    let mut column_data_builder = column_datas_builder.reborrow().get(index as u32);
                    deserialize_column_data_from_bytes(blob, &mut column_data_builder)?;
                }
                Ok(())
            });

        match result {
            Ok(_) => Ok(()),
            Err(MockDatabaseError::OperationError(e)) => {
                report_error(&mut status_builder, e);
                Ok(())
            }
            Err(MockDatabaseError::CapnpError(e)) => Err(e),
        }
    }

    async fn create_table(
        self: Rc<Self>,
        params: tabledb_capnp::database::CreateTableParams,
        mut results: tabledb_capnp::database::CreateTableResults,
    ) -> ::capnp::Result<()> {
        let params = params.get()?;
        let mut status_builder = results.get().init_result().init_status();

        let TableParam {
            table_name,
            datasource_name,
        } = TableParam::from_reader(params.get_table()?)?;

        let result =
            self.state
                .borrow_mut()
                .modify_datasource_tables(&datasource_name, true, |ds_tables| {
                    // Check that table name is unique
                    if ds_tables.contains_key(&table_name) {
                        return Err(MockDatabaseError::OperationError(format!(
                            "table {} already exists in data source {}",
                            &table_name, &datasource_name
                        )));
                    }

                    // Create table object
                    let params_schema = params.get_schema()?;
                    let columns = params_schema.get_columns()?;

                    let mut table = Table::with_capacity(columns.len() as usize);
                    for c in columns.iter() {
                        let column_name = c.get_name()?.to_str()?;
                        let type_reader = c.get_type()?;

                        let column_type = TdbType {
                            domain: type_reader.get_domain()?.to_string()?,
                            name: type_reader.get_name()?.to_string()?,
                            size: type_reader.get_size(),
                        };
                        table.add_column(Column::new(column_name, column_type))?;
                    }

                    // Store table object
                    ds_tables.insert(table_name.clone(), table);
                    Ok(())
                });

        match result {
            Ok(_) => set_default_status_ok_value(&mut status_builder),
            Err(MockDatabaseError::OperationError(e)) => {
                report_error(&mut status_builder, e);
                Ok(())
            }
            Err(MockDatabaseError::CapnpError(e)) => Err(e),
        }
    }

    async fn delete_table(
        self: Rc<Self>,
        params: tabledb_capnp::database::DeleteTableParams,
        mut results: tabledb_capnp::database::DeleteTableResults,
    ) -> ::capnp::Result<()> {
        let params = params.get()?;
        let mut status_builder = results.get().init_result().init_status();

        let TableParam {
            table_name,
            datasource_name,
        } = TableParam::from_reader(params.get_table()?)?;

        let result = self.state.borrow_mut().modify_datasource_tables(
            &datasource_name,
            false,
            |ds_tables| {
                // Delete table from data source
                if ds_tables.remove(&table_name).is_none() {
                    return Err(MockDatabaseError::OperationError(format!(
                        "table {} not found in data source {}",
                        table_name, datasource_name
                    )));
                }

                Ok(())
            },
        );

        match result {
            Ok(_) => {
                // Write ok result
                set_default_status_ok_value(&mut status_builder)
            }
            Err(MockDatabaseError::OperationError(e)) => {
                report_error(&mut status_builder, e);
                Ok(())
            }
            Err(MockDatabaseError::CapnpError(e)) => Err(e),
        }
    }

    async fn get_table_names(
        self: Rc<Self>,
        params: tabledb_capnp::database::GetTableNamesParams,
        mut results: tabledb_capnp::database::GetTableNamesResults,
    ) -> ::capnp::Result<()> {
        let datasource_name = params.get()?.get_datasource()?.to_string()?;
        let mut status_builder = results.get().init_result().init_status();

        let result = self
            .state
            .borrow()
            .read_datasource_tables(&datasource_name, |ds_tables| {
                // Write table names to result
                // Note: original table order is not maintained by the hashmap

                let mut names_builder = status_builder
                    .reborrow()
                    .initn_status_ok(ds_tables.len() as u32);
                for (index, table_name) in ds_tables.keys().enumerate() {
                    names_builder.reborrow().set(index as u32, table_name);
                }
                Ok(())
            });

        match result {
            Ok(_) => Ok(()),
            Err(MockDatabaseError::OperationError(e)) => {
                report_error(&mut status_builder, e);
                Ok(())
            }
            Err(MockDatabaseError::CapnpError(e)) => Err(e),
        }
    }

    async fn get_table_exists(
        self: Rc<Self>,
        params: tabledb_capnp::database::GetTableExistsParams,
        mut results: tabledb_capnp::database::GetTableExistsResults,
    ) -> ::capnp::Result<()> {
        let params = params.get()?;
        let mut status_builder = results.get().init_result().init_status();

        let TableParam {
            table_name,
            datasource_name,
        } = TableParam::from_reader(params.get_table()?)?;

        let result = self
            .state
            .borrow()
            .read_datasource_tables(&datasource_name, |ds_tables| {
                // Write result
                let table_exists = ds_tables.contains_key(&table_name);
                status_builder
                    .reborrow()
                    .init_status_ok()
                    .set_value(table_exists);
                Ok(())
            });

        match result {
            Ok(_) => Ok(()),
            Err(MockDatabaseError::OperationError(e)) => {
                report_error(&mut status_builder, e);
                Ok(())
            }
            Err(MockDatabaseError::CapnpError(e)) => Err(e),
        }
    }

    async fn get_table_metadata(
        self: Rc<Self>,
        params: tabledb_capnp::database::GetTableMetadataParams,
        mut results: tabledb_capnp::database::GetTableMetadataResults,
    ) -> ::capnp::Result<()> {
        let params = params.get()?;
        let mut status_builder = results.get().init_result().init_status();

        let TableParam {
            table_name,
            datasource_name,
        } = TableParam::from_reader(params.get_table()?)?;

        let result = self
            .state
            .borrow()
            .read_table(&datasource_name, &table_name, |table| {
                // Write metadata to result
                let mut metadata_builder = status_builder.reborrow().init_status_ok();
                metadata_builder.set_row_count(table.row_count);

                let schema_builder = metadata_builder.init_schema();
                let mut columns_builder = schema_builder.init_columns(table.column_count() as u32);
                for (index, column) in table.schema.iter().enumerate() {
                    let mut column_builder = columns_builder.reborrow().get(index as u32);
                    column_builder.set_name(&column.name);

                    let mut type_builder = column_builder.init_type();
                    type_builder.set_domain(&column.type_.domain);
                    type_builder.set_name(&column.type_.name);
                    type_builder.set_size(column.type_.size);
                }

                Ok(())
            });

        match result {
            Ok(_) => Ok(()),
            Err(MockDatabaseError::OperationError(e)) => {
                report_error(&mut status_builder, e);
                Ok(())
            }
            Err(MockDatabaseError::CapnpError(e)) => Err(e),
        }
    }
}
