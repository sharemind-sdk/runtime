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
    ffi::{CStr, CString},
    mem, ptr,
};

use batch::TdbValueBatch;
use error::{set_tabledb_module_error_code, TdbError};
use rpc::TabledbRpc;
use vmap::{action_with_new_vector_map, get_vector_map};

use crate::{
    common::container::PointerVector,
    ffi::{
        bindings::{
            SharemindModuleApi0x1Reference, SharemindModuleApi0x1SyscallContext,
            SharemindTdbError__SHAREMIND_TDB_OK, SharemindTdbIndex, SharemindTdbString,
            SharemindTdbString_delete, SharemindTdbString_new2, SharemindTdbType,
            SharemindTdbType_delete, SharemindTdbType_new2, SharemindTdbValue,
            SharemindTdbVectorMapError__TDB_VECTOR_MAP_OK,
        },
        vm::syscall::{
            error::{SyscallError, SyscallErrorKind},
            interface::{validate_syscall, SyscallRule},
        },
    },
    ref_to_cstr_or_err, ref_to_str_or_err, sharemind_module_api_0x1_syscall, syserr,
};

mod batch;
mod error;
pub mod rpc;
pub mod types;
mod vmap;

/*
  Overview of key changes in tabledb database integration.

  Sharemind MPC (original):
    * mod_tabledb provides VM syscalls for:
      a) Database API
        - High-level database operations
        - Table database operations
      b) Vector map API (for parameters and results)
    * The Database API syscalls are actually stubs that check for permissions and
      forward the call to a tabledb submodule associated with the given data source.
    * A tabledb submodule implements the Database API syscalls for a specific
      underlying database engine.
    * The mod_tabledb loads its submodules according to tabledb configuration
      using its own instance of the Sharemind Loadable Module system (similarly
      to how Application Server does).

  Sharemind MPC (Carbyne Stack):
    * Amphora is the database for secret data.
    * Runtime accesses Amphora via Ephemeral.
      - Runtime provides generic tabledb Database API syscalls that relay the
        calls from a SecreC program runnig in VM to Ephemeral/Amphora.
    * Amphora decides how to store the data.
      - Tabledb submodules like hdf5 no longer have any meaning in the runtime.
    * ACL checks move to Amphora or even to further authn/authz layer (awaiting from Bosch)
      - Runtime only sees whether some Ephemeral/Amphora call succeeded.
*/

/////////////////////////
// HELPERS
/////////////////////////

/// Handle the TableDB result error and extract the result value if no error occurred.
///
/// Extract the error code from the result.
/// Set the error code in tabledb module.
/// Set error code in the optional VM reference, if one is provided.
/// Propagate errors or the value to the caller.
///
/// Returns:
///   Some(T), if no tabledb error occurred.
///   None, if tabledb error occurred and error code was stored in the
///         provided VM reference. Allows the caller to diverge from the normal
///         execution path on tabledb errors, e.g. stop execution early.
///   Err(SyscallError), if
///         a) this function encountered a critical error or,
///         b) tabledb error occurred, but no VM reference was provided.
fn handle_tabledb_result<T>(
    ctx: &SharemindModuleApi0x1SyscallContext,
    ds_name: &CStr,
    ecode_ref: Option<&SharemindModuleApi0x1Reference>,
    result: Result<T, TdbError>,
) -> Result<Option<T>, SyscallError> {
    // Get the FFI-compatible error code from the tabledb result
    let error_code = match &result {
        Ok(_) => SharemindTdbError__SHAREMIND_TDB_OK,
        Err(e) => e.kind.clone().into(),
    };

    // Set error code in the module and propagate critical errors
    set_tabledb_module_error_code(ctx, ds_name, error_code)?;

    // Store error code in the optional VM reference, if provided
    if let Some(&r) = ecode_ref {
        unsafe {
            *(r.pData as *mut i64) = error_code as i64;
        }
        if let Err(e) = &result {
            tracing::warn!("Handled tabledb error: {e}");
            return Ok(None); // Error was handled, return early
        }
    }

    match result {
        Ok(value) => Ok(Some(value)), // No errors occurred, return the value
        Err(e) => Err(e.into()),      // No VM reference provided, propagate tabledb error
    }
}

fn validate_table_id_str(s: &str) -> Result<(), &'static str> {
    if s.is_empty() {
        return Err("string must be a non-empty string");
    }

    for c in s.chars() {
        if !c.is_ascii() {
            return Err("string must not contain non-ASCII characters");
        }

        match c {
            '\0' => {
                return Err("string must not contain NULL characters");
            }
            '/' => {
                return Err("string must not contain '/' characters");
            }
            _ => (),
        }
    }

    Ok(())
}

fn validate_table_name(name: &str) -> Result<(), SyscallError> {
    validate_table_id_str(name)
        .map_err(|e| syserr!(SyscallErrorKind::InvalidCall, "invalid table name", e))
}

fn validate_datasource_name(name: &str) -> Result<(), SyscallError> {
    validate_table_id_str(name)
        .map_err(|e| syserr!(SyscallErrorKind::InvalidCall, "invalid datasource name", e))
}

/// Retrieves the TabledbRpc object installed in the runtime process facilities.
///
/// # Safety
///
/// NB! The `moduleHandle` in syscall context points to a C++ structure created
/// during tabledb module initialization. Therefore it cannot be used in Rust
/// code to access module facilities. We fallback to the less efficient
/// process facilities mechanism.
fn get_tabledb_rpc(ctx: &SharemindModuleApi0x1SyscallContext) -> Result<&TabledbRpc, SyscallError> {
    Ok(ctx.get_process_facility::<TabledbRpc>(c"TabledbRpc")?)
}

/////////////////////////////////////////////
// SYSCALLS: HIGH LEVEL DATABASE OPERATIONS
/////////////////////////////////////////////

sharemind_module_api_0x1_syscall! {
/// Open connection to the data source.
///
/// NB! This operation is not supported in our Carbyne Stack integration.
/// This syscall is provided for compatibility with existing code.
/// It is effectively a no-op.
///
/// Stack arguments:
///  N\A
///
/// Reference parameters:
///  N\A
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the data source (string)
///
/// No return value
tdb_open, |args, refs, crefs, return_value, _c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 0, num_refs: 0, num_crefs: 1, need_return_value: false}
    ])?;

    // no-op

    Ok(())
}}

sharemind_module_api_0x1_syscall! {
/// Close connection to the data source.
///
/// NB! This operation is not supported in our Carbyne Stack integration.
/// This syscall is provided for compatibility with existing code.
/// It is effectively a no-op.
///
/// Stack arguments:
///  N\A
///
/// Reference parameters:
///  N\A
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the data source (string)
///
/// No return value
tdb_close, |args, refs, crefs, return_value, _c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 0, num_refs: 0, num_crefs: 1, need_return_value: false}
    ])?;

    // no-op

    Ok(())
}}

sharemind_module_api_0x1_syscall! {
/// Retrieve a list of names of all database tables available in the datasource.
///
/// Stack arguments:
///  N/A
///
/// Reference parameters:
///  Optional `refs[0]`: returned error code (int64)
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the data source to check for tables (string)
///
/// Return value: id of the vector map containing the retrieved table names in
///               the "names" string vector (uint64).
tdb_table_names, |args, refs, crefs, return_value, c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 0, num_refs: 0, num_crefs: 1, need_return_value: true},
        SyscallRule {num_args: 0, num_refs: 1, num_crefs: 1, need_return_value: true},
    ])?;

    if !refs.is_empty() && refs[0].size != mem::size_of::<i64>() {
        return Err(syserr!(SyscallErrorKind::InvalidCall, "invalid refs[0]"));
    }

    let datasource_name_cstr = ref_to_cstr_or_err!(crefs[0]);
    let datasource_name = ref_to_str_or_err!(crefs[0]);

    // Get RPC interface
    let tabledb_rpc = get_tabledb_rpc(c)?;

    // Get table names via RPC
    let table_names = {
        let names_result = tabledb_rpc.get_table_names(datasource_name);
        if let Some(names) = handle_tabledb_result(c, datasource_name_cstr, refs.first(), names_result)? {
            names
        } else {
            return Ok(());  // non-OK error code stored in VM reference, return early
        }
    };

    // Store table names in the result vector map
    let vmap_id = action_with_new_vector_map(c, |vmap| {
        vmap.store_objects(c"names", table_names)
    })?;

    (*return_value).uint64[0] = vmap_id;

    Ok(())
}}

/////////////////////////////////////////////
// SYSCALLS: TABLE DATABASE OPERATIONS
/////////////////////////////////////////////

sharemind_module_api_0x1_syscall! {
/// Create a table in the database.
/// All columns have the same type and are named by their index starting with 0.
///
/// Stack arguments:
///  Mandatory `args[0]`: size of the column data type (uint64)
///  Mandatory `args[1]`: number of columns in the table (uint64)
///
/// Reference parameters:
///  Optional `refs[0]`: returned error code (int64)
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the data source (string)
///  Mandatory `crefs[1]`: name of the table (string)
///  Mandatory `crefs[2]`: name of the column protection domain (string)
///  Mandatory `crefs[3]`: name of the column data type (string)
///
/// No return value
tdb_tbl_create, |args, refs, crefs, return_value, c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 2, num_refs: 0, num_crefs: 4, need_return_value: false},
        SyscallRule {num_args: 2, num_refs: 1, num_crefs: 4, need_return_value: false},
    ])?;

    if !refs.is_empty() && refs[0].size != mem::size_of::<i64>() {
        return Err(syserr!(SyscallErrorKind::InvalidCall, "invalid refs[0]"));
    }

    let datasource_name_cstr = ref_to_cstr_or_err!(crefs[0]);
    let datasource_name = ref_to_str_or_err!(crefs[0]);
    let table_name = ref_to_str_or_err!(crefs[1]);
    let type_domain = ref_to_cstr_or_err!(crefs[2]);
    let type_name = ref_to_cstr_or_err!(crefs[3]);

    validate_datasource_name(datasource_name)?;
    validate_table_name(table_name)?;

    let type_size: u64 = args[0].uint64[0];
    let ncols: u64 = args[1].uint64[0];

    if ncols == 0 {
        return Err(syserr!(SyscallErrorKind::InvalidCall, "invalid args[1]: ncols cannot be zero"));
    }

    // Construct table schema parameters

    // Create numbered column names
    let mut name_strings: Vec<CString> = Vec::with_capacity(ncols as usize);
    let mut name_structs: Vec<SharemindTdbString> = Vec::with_capacity(ncols as usize);
    let names = (0..ncols)
        .map(|i| {
            let name = CString::new(i.to_string())
                .map_err(|_| syserr!(SyscallErrorKind::GeneralError, "NulError while creating a C-string"))?;
            name_strings.push(name);
            name_structs.push(SharemindTdbString { str_: name_strings[i as usize].as_ptr() as *mut _ });
            Ok(&mut name_structs[i as usize] as *mut _)
        })
        .collect::<Result<Vec<_>, SyscallError>>()?;

    // Create column types
    let column_type = SharemindTdbType {
        domain: type_domain.as_ptr() as *mut _,
        name: type_name.as_ptr() as *mut _,
        size: type_size,
    };

    let type_ptr = &column_type as *const _ as *mut _;
    let types = vec![type_ptr; ncols as usize];

    // Get RPC interface
    let tabledb_rpc = get_tabledb_rpc(c)?;

    // Create table via RPC
    let create_result = tabledb_rpc.create_table(table_name, datasource_name, &names, &types);
    handle_tabledb_result(c, datasource_name_cstr, refs.first(), create_result)?;

    Ok(())
}}

sharemind_module_api_0x1_syscall! {
/// Create a table in the database from.
/// Parameters for creating the table are retrieved from a vector map.
///
/// Stack arguments:
///  Mandatory `args[0]`: id of the vector map containing the parameters for creating
///                      the table (uint64). The vectors "types" and "names"
///                      should contain the types and names of the columns.
///
/// Reference parameters:
///  Optional `refs[0]`: returned error code (int64)
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the data source (string)
///  Mandatory `crefs[1]`: name of the table (string)
///
/// No return value
tdb_tbl_create2, |args, refs, crefs, return_value, c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 1, num_refs: 0, num_crefs: 2, need_return_value: false},
        SyscallRule {num_args: 1, num_refs: 1, num_crefs: 2, need_return_value: false},
    ])?;

    if !refs.is_empty() && refs[0].size != mem::size_of::<i64>() {
        return Err(syserr!(SyscallErrorKind::InvalidCall, "invalid refs[0]"));
    }

    let datasource_name_cstr = ref_to_cstr_or_err!(crefs[0]);
    let datasource_name = ref_to_str_or_err!(crefs[0]);
    let table_name = ref_to_str_or_err!(crefs[1]);

    let vmap_id: u64 = args[0].uint64[0];

    // Get the parameter vmap
    let pmap = get_vector_map(c, vmap_id)?;

    // Parse the "names" parameter
    let names = pmap.get_objects::<SharemindTdbString>(c"names")?;

    // Parse the "types" parameter
    let types = pmap.get_objects::<SharemindTdbType>(c"types")?;

    // Get RPC interface
    let tabledb_rpc = get_tabledb_rpc(c)?;

    // Create table via RPC
    let create_result = tabledb_rpc.create_table(table_name, datasource_name, names, types);
    handle_tabledb_result(c, datasource_name_cstr, refs.first(), create_result)?;

    Ok(())
}}

sharemind_module_api_0x1_syscall! {
/// Delete a table from the database.
///
/// Stack arguments:
///  N/A
///
/// Reference parameters:
///  Optional `refs[0]`: returned error code (int64)
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the data source (string)
///  Mandatory `crefs[1]`: name of the table (string)
///
/// No return value
tdb_tbl_delete, |args, refs, crefs, return_value, c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 0, num_refs: 0, num_crefs: 2, need_return_value: false},
        SyscallRule {num_args: 0, num_refs: 1, num_crefs: 2, need_return_value: false},
    ])?;

    if !refs.is_empty() && refs[0].size != mem::size_of::<i64>() {
        return Err(syserr!(SyscallErrorKind::InvalidCall, "invalid refs[0]"));
    }

    let datasource_name_cstr = ref_to_cstr_or_err!(crefs[0]);
    let datasource_name = ref_to_str_or_err!(crefs[0]);
    let table_name = ref_to_str_or_err!(crefs[1]);

    validate_datasource_name(datasource_name)?;
    validate_table_name(table_name)?;

    // Get RPC interface
    let tabledb_rpc = get_tabledb_rpc(c)?;

    // Delete table via RPC
    let delete_result = tabledb_rpc.delete_table(table_name, datasource_name);
    handle_tabledb_result(c, datasource_name_cstr, refs.first(), delete_result)?;

    Ok(())
}}

sharemind_module_api_0x1_syscall! {
/// Check if a database table exists.
///
/// Stack arguments:
///  N/A
///
/// Reference parameters:
///  Optional `refs[0]`: returned error code (int64)
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the data source containing the table (string)
///  Mandatory `crefs[1]`: name of the table (string)
///
/// Return value: indication of table existence (uint64).
///               0 = table does not exist, 1 = table exists.
tdb_tbl_exists, |args, refs, crefs, return_value, c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 0, num_refs: 0, num_crefs: 2, need_return_value: true},
        SyscallRule {num_args: 0, num_refs: 1, num_crefs: 2, need_return_value: true},
    ])?;

    if !refs.is_empty() && refs[0].size != mem::size_of::<i64>() {
        return Err(syserr!(SyscallErrorKind::InvalidCall, "invalid refs[0]"));
    }

    let datasource_name_cstr = ref_to_cstr_or_err!(crefs[0]);
    let datasource_name = ref_to_str_or_err!(crefs[0]);
    let table_name = ref_to_str_or_err!(crefs[1]);

    validate_datasource_name(datasource_name)?;
    validate_table_name(table_name)?;

    // Get RPC interface
    let tabledb_rpc = get_tabledb_rpc(c)?;

    // Check the existence of a table via RPC
    let exists_result = tabledb_rpc.get_table_exists(table_name, datasource_name);

    let table_exists = if let Some(table_exists) = handle_tabledb_result(c, datasource_name_cstr, refs.first(), exists_result)? {
        table_exists
    } else {
        return Ok(()); // non-OK error code stored in VM reference, return early
    };

    (*return_value).uint64[0] = table_exists as u64;

    Ok(())
}}

sharemind_module_api_0x1_syscall! {
/// Retrieve the number of columns in the database table.
///
/// Stack arguments:
///  N/A
///
/// Reference parameters:
///  Optional `refs[0]`: returned error code (int64)
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the data source containing the table (string)
///  Mandatory `crefs[1]`: name of the table (string)
///
/// Return value: number of columns in the table (uint64).
tdb_tbl_col_count, |args, refs, crefs, return_value, c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 0, num_refs: 0, num_crefs: 2, need_return_value: true},
        SyscallRule {num_args: 0, num_refs: 1, num_crefs: 2, need_return_value: true},
    ])?;

    if !refs.is_empty() && refs[0].size != mem::size_of::<i64>() {
        return Err(syserr!(SyscallErrorKind::InvalidCall, "invalid refs[0]"));
    }

    let datasource_name_cstr = ref_to_cstr_or_err!(crefs[0]);
    let datasource_name = ref_to_str_or_err!(crefs[0]);
    let table_name = ref_to_str_or_err!(crefs[1]);

    validate_datasource_name(datasource_name)?;
    validate_table_name(table_name)?;

    // Get RPC interface
    let tabledb_rpc = get_tabledb_rpc(c)?;

    // Retrieve table metadata via RPC
    let table_metadata = {
        let metadata_result = tabledb_rpc.get_table_metadata(table_name, datasource_name);
        if let Some(metadata) = handle_tabledb_result(c, datasource_name_cstr, refs.first(), metadata_result)? {
            metadata
        } else {
            return Ok(());  // non-OK error code stored in VM reference, return early
        }
    };

    (*return_value).uint64[0] = table_metadata.get_column_count() as u64;

    Ok(())
}}

sharemind_module_api_0x1_syscall! {
/// Retrieve a list of names of all the columns in the database table.
///
/// Stack arguments:
///  N/A
///
/// Reference parameters:
///  Optional `refs[0]`: returned error code (int64)
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the data source containing the table (string)
///  Mandatory `crefs[1]`: name of the table (string)
///
/// Return value: id of the vector map containing the retrieved column names in
///               the "names" string vector (uint64).
tdb_tbl_col_names, |args, refs, crefs, return_value, c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 0, num_refs: 0, num_crefs: 2, need_return_value: true},
        SyscallRule {num_args: 0, num_refs: 1, num_crefs: 2, need_return_value: true},
    ])?;

    if !refs.is_empty() && refs[0].size != mem::size_of::<i64>() {
        return Err(syserr!(SyscallErrorKind::InvalidCall, "invalid refs[0]"));
    }

    let datasource_name_cstr = ref_to_cstr_or_err!(crefs[0]);
    let datasource_name = ref_to_str_or_err!(crefs[0]);
    let table_name = ref_to_str_or_err!(crefs[1]);

    validate_datasource_name(datasource_name)?;
    validate_table_name(table_name)?;

    // Get RPC interface
    let tabledb_rpc = get_tabledb_rpc(c)?;

    // Retrieve table metadata via RPC
    let table_metadata = {
        let metadata_result = tabledb_rpc.get_table_metadata(table_name, datasource_name);
        if let Some(metadata) = handle_tabledb_result(c, datasource_name_cstr, refs.first(), metadata_result)? {
            metadata
        } else {
            return Ok(());  // non-OK error code stored in VM reference, return early
        }
    };

    // Create an owned list of FFI strings holding the column names
    let mut column_names = PointerVector::<SharemindTdbString>::new(
        table_metadata.get_column_count(),
        Some(SharemindTdbString_delete),
    );

    for column in table_metadata.get_schema() {
        let name_ptr = unsafe {
            SharemindTdbString_new2(column.name.as_ptr() as *const i8, column.name.len())
        };
        if name_ptr.is_null() {
            return Err(syserr!(SyscallErrorKind::OutOfMemory, "could not allocate the column name string"));
        }
        column_names.push(name_ptr)
    }

    // Store column names in the result vector map
    let vmap_id = action_with_new_vector_map(c, |vmap| {
        vmap.store_objects(c"names", column_names)
    })?;

    (*return_value).uint64[0] = vmap_id;

    Ok(())
}}

sharemind_module_api_0x1_syscall! {
/// Retrieve a list of types of all the columns in the database table.
///
/// Stack arguments:
///  N/A
///
/// Reference parameters:
///  Optional `refs[0]`: returned error code (int64)
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the data source containing the table (string)
///  Mandatory `crefs[1]`: name of the table (string)
///
/// Return value: id of the vector map containing the retrieved column types in
///               the "types" type vector (uint64).
tdb_tbl_col_types, |args, refs, crefs, return_value, c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 0, num_refs: 0, num_crefs: 2, need_return_value: true},
        SyscallRule {num_args: 0, num_refs: 1, num_crefs: 2, need_return_value: true},
    ])?;


    if !refs.is_empty() && refs[0].size != mem::size_of::<i64>() {
        return Err(syserr!(SyscallErrorKind::InvalidCall, "invalid refs[0]"));
    }

    let datasource_name_cstr = ref_to_cstr_or_err!(crefs[0]);
    let datasource_name = ref_to_str_or_err!(crefs[0]);
    let table_name = ref_to_str_or_err!(crefs[1]);

    validate_datasource_name(datasource_name)?;
    validate_table_name(table_name)?;

    // Get RPC interface
    let tabledb_rpc = get_tabledb_rpc(c)?;

    // Retrieve table metadata via RPC
    let table_metadata = {
        let metadata_result = tabledb_rpc.get_table_metadata(table_name, datasource_name);
        if let Some(metadata) = handle_tabledb_result(c, datasource_name_cstr, refs.first(), metadata_result)? {
            metadata
        } else {
            return Ok(());  // non-OK error code stored in VM reference, return early
        }
    };

    // Create an owned list of FFI strings holding the column names
    let mut column_types = PointerVector::<SharemindTdbType>::new(
        table_metadata.get_column_count(),
        Some(SharemindTdbType_delete),
    );

    for column in table_metadata.get_schema() {
        let type_ptr = unsafe {
            SharemindTdbType_new2(
                column.type_.domain.as_ptr() as *const i8,
                column.type_.domain.len(),
                column.type_.name.as_ptr() as *const i8,
                column.type_.name.len(),
                column.type_.size
        )};
        if type_ptr.is_null() {
            return Err(syserr!(SyscallErrorKind::OutOfMemory, "could not allocate the column type object"));
        }
        column_types.push(type_ptr)
    }

    // Store column types in the result vector map
    let vmap_id = action_with_new_vector_map(c, |vmap| {
        vmap.store_objects(c"types", column_types)
    })?;

    (*return_value).uint64[0] = vmap_id;

    Ok(())
}}

sharemind_module_api_0x1_syscall! {
/// Retrieve the number of rows in the database table.
///
/// Stack arguments:
///  N/A
///
/// Reference parameters:
///  Optional `refs[0]`: returned error code (int64)
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the data source containing the table (string)
///  Mandatory `crefs[1]`: name of the table (string)
///
/// Return value: number of rows in the table (uint64).
tdb_tbl_row_count, |args, refs, crefs, return_value, c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 0, num_refs: 0, num_crefs: 2, need_return_value: true},
        SyscallRule {num_args: 0, num_refs: 1, num_crefs: 2, need_return_value: true},
    ])?;

    if !refs.is_empty() && refs[0].size != mem::size_of::<i64>() {
        return Err(syserr!(SyscallErrorKind::InvalidCall, "invalid refs[0]"));
    }

    let datasource_name_cstr = ref_to_cstr_or_err!(crefs[0]);
    let datasource_name = ref_to_str_or_err!(crefs[0]);
    let table_name = ref_to_str_or_err!(crefs[1]);

    validate_datasource_name(datasource_name)?;
    validate_table_name(table_name)?;

    // Get RPC interface
    let tabledb_rpc = get_tabledb_rpc(c)?;

    // Retrieve table metadata via RPC
    let table_metadata = {
        let metadata_result = tabledb_rpc.get_table_metadata(table_name, datasource_name);
        if let Some(metadata) = handle_tabledb_result(c, datasource_name_cstr, refs.first(), metadata_result)? {
            metadata
        } else {
            return Ok(());  // non-OK error code stored in VM reference, return early
        }
    };

    (*return_value).uint64[0] = table_metadata.get_row_count() as u64;

    Ok(())
}}

sharemind_module_api_0x1_syscall! {
/// Insert row(s) into a database table from a data buffer.
///
/// This syscall puts the input data buffer and type information into a single
/// SharemindTdbValue object and writes it to the database in a single batch.
///
/// The input buffer is interpreted based on the provided type information.
/// With a fixed-size type, the buffer can contain a scalar value or a vector of
/// scalar values of the same type.
/// With a variable-length type, the buffer represents a single value of variable
/// length (e.g. a string or a vector) that can be inserted into a table with a
/// single column.
///
/// Stack arguments:
///  Mandatory `args[0]`: size of the data type (uint64)
///  Optional  `args[1]`: value_as_column flag (boolean as uint64)
///                      Affects how the contents of `crefs[4]` are processed.
///                      If true, the data values belong to the same column and
///                         the table shall have only one column. Effectively
///                         insert multiple rows into a single-column table.
///                      If false (default), the data values belong to the same
///                         row, and the number of values must match the number
///                         of columns. Effectively insert one row into a table.
///
/// Reference parameters:
///  Optional `refs[0]`: returned error code (int64)
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the data source (string)
///  Mandatory `crefs[1]`: name of the table (string)
///  Mandatory `crefs[2]`: name of the protection domain (string)
///  Mandatory `crefs[3]`: name of the data type (string)
///  Mandatory `crefs[4]`: buffer of data to be inserted (byte array)
///
/// No return value
tdb_insert_row, |args, refs, crefs, return_value, c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 1, num_refs: 0, num_crefs: 5, need_return_value: false},
        SyscallRule {num_args: 1, num_refs: 1, num_crefs: 5, need_return_value: false},
        SyscallRule {num_args: 2, num_refs: 0, num_crefs: 5, need_return_value: false},
        SyscallRule {num_args: 2, num_refs: 1, num_crefs: 5, need_return_value: false}
    ])?;

    if !refs.is_empty() && refs[0].size != mem::size_of::<i64>() {
        return Err(syserr!(SyscallErrorKind::InvalidCall, "invalid refs[0]"));
    }

    let type_size: u64 = args[0].uint64[0];
    let value_as_column: bool = if args.len() == 2 { args[1].uint64[0] != 0} else { false };

    let datasource_name_cstr = ref_to_cstr_or_err!(crefs[0]);
    let datasource_name = ref_to_str_or_err!(crefs[0]);
    let table_name = ref_to_str_or_err!(crefs[1]);
    let domain_name = ref_to_cstr_or_err!(crefs[2]);
    let type_name = ref_to_cstr_or_err!(crefs[3]);

    validate_datasource_name(datasource_name)?;
    validate_table_name(table_name)?;

    const_assert!(mem::size_of::<usize>() == mem::size_of::<u64>());
    let buf_size = crefs[4].size;

    // Construct the type and value from inputs
    let type_ = SharemindTdbType {
        domain: domain_name.as_ptr() as *mut _,
        name: type_name.as_ptr() as *mut _,
        size: type_size
    };

    let value = SharemindTdbValue {
        type_: &type_ as *const _ as *mut _,
        buffer: crefs[4].pData as *mut _,
        size: buf_size as u64
    };

    // Construct the batch
    let value_ptr = &value as *const _ as *mut SharemindTdbValue;   // get raw pointer
    let value_batch = TdbValueBatch::from_ptr_slice(&[value_ptr], value_as_column, false)?;
    let value_batches: Vec<TdbValueBatch> = vec![value_batch];

    // Get RPC interface
    let tabledb_rpc = get_tabledb_rpc(c)?;

    // Write batches to the database via RPC
    let write_result = tabledb_rpc.write(table_name, datasource_name, &value_batches);
    handle_tabledb_result(c, datasource_name_cstr, refs.first(), write_result)?;

    Ok(())
}}

sharemind_module_api_0x1_syscall! {
/// Insert rows into a database table from a vector map.
///
/// The input vector map is organized into batches. Each batch in a vector map
/// represents one rectangular table segment (K columns x L rows) to be inserted
/// (appended) into a table with K columns. In other words the batch is expected
/// to cover all the table columns, but may represent one or more rows. The cells
/// in each column can hold either fixed-size or variable-length data based on
/// the table column type.
///
/// A table segment is encoded in a batch using two vectors:
///   1. The "values" vector of SharemindTdbValue objects. Each object
///      contains a raw data buffer and data type information, that encode
///      fixed-size or variable-length data.
///   2. Optionally, the "valueAsColumn" vector with one SharemindTdbIndex
///      object, that encodes the Value-As-Column parameter. The VAC parameter
///      specifies whether all the SharemindTdbValue objects in the batch
///      represent column values (true) or row values (false). The default
///      value for the VAC parameter is false.
///
/// This syscall first aggregates the batches information by parsing the "values"
/// and the "valueAsColumn" vectors for each batch in a vector map. It then
/// serializes the batches (segments) into a single column-major request and
/// writes the columns to the database.
///
/// The batches translate into table segments based on
///   a) whether SharemindTdbValue objects contain fixed-size of variable-length data,
///   b) whether SharemindTdbValue objects represent columns or rows
///      (the VAC parameter specifies this for the entire batch/segment)
///
/// The general translation scheme is as follows:
/// a) If a SharemindTdbValue object contains variable-length data (type_size == 0):
///   * The entire data contains a single value of any length (string or vector)
///   * The object represents a single cell in the table segment
///   * The batch represents a segment with one row, and must have K number of
///     SharemindTdbValue objects (one per column).
///   * VAC has no effect, because the data is a single piece.
/// b) If a SharemindTdbValue object contains fixed-size data (type_size > 0), then
///    in general the data is an array of N fixed-size values (i.e.
///    data_size % type_size == 0 and N = data_size/type_size). As a special case,
///    the data contains a single scalar value if N is 1 (i.e. data_size == type_size).
///    If VAC is true
///      * The object represents N rows in a single column.
///      * The batch represents a rectangular table segment (K columns x N rows),
///        and must contain one SharemindTdbValue object per column (K in total).
///    If VAC is false
///      * The SharemindTdbValue object represents N values that cover a part
///        of a single row (one per column), but not necessarily the entire row.
///        The N values cover the next adjacent N columns as long as these columns
///        have same type that matches the value type.
///      * The batch represents a rectangular table segment (K columns x 1 row),
///        i.e. a single row. The batch must contain as many SharemindTdbValue
///        objects as necessary to cover the entire row exactly, i.e. the
///        total number of values represented by all SharemindTdbValue
///        objects in the batch is exactly equal the number of columns
///        in the table schema (K).
///
/// Stack arguments:
///  Mandatory `args[0]`: id of the vector map containing the data to be inserted (uint64).
///
/// Reference parameters:
///  Optional `refs[0]`: returned error code (int64)
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the data source containing the table (string)
///  Mandatory `crefs[1]`: name of the table (string)
///
/// No return value
tdb_insert_row2, |args, refs, crefs, return_value, c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 1, num_refs: 0, num_crefs: 2, need_return_value: false},
        SyscallRule {num_args: 1, num_refs: 1, num_crefs: 2, need_return_value: false}
    ])?;

    if !refs.is_empty() && refs[0].size != mem::size_of::<i64>() {
        return Err(syserr!(SyscallErrorKind::InvalidCall, "invalid refs[0]"));
    }

    let vmap_id: u64 = args[0].uint64[0];

    let datasource_name_cstr = ref_to_cstr_or_err!(crefs[0]);
    let datasource_name = ref_to_str_or_err!(crefs[0]);
    let table_name = ref_to_str_or_err!(crefs[1]);

    validate_datasource_name(datasource_name)?;
    validate_table_name(table_name)?;

    // Get the parameter vmap
    let pmap = get_vector_map(c, vmap_id)?;

    let mut batch_count: usize = 0;
    if pmap.batch_count.unwrap()(pmap, &mut batch_count) != SharemindTdbVectorMapError__TDB_VECTOR_MAP_OK {
        return Err(syserr!(SyscallErrorKind::GeneralError, "failed to get parameter vector map batch count"));
    }

    let mut value_batches: Vec<TdbValueBatch> = Vec::with_capacity(batch_count);

    // Process each parameter batch
    for i in 0..batch_count {
        if pmap.set_batch.unwrap()(pmap, i) != SharemindTdbVectorMapError__TDB_VECTOR_MAP_OK {
            return Err(syserr!(SyscallErrorKind::GeneralError, "failed to iterate parameter vector map batches"));
        }

        // Parse the "values" parameter
        let values = pmap.get_objects::<SharemindTdbValue>(c"values")?;
        if values.len() > u32::MAX.try_into().unwrap() {
            return Err(syserr!(SyscallErrorKind::InvalidCall, format!("batch contains too many values ({}) for tabledb RPC", values.len())));
        }

        // Check if the optional parameter "valueAsColumn" is set
        let mut vac_is_set = false;
        if pmap.is_index_vector.unwrap()(pmap, c"valueAsColumn".as_ptr(), &mut vac_is_set)
            != SharemindTdbVectorMapError__TDB_VECTOR_MAP_OK
        {
            return Err(syserr!(SyscallErrorKind::GeneralError, "failed to check \"valueAsColumn\" index vector"));
        }

        let value_as_column = if vac_is_set {
            // Parse the "valueAsColumn" parameter
            let mut count: usize = 0;
            let mut value_as_column: *mut *mut SharemindTdbIndex = ptr::null_mut();
            if pmap.get_index_vector.unwrap()(pmap, c"valueAsColumn".as_ptr(), &mut value_as_column, &mut count)
                != SharemindTdbVectorMapError__TDB_VECTOR_MAP_OK
            {
                return Err(syserr!(SyscallErrorKind::GeneralError, "failed to get \"valueAsColumn\" index vector parameter"));
            }

            if count < 1 {
                return Err(syserr!(SyscallErrorKind::GeneralError, "empty \"valueAsColumn\" index vector parameter"));
            }

            (*(*value_as_column)).idx != 0
        } else {
            // Set the default value
            false
        };

        value_batches.push(TdbValueBatch::from_ptr_slice(values, value_as_column, false)?);
    }

    // Get RPC interface
    let tabledb_rpc = get_tabledb_rpc(c)?;

    // Write batches to the database via RPC
    let write_result = tabledb_rpc.write(table_name, datasource_name, &value_batches);
    handle_tabledb_result(c, datasource_name_cstr, refs.first(), write_result)?;

    Ok(())
}}

sharemind_module_api_0x1_syscall! {
/// Read a column from a database table into a vector map.
/// Column can be read either by column index or by column name in the table schema.
/// One of them must be provided.
///
/// Stack arguments:
///  Optional `args[0]`: index of the table column (uint64).
///
/// Reference parameters:
///  Optional `refs[0]`: returned error code (int64)
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the data source containing the table (string)
///  Mandatory `crefs[1]`: name of the table (string)
///  Optional  `crefs[2]`: name of the table column according to table schema (string)
///
/// Return value: id of the vector map containing the data read from the table (uint64).
tdb_read_col, |args, refs, crefs, return_value, c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 1, num_refs: 0, num_crefs: 2, need_return_value: true},
        SyscallRule {num_args: 1, num_refs: 1, num_crefs: 2, need_return_value: true},
        SyscallRule {num_args: 0, num_refs: 0, num_crefs: 3, need_return_value: true},
        SyscallRule {num_args: 0, num_refs: 1, num_crefs: 3, need_return_value: true},
    ])?;

    if !refs.is_empty() && refs[0].size != mem::size_of::<i64>() {
        return Err(syserr!(SyscallErrorKind::InvalidCall, "invalid refs[0]"));
    }

    let datasource_name_cstr = ref_to_cstr_or_err!(crefs[0]);
    let datasource_name = ref_to_str_or_err!(crefs[0]);
    let table_name = ref_to_str_or_err!(crefs[1]);

    validate_datasource_name(datasource_name)?;
    validate_table_name(table_name)?;

    // Get RPC interface
    let tabledb_rpc = get_tabledb_rpc(c)?;

    // Retrieve table metadata via RPC
    let table_metadata = {
        let metadata_result = tabledb_rpc.get_table_metadata(table_name, datasource_name);
        if let Some(metadata) = handle_tabledb_result(c, datasource_name_cstr, refs.first(), metadata_result)? {
            metadata
        } else {
            return Ok(());  // non-OK error code stored in VM reference, return early
        }
    };

    // Get the index of the column to read from the database table
    let col_idx = if (SyscallRule {num_args: 1, num_refs: 0, num_crefs: 2, need_return_value: true}).applies(args, &[], crefs, return_value) {
        // Read the column by column index
        let col_idx = args[0].uint64[0];

        if col_idx >= table_metadata.get_column_count() as u64 {
            return Err(syserr!(SyscallErrorKind::InvalidCall, "column number out of range"));
        }

        col_idx
    } else {
        assert!((SyscallRule {num_args: 0, num_refs: 0, num_crefs: 3, need_return_value: true}).applies(args, &[], crefs, return_value));

        // Read the column by column name
        let col_name = ref_to_str_or_err!(crefs[2]);

        // Convert column name to column index.
        // Linear search, but lets optimize when it becomes a problem.
        table_metadata
            .get_schema()
            .iter()
            .position(|col| col.name == col_name)
            .ok_or_else(|| syserr!(SyscallErrorKind::InvalidCall, format!("table does not contain column \"{col_name}\"")))? as u64
    };

    // Read a column from the database table via RPC
    let values = {
        let read_result = tabledb_rpc.read(table_name, datasource_name, col_idx, &table_metadata);
        if let Some(values) = handle_tabledb_result(c, datasource_name_cstr, refs.first(), read_result)? {
            values
        } else {
            return Ok(()); // non-OK error code stored in VM reference, return early
        }
    };

    // Store the values for the column in a result map.
    // Note: One batch already exists in the new result map.
    //       Should we want to support the retrieval of multiple columns, then
    //       a new batch must be added in result map for every additional column.
    //       This would require a different version of `store_objects_in_new_vector_map`.
    let vmap_id = action_with_new_vector_map(c, |vmap| {
        vmap.store_objects(c"values", values)
    })?;

    (*return_value).uint64[0] = vmap_id;

    Ok(())
}}

sharemind_module_api_0x1_syscall! {
/// Retrieve a list of attributes for the database table.
///
/// NB! Table attributes are currently not supported in our Carbyne Stack integration.
/// This syscall is provided for compatibility with existing code.
/// It currently returns no attributes.
///
/// Stack arguments:
///  N/A
///
/// Reference parameters:
///  N/A
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the data source containing the table (string)
///  Mandatory `crefs[1]`: name of the table (string)
///
/// Return value: id of the vector map containing the retrieved table attributes
///               in the "keys" and "values" string vectors (uint64).
tdb_get_attributes, |args, refs, crefs, return_value, c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 0, num_refs: 0, num_crefs: 2, need_return_value: true}
    ])?;

    // Table attributes currently not supported.
    // This syscall returns no attributes.
    let keys = PointerVector::<SharemindTdbString>::new(0, None);
    let values = PointerVector::<SharemindTdbString>::new(0, None);

    let vmap_id = action_with_new_vector_map(c, |vmap| {
        vmap.store_objects(c"keys", keys)?;
        vmap.store_objects(c"values", values)
    })?;

    (*return_value).uint64[0] = vmap_id;

    Ok(())
}}

sharemind_module_api_0x1_syscall! {
/// Set attributes for the database table.
///
/// NB! Table attributes are currently not supported in our Carbyne Stack integration.
/// This syscall is provided for compatibility with existing code.
/// It currently does nothing.
///
/// Stack arguments:
///  Mandatory `args[0]`: id of the vector map containing the table attributes (uint64)
///
/// Reference parameters:
///  N/A
///
/// Constant reference parameters:
///  Mandatory `crefs[0]`: name of the data source containing the table (string)
///  Mandatory `crefs[1]`: name of the table (string)
///
/// No return value
tdb_set_attributes, |args, refs, crefs, return_value, _c| {
    validate_syscall(args, refs, crefs, return_value, &[
        SyscallRule {num_args: 1, num_refs: 0, num_crefs: 2, need_return_value: false}
    ])?;

    // Table attributes currently not supported.
    // This syscall is no-op.
    Ok(())
}}
