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

use std::{collections::HashSet, ffi::CStr, path::Path};

use crate::{
    common::container::PointerVector,
    ffi::{
        bindings::{
            SharemindTdbString, SharemindTdbString_delete, SharemindTdbString_new2,
            SharemindTdbType,
        },
        facility::Facility,
    },
    tabledb_capnp,
};
use capnp::{
    capability::{FromTypelessPipeline, Request, Response},
    traits::{Owned, Pipelined},
};
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use futures::AsyncReadExt;

use super::{
    batch::{
        deserialize_column_batch_from_reader, serialize_batches_to_write_request, TdbValueBatch,
        TdbValuePtrVec,
    },
    error::{TdbError, TdbErrorKind},
    types::TdbType,
};

// Helpers

fn validate_column_names(name_ptrs: &[*mut SharemindTdbString]) -> Result<(), TdbError> {
    if name_ptrs.is_empty() {
        return Err(TdbError::invalid_argument("no column names provided"));
    }

    let mut seen_names = HashSet::new();

    for &name_ptr in name_ptrs {
        assert!(!name_ptr.is_null(), "column name pointer must not be null");
        let name = unsafe { (*name_ptr).as_str() };

        if name.is_empty() {
            return Err(TdbError::invalid_argument(
                "invalid column name: column name must be a non-empty string",
            ));
        }

        if !seen_names.insert(name) {
            return Err(TdbError::invalid_argument(format!(
                "invalid column name: column names must be unique (duplicate: {name})",
            )));
        }
    }

    Ok(())
}

fn validate_column_types(
    type_ptrs: &[*mut SharemindTdbType],
    name_ptrs: &[*mut SharemindTdbString],
) -> Result<(), TdbError> {
    assert_eq!(type_ptrs.len(), name_ptrs.len());

    if type_ptrs.is_empty() {
        return Err(TdbError::invalid_argument("no column types provided"));
    }

    for (i, &type_ptr) in type_ptrs.iter().enumerate() {
        let column_type = unsafe { &*type_ptr };
        if let Err(e) = column_type.is_valid() {
            return Err(TdbError::invalid_argument(format!(
                "column \"{}\" has invalid type: {e}",
                (unsafe { *name_ptrs[i] }).as_str()
            )));
        }
    }
    Ok(())
}

// RPC interface

pub struct TabledbRpc {
    pub client: tabledb_capnp::database::Client,
    runtime: tokio::runtime::Runtime,
    local: tokio::task::LocalSet,
}

impl TabledbRpc {
    pub fn new(socket_addr: &Path) -> Result<Self, TdbError> {
        // Create a custom tokio runtime
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let local = tokio::task::LocalSet::new();

        // Establish a socket connection to the server
        let stream = runtime
            .block_on(async { tokio::net::UnixStream::connect(socket_addr).await })
            .map_err(|e| {
                TdbError::io_error(format!(
                    "Could not connect to socket '{}': {e}",
                    socket_addr.to_string_lossy()
                ))
            })?;

        // Create the Cap'n Proto client
        let (client, rpc_system) = {
            let (reader, writer) =
                tokio_util::compat::TokioAsyncReadCompatExt::compat(stream).split();
            let rpc_network = twoparty::VatNetwork::new(
                futures::io::BufReader::new(reader),
                futures::io::BufWriter::new(writer),
                rpc_twoparty_capnp::Side::Client,
                Default::default(),
            );

            let mut rpc_system = RpcSystem::new(Box::new(rpc_network), None);
            let client: tabledb_capnp::database::Client =
                rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);

            (client, rpc_system)
        };

        // Spawn the RPC system in the runtime
        local.spawn_local(rpc_system);

        Ok(Self {
            client,
            runtime,
            local,
        })
    }

    pub fn with_client(client: tabledb_capnp::database::Client) -> Result<Self, TdbError> {
        // Create a dummy tokio runtime
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let local = tokio::task::LocalSet::new();

        Ok(Self {
            client,
            runtime,
            local,
        })
    }

    pub fn call<Params, Results>(
        &self,
        request: Request<Params, Results>,
    ) -> Result<Response<Results>, TdbError>
    where
        Params: Pipelined + Owned,
        Results: Pipelined + Owned + Unpin + 'static,
        <Results as Pipelined>::Pipeline: FromTypelessPipeline,
    {
        self.runtime
            .block_on(self.local.run_until(async { request.send().promise.await }))
            .map_err(|e| TdbError::io_error(format!("RPC call error: {e}")))
    }

    pub fn get_table_names(
        &self,
        datasource_name: &str,
    ) -> Result<PointerVector<SharemindTdbString>, TdbError> {
        // Build the RPC request
        let mut request = self.client.get_table_names_request();
        request.get().set_datasource(datasource_name);

        // Execute the RPC call
        let response = self.call(request)?;

        // Parse the RPC call results
        let table_names_reader = get_database_result(&response)?;

        let table_names_count = table_names_reader.len();
        let mut table_names = PointerVector::<SharemindTdbString>::new(
            table_names_count as usize,
            Some(SharemindTdbString_delete),
        );

        for i in 0..table_names_count {
            let name_reader = table_names_reader.get(i).map_err(|e| {
                TdbError::io_error(format!("Error parsing table name at index {i}: {e}"))
            })?;

            let name_bytes = name_reader.as_bytes();
            let name_ptr = unsafe {
                SharemindTdbString_new2(name_bytes.as_ptr() as *const i8, name_bytes.len())
            };

            if name_ptr.is_null() {
                return Err(TdbError::general_error(
                    "Could not allocate memory for table name",
                ));
            }

            table_names.push(name_ptr);
        }

        Ok(table_names)
    }

    pub fn get_table_exists(
        &self,
        table_name: &str,
        datasource_name: &str,
    ) -> Result<bool, TdbError> {
        // Build the RPC request
        let mut request = self.client.get_table_exists_request();

        {
            let mut table = request.get().init_table();
            table.set_name(table_name);
            table.set_datasource(datasource_name);
        }

        // Execute the RPC call
        let response = self.call(request)?;

        // Parse the RPC call results
        let table_exists_reader = get_database_result(&response)?;

        Ok(table_exists_reader.get_value())
    }

    pub fn get_table_metadata(
        &self,
        table_name: &str,
        datasource_name: &str,
    ) -> Result<TableMetadata, TdbError> {
        // Build the RPC request
        let mut request = self.client.get_table_metadata_request();

        {
            let mut table = request.get().init_table();
            table.set_name(table_name);
            table.set_datasource(datasource_name);
        }

        // Execute the RPC call
        let response = self.call(request)?;

        // Parse the RPC call results
        let metadata_reader = get_database_result(&response)?;
        let metadata = TableMetadata::from_capnp_reader(metadata_reader)
            .map_err(|e| TdbError::io_error(format!("error parsing metadata response: {e}")))?;

        Ok(metadata)
    }

    pub fn create_table(
        &self,
        table_name: &str,
        datasource_name: &str,
        column_names: &[*mut SharemindTdbString],
        column_types: &[*mut SharemindTdbType],
    ) -> Result<(), TdbError> {
        // Do some simple checks on the input parameters
        if column_names.len() != column_types.len() {
            return Err(TdbError::invalid_argument(
                "different number of column names and column types provided",
            ));
        }

        validate_column_names(column_names)?;
        validate_column_types(column_types, column_names)?;

        // Build the RPC request
        let mut request = self.client.create_table_request();

        {
            let mut table = request.get().init_table();
            table.set_name(table_name);
            table.set_datasource(datasource_name);
        }

        {
            let schema = request.get().init_schema();
            let column_count = column_names.len() as u32;
            let mut columns = schema.init_columns(column_count);

            for i in 0..column_count {
                let name_struct = unsafe { &*column_names[i as usize] };
                let mut column = columns.reborrow().get(i);
                column.set_name(name_struct.as_str());

                let type_struct = unsafe { &*column_types[i as usize] };
                let mut column_type = column.init_type();
                column_type.set_domain(type_struct.domain_str());
                column_type.set_name(type_struct.name_str());
                column_type.set_size(type_struct.size);
            }
        }

        // Execute the RPC call
        let response = self.call(request)?;

        // Parse the RPC response
        let _ = get_database_result(&response)?;

        Ok(())
    }

    pub fn delete_table(&self, table_name: &str, datasource_name: &str) -> Result<(), TdbError> {
        // Build the RPC request
        let mut request = self.client.delete_table_request();

        {
            let mut table = request.get().init_table();
            table.set_name(table_name);
            table.set_datasource(datasource_name);
        }

        // Execute the RPC call
        let response = self.call(request)?;

        // Parse the RPC response
        let _ = get_database_result(&response)?;

        Ok(())
    }

    // Serializes the inputs as a list of ColumnData structures and executes the Database.write request.
    // Additionally retrieves table metadata (used for serialization) via the
    // Database.getTableMetadata request.
    // The database backend stores the ColumnData structures as blobs without
    // deserializing nor concatenating them with previous blobs in the
    // corresponding columns.
    pub fn write(
        &self,
        table_name: &str,
        datasource_name: &str,
        value_batches: &[TdbValueBatch],
    ) -> Result<(), TdbError> {
        // Retrieve table metadata
        let table_metadata = self.get_table_metadata(table_name, datasource_name)?;

        // Construct the RPC request
        let mut request = self.client.write_request();

        {
            let mut table = request.get().init_table();
            table.set_name(table_name);
            table.set_datasource(datasource_name);
        }

        serialize_batches_to_write_request(&table_metadata, value_batches, request.get())?;

        // Execute the RPC call
        let response = self.call(request)?;

        // Parse the RPC response
        let _ = get_database_result(&response)?;

        Ok(())
    }

    // Executes the Database.read request to retrieve a list of blobs stored for
    //  the same column by the write method calls.
    // Each blob represents the ColumnData structure and is deserialized as such.
    pub fn read(
        &self,
        table_name: &str,
        datasource_name: &str,
        col_idx: u64,
        table_metadata: &TableMetadata,
    ) -> Result<TdbValuePtrVec, TdbError> {
        // Construct the RPC request
        let mut request = self.client.read_request();

        {
            let mut table = request.get().init_table();
            table.set_name(table_name);
            table.set_datasource(datasource_name);
        }

        request.get().set_column_idx(col_idx);

        // Execute the RPC call
        let response = self.call(request)?;

        // Parse the RPC response
        let column_data_reader = get_database_result(&response)?;
        let column_metadata = table_metadata
            .get_column(col_idx as usize)
            .ok_or_else(|| TdbError::general_error("column metadata not found"))?;

        deserialize_column_batch_from_reader(column_data_reader, column_metadata)
    }
}

unsafe impl Send for TabledbRpc {}
unsafe impl Sync for TabledbRpc {}

impl Facility<TabledbRpc> for TabledbRpc {
    /// Returns pointer to self, thus self has to have a stable memory address.
    fn inner(&mut self) -> *mut TabledbRpc {
        self as *mut _
    }

    fn name(&self) -> &CStr {
        c"TabledbRpc"
    }
}

pub struct ColumnMetadata {
    pub name: String,
    pub type_: TdbType,
}

pub struct TableMetadata {
    pub schema: Vec<ColumnMetadata>,
    pub row_count: u64,
}

impl TableMetadata {
    fn from_capnp_reader(
        metadata_reader: tabledb_capnp::table_metadata::Reader<'_>,
    ) -> ::capnp::Result<Self> {
        let schema_reader = metadata_reader.get_schema()?;
        let columns_reader = schema_reader.get_columns()?;

        let mut schema: Vec<ColumnMetadata> = Vec::with_capacity(columns_reader.len() as usize);
        for column in columns_reader {
            schema.push(ColumnMetadata {
                name: column.get_name()?.to_string()?,
                type_: TdbType {
                    domain: column.get_type()?.get_domain()?.to_string()?,
                    name: column.get_type()?.get_name()?.to_string()?,
                    size: column.get_type()?.get_size(),
                },
            });
        }

        Ok(Self {
            schema,
            row_count: metadata_reader.get_row_count(),
        })
    }

    pub fn get_schema(&self) -> &Vec<ColumnMetadata> {
        &self.schema
    }

    pub fn get_column(&self, idx: usize) -> Option<&ColumnMetadata> {
        self.schema.get(idx)
    }

    pub fn get_column_count(&self) -> usize {
        self.schema.len()
    }

    pub fn get_row_count(&self) -> usize {
        const_assert_eq!(std::mem::size_of::<u64>(), std::mem::size_of::<usize>());
        self.row_count as usize
    }
}

// Purpose: Enable unified access to `get_result` accessors in capnp responses.
// Reason:  We want to implement a single result retrieval and parsing function.
//          However, the `get_result` accessor for each capnp response is
//          defined in a separate module and returns a different type.
pub trait GetResult<'a> {
    type ReaderType;
    fn get_result<'b: 'a>(&'b self) -> ::capnp::Result<Self::ReaderType>;
}

macro_rules! define_get_result {
    ($response_type:ty, $result_type:ty) => {
        impl<'a> GetResult<'a> for Response<$response_type> {
            type ReaderType = tabledb_capnp::database::result::Reader<'a, $result_type>;
            fn get_result<'b: 'a>(&'b self) -> ::capnp::Result<Self::ReaderType> {
                self.get()?.get_result()
            }
        }
    };
}

define_get_result!(
    tabledb_capnp::database::write_results::Owned,
    ::capnp::any_pointer::Owned
);
define_get_result!(
    tabledb_capnp::database::read_results::Owned,
    ::capnp::struct_list::Owned<crate::tabledb_capnp::column_data::Owned>
);
define_get_result!(
    tabledb_capnp::database::create_table_results::Owned,
    ::capnp::any_pointer::Owned
);
define_get_result!(
    tabledb_capnp::database::delete_table_results::Owned,
    ::capnp::any_pointer::Owned
);
define_get_result!(
    tabledb_capnp::database::get_table_names_results::Owned,
    ::capnp::text_list::Owned
);
define_get_result!(
    tabledb_capnp::database::get_table_exists_results::Owned,
    tabledb_capnp::bool_value::Owned
);
define_get_result!(
    tabledb_capnp::database::get_table_metadata_results::Owned,
    tabledb_capnp::table_metadata::Owned
);

// Retrieves a database result reader and the error code from the response.
fn get_database_result<'a, R, T>(response: &'a R) -> Result<<T as Owned>::Reader<'a>, TdbError>
where
    R: GetResult<'a, ReaderType = tabledb_capnp::database::result::Reader<'a, T>>,
    T: Owned,
{
    // Until try_blocks is stabilizied, use a closure to catch all capnp errors for conversion.
    // https://doc.rust-lang.org/stable/unstable-book/language-features/try-blocks.html
    (|| {
        let response_result_reader = response.get_result()?;
        match response_result_reader.get_status().which()? {
            tabledb_capnp::database::result::status::StatusOk(tdb_result_reader) => {
                Ok(Ok(tdb_result_reader?))
            }
            tabledb_capnp::database::result::status::StatusErr(_) => {
                // TODO: Should we encode TdbError enum in capnp response to return appropriate error code here?
                Ok(Err(TdbError::new(
                    TdbErrorKind::UnknownError,
                    "error returned by database",
                )))
            }
        }
    })()
    .map_err(|e: ::capnp::Error| {
        TdbError::io_error(format!(
            "error parsing the database result from response: {e}"
        ))
    })?
}
