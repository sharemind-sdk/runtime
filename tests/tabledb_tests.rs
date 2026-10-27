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

// Compile this file only if tabledb feature is enabled
#![cfg(feature = "tabledb")] // inner attribute

use core::{panic, str};
use std::{
    cell::RefCell,
    collections::{HashSet, VecDeque},
    error::Error,
    path::PathBuf,
    process::exit,
    rc::Rc,
    sync::LazyLock,
    thread,
};

use rand::{seq::IteratorRandom, Rng};
use sharemind_runtime::{
    self,
    ffi::{
        bindings::ProcessExitStatus_PROCESS_EXIT_SYSCALL_ERROR,
        facility::network::NetworkFacility,
        loghard::LogHardLogger,
        primitive_types::{convert_into_vmdata, VmData, VmDataRef, VmDataView, VmType},
        vm::ProcessFacilityMap,
    },
    network::{
        self,
        in_memory::{self, InmemoryNetworkFactory},
    },
    program_io::{ProgramArgument, ProgramArgumentMap},
    syscalls::tabledb::{rpc::TabledbRpc, types::TdbType},
    test_util::tabledb_mock::{Column, MockDatabaseServer, MockDatabaseState, Table},
    SharemindRuntime,
};
use strum::IntoEnumIterator;

// General notes
//
// If data is written to MockDatabase, we can observe the MockDatabase object state after the request.
// If data is read from MockDatabase, we can pre-configure the database state and validate that we get same data on the reader side.
// Alternatively, we could implement hooks in MockDatabase methods for directly inspecting RPC requests.

const DATASOURCE_NAME: &str = "DS1";

static GLOBAL_CONTEXT: LazyLock<GlobalContext> = LazyLock::new(GlobalContext::new);

/// Initializes the global context common for all tests on first access.
/// Each test must call this first before proceeding with the test to ensure
/// that the global context is initialized.
pub fn init_global_context() {
    // Explicitly force lazy initialization
    LazyLock::force(&GLOBAL_CONTEXT);
}

struct GlobalContext {
    // The same logger is reused for all the tests
    logger: LogHardLogger,
}

impl GlobalContext {
    pub fn new() -> Self {
        // Initialize tracing for structured logging.
        // Environment variable RUST_LOG can be used to customize the logging level.
        let env_filter =
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or("info".into());
        let sub = tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(false)
            .finish();
        tracing::subscriber::set_global_default(sub)
            .expect("Failed to set tracing global default subscriber");

        // Setup LogHard logging
        // Shared between the module and Rust's logging facade.
        #[allow(unused_mut)]
        let mut logger = LogHardLogger::new(
            &tracing::level_filters::LevelFilter::current().to_string(),
            c"",
        );
        if let Err(e) = log::set_boxed_logger(Box::new(logger.sublogger(c"[Log]"))) {
            println!("Failed to set global logger: {}", e);
            exit(1);
        };

        GlobalContext { logger }
    }
}

struct TestContext {
    network_contexts: VecDeque<network::in_memory::context::WorkerContext>,
}

struct RuntimeContext {
    // Observable tabledb state
    tabledb_state: Rc<RefCell<MockDatabaseState>>,

    // Tabledb RPC facility object. One instance for one runtime.
    // Boxed, because this object is accessed by pointer by the FFI code,
    // and it needs a fixed memory address unaffected by moves and returns.
    _tabledb_rpc: Box<TabledbRpc>,
}

impl TestContext {
    fn new() -> Self {
        // Ensure global context is initialized
        init_global_context();

        // Create the interconnected in-memory network contexts
        let network_contexts = VecDeque::from(in_memory::context::create_contexts());

        Self { network_contexts }
    }

    fn new_runtime(&mut self) -> Result<(SharemindRuntime, RuntimeContext), Box<dyn Error>> {
        // Create the network facility
        let network_ctx = self
            .network_contexts
            .pop_front()
            .ok_or("no more network contexts available")?;
        let net_factory = Box::new(InmemoryNetworkFactory::new(network_ctx));
        let net_facility = NetworkFacility::new(net_factory);

        // Create the TableDB RPC facility mocked with an in-memory database with observable shared state
        let tabledb_state = Rc::new(RefCell::new(MockDatabaseState::new()));
        let tabledb_client = MockDatabaseServer::new(tabledb_state.clone()).into_client();
        let mut tabledb_rpc = Box::new(TabledbRpc::with_client(tabledb_client)?);

        let mut process_facilities = ProcessFacilityMap::new();
        process_facilities.set_facility(tabledb_rpc.as_mut());

        let mut runtime =
            SharemindRuntime::new(&GLOBAL_CONTEXT.logger, net_facility, process_facilities);

        // Load feature modules
        runtime.load_module(&PathBuf::from("libsharemind_mod_algorithms.so"), None)?;

        runtime.load_module(
            &PathBuf::from("libsharemind_mod_tabledb.so"),
            Some(&PathBuf::from("tests/tabledb_tests/config/tabledb.conf")),
        )?;

        // Skip loading protection domains, as tabledb tests do not use any.

        Ok((
            runtime,
            RuntimeContext {
                tabledb_state,
                _tabledb_rpc: tabledb_rpc,
            },
        ))
    }
}

fn new_public_column_type(vm_type: VmType) -> TdbType {
    TdbType {
        domain: "public".to_owned(),
        name: vm_type.type_name().to_owned(),
        size: vm_type.type_size().unwrap_or(0) as u64,
    }
}

fn new_public_vlen_column_type(vm_type: VmType) -> TdbType {
    TdbType {
        domain: "public".to_owned(),
        name: vm_type.type_name().to_owned(),
        size: 0, // enforce variable-length column type
    }
}

fn create_two_different_tables() -> (Table, Table) {
    let table1 = Table::from_vec(vec![
        Column::new("col11", new_public_column_type(VmType::Uint64)),
        Column::new("col12", new_public_column_type(VmType::Float32)),
    ])
    .unwrap();

    let table2 = Table::from_vec(vec![
        Column::new("col21", new_public_column_type(VmType::Uint32)),
        Column::new("col22", new_public_column_type(VmType::Float64)),
        Column::new("col23", new_public_column_type(VmType::Bool)),
    ])
    .unwrap();

    (table1, table2)
}

fn check_exit_status_is_syscall_error<E: std::fmt::Display>(err: &E) -> bool {
    let error_msg = format!("{err}");
    let expected_exit_status = format!(
        "exit status {}",
        ProcessExitStatus_PROCESS_EXIT_SYSCALL_ERROR
    );
    error_msg.contains(&expected_exit_status)
}

fn check_expected_result<T, E>(result: Result<T, E>, should_succeed: bool) -> Option<T>
where
    E: std::fmt::Debug + std::fmt::Display,
{
    match (result, should_succeed) {
        (Ok(v), true) => Some(v), // Expected success.
        (Err(err), true) => {
            panic!(
                "{:?} Operation failed but was expected to succeed: {:?}",
                thread::current().id(),
                err
            );
        }
        (Ok(_), false) => {
            panic!(
                "{:?} Operation succeeded but was expected to fail",
                thread::current().id()
            );
        }
        (Err(err), false) => {
            if check_exit_status_is_syscall_error(&err) {
                tracing::info!("Operation failed as expected");
            } else {
                panic!(
                    "{:?} Operation failed as expected, but not with a syscall error: {:?}",
                    thread::current().id(),
                    err
                );
            }

            None
        }
    }
}

type PA = ProgramArgument;

#[test]
fn test_tdb_table_names() {
    let mut test_context = TestContext::new();

    // Setup the runtime
    let (mut runtime, runtime_context) = test_context.new_runtime().unwrap();

    // Setup the mock database state
    runtime_context
        .tabledb_state
        .borrow_mut()
        .modify_datasource_tables(DATASOURCE_NAME, true, |ds_tables| {
            assert!(ds_tables.is_empty());
            let (table1, table2) = create_two_different_tables();
            assert_eq!(ds_tables.insert("table1".to_owned(), table1), None);
            assert_eq!(ds_tables.insert("table2".to_owned(), table2), None);
            Ok(())
        })
        .unwrap();

    // Execute a bytecode process
    let arguments = ProgramArgumentMap::from([(
        "datasource_name".to_owned(),
        PA::from(convert_into_vmdata(DATASOURCE_NAME).unwrap()),
    )]);

    let result = runtime.execute_program("tests/tabledb_tests/tdb_table_names.sb", arguments);
    let results = check_expected_result(result, true).unwrap();

    // Inspect bytecode results
    runtime_context
        .tabledb_state
        .borrow()
        .read_datasource_tables(DATASOURCE_NAME, |ds_tables| {
            let expected_num_tables = ds_tables.len();

            assert_eq!(
                results.len(),
                expected_num_tables + 1 /* names_count */
            );
            let names_count = results
                .get("names_count")
                .unwrap()
                .as_vm_data()
                .unwrap()
                .to_value::<u64>()
                .unwrap();
            assert_eq!(names_count, expected_num_tables as u64);

            // Parse table names from results
            let mut returned_table_names = HashSet::with_capacity(expected_num_tables);
            for i in 0..expected_num_tables {
                // Parse next result
                let result_name = format!("name_{}", i);
                let table_name = results
                    .get(&result_name)
                    .unwrap()
                    .as_vm_data()
                    .unwrap()
                    .to_value::<String>()
                    .unwrap();
                assert!(returned_table_names.insert(table_name));
            }

            // Validate that results contain all the tables in datasource
            for ds_table_name in ds_tables.keys() {
                assert!(returned_table_names.contains(ds_table_name.as_str()));
            }

            Ok(())
        })
        .unwrap();
}

#[test]
fn test_tdb_tbl_create() {
    let mut test_context = TestContext::new();

    // Setup the runtime
    let (mut runtime, runtime_context) = test_context.new_runtime().unwrap();

    let mut test_with_column_type = |vm_type: VmType| {
        tracing::info!(
            "Testing tdb_tbl_create with column type {:?}",
            vm_type.type_name()
        );

        // Setup the mock database state
        runtime_context
            .tabledb_state
            .borrow_mut()
            .modify_datasource_tables(DATASOURCE_NAME, true, |ds_tables| {
                ds_tables.clear(); // reset tables
                Ok(())
            })
            .unwrap();

        // Create test data
        // Requirement: All columns have the same type and are named by their index starting with 0.

        let col_type = new_public_column_type(vm_type);
        let table1 = Table::from_vec(vec![
            Column::new("0", col_type.clone()),
            Column::new("1", col_type.clone()),
            Column::new("2", col_type.clone()),
            Column::new("3", col_type.clone()),
            Column::new("4", col_type.clone()),
        ])
        .unwrap();

        // Prepare program arguments
        let mut arguments = ProgramArgumentMap::from([
            (
                "datasource_name".to_owned(),
                PA::from(convert_into_vmdata(DATASOURCE_NAME).unwrap()),
            ),
            (
                "table_name".to_owned(),
                PA::from(convert_into_vmdata("table1").unwrap()),
            ),
            (
                "column_count".to_owned(),
                PA::from(convert_into_vmdata(table1.column_count() as u64).unwrap()),
            ),
            (
                "type_name".to_owned(),
                PA::from(convert_into_vmdata(col_type.name.as_str()).unwrap()),
            ),
            (
                "type_domain".to_owned(),
                PA::from(convert_into_vmdata(col_type.domain.as_str()).unwrap()),
            ),
        ]);

        // Execute a bytecode process
        let result =
            runtime.execute_program("tests/tabledb_tests/tdb_tbl_create.sb", arguments.clone());
        let _ = check_expected_result(result, true).unwrap();

        // Inspect db state
        runtime_context
            .tabledb_state
            .borrow()
            .read_datasource_tables(DATASOURCE_NAME, |ds_tables| {
                assert_eq!(ds_tables.len(), 1);
                let ds_table1 = ds_tables.get("table1").unwrap();
                assert_eq!(table1, *ds_table1);

                // Test with one extra column in test data
                let mut table1a = table1;
                table1a
                    .add_column(Column::new("5", col_type.clone()))
                    .unwrap();
                assert_ne!(table1a, *ds_table1);
                Ok(())
            })
            .unwrap();

        // Test with zero columns. Should fail.
        arguments.insert(
            "column_count".to_owned(),
            PA::from(convert_into_vmdata(0_u64).unwrap()),
        );
        let result = runtime.execute_program("tests/tabledb_tests/tdb_tbl_create.sb", arguments);
        let _ = check_expected_result(result, false);
    };

    // Run tests for each column type individually

    // Boolean
    test_with_column_type(VmType::Bool);

    // Unsigned integers
    test_with_column_type(VmType::Uint8);
    test_with_column_type(VmType::Uint16);
    test_with_column_type(VmType::Uint32);
    test_with_column_type(VmType::Uint64);

    // Signed integers
    test_with_column_type(VmType::Int8);
    test_with_column_type(VmType::Int16);
    test_with_column_type(VmType::Int32);
    test_with_column_type(VmType::Int64);

    // Floating point
    test_with_column_type(VmType::Float32);
    test_with_column_type(VmType::Float64);
}

#[test]
fn test_tdb_tbl_create2() {
    let mut test_context = TestContext::new();

    // Setup the runtime
    let (mut runtime, runtime_context) = test_context.new_runtime().unwrap();

    // Setup the mock database state
    runtime_context
        .tabledb_state
        .borrow_mut()
        .modify_datasource_tables(DATASOURCE_NAME, true, |ds_tables| {
            ds_tables.clear();
            Ok(())
        })
        .unwrap();

    // Create test data

    let table1 = Table::from_vec(vec![
        Column::new("col_0", new_public_column_type(VmType::Bool)),
        Column::new("col_1", new_public_column_type(VmType::Uint8)),
        Column::new("col_2", new_public_column_type(VmType::Uint16)),
        Column::new("col_3", new_public_column_type(VmType::Uint32)),
        Column::new("col_4", new_public_column_type(VmType::Uint64)),
        Column::new("col_5", new_public_column_type(VmType::Int8)),
        Column::new("col_6", new_public_column_type(VmType::Int16)),
        Column::new("col_7", new_public_column_type(VmType::Int32)),
        Column::new("col_8", new_public_column_type(VmType::Int64)),
        Column::new("col_9", new_public_column_type(VmType::Float32)),
        Column::new("col_10", new_public_column_type(VmType::Float64)),
    ])
    .unwrap();

    // Prepare program arguments
    let mut arguments = ProgramArgumentMap::from([
        (
            "datasource_name".to_owned(),
            PA::from(convert_into_vmdata(DATASOURCE_NAME).unwrap()),
        ),
        (
            "table_name".to_owned(),
            PA::from(convert_into_vmdata("table1").unwrap()),
        ),
        (
            "column_count".to_owned(),
            PA::from(convert_into_vmdata(table1.column_count() as u64).unwrap()),
        ),
    ]);

    for (i, column) in table1.schema.iter().enumerate() {
        assert!(arguments
            .insert(
                format!("column_name_{}", i),
                PA::from(convert_into_vmdata(column.name.as_str()).unwrap())
            )
            .is_none());
        let ctype = &column.type_;
        assert!(arguments
            .insert(
                format!("type_name_{}", i),
                PA::from(convert_into_vmdata(ctype.name.as_str()).unwrap())
            )
            .is_none());
        assert!(arguments
            .insert(
                format!("type_domain_{}", i),
                PA::from(convert_into_vmdata(ctype.domain.as_str()).unwrap())
            )
            .is_none());
    }

    // Execute a bytecode process
    let result = runtime.execute_program("tests/tabledb_tests/tdb_tbl_create2.sb", arguments);
    let _ = check_expected_result(result, true).unwrap();

    // Inspect db state
    runtime_context
        .tabledb_state
        .borrow()
        .read_datasource_tables(DATASOURCE_NAME, |ds_tables| {
            assert_eq!(ds_tables.len(), 1);
            let ds_table1 = ds_tables.get("table1").unwrap();
            assert_eq!(table1, *ds_table1);

            // Test with one extra column in test data
            let mut table1a = table1;
            table1a
                .add_column(Column::new("5", new_public_column_type(VmType::Uint64)))
                .unwrap();
            assert_ne!(table1a, *ds_table1);
            Ok(())
        })
        .unwrap();
}

#[test]
fn test_tdb_tbl_delete() {
    let mut test_context = TestContext::new();

    // Setup the runtime
    let (mut runtime, runtime_context) = test_context.new_runtime().unwrap();

    // Create test data
    let (table1, table2) = create_two_different_tables();

    // Setup the mock database state
    runtime_context
        .tabledb_state
        .borrow_mut()
        .modify_datasource_tables(DATASOURCE_NAME, true, |ds_tables| {
            assert!(ds_tables.is_empty());
            assert_eq!(ds_tables.insert("table1".to_owned(), table1), None);
            assert_eq!(ds_tables.insert("table2".to_owned(), table2.clone()), None);
            assert!(ds_tables.contains_key("table1"));
            assert!(ds_tables.contains_key("table2"));
            assert!(!ds_tables.contains_key("table_missing"));
            Ok(())
        })
        .unwrap();

    // Execute a bytecode process
    let arguments = ProgramArgumentMap::from([
        (
            "datasource_name".to_owned(),
            PA::from(convert_into_vmdata(DATASOURCE_NAME).unwrap()),
        ),
        (
            "table_name".to_owned(),
            PA::from(convert_into_vmdata("table1").unwrap()),
        ),
    ]);

    let result = runtime.execute_program("tests/tabledb_tests/tdb_tbl_delete.sb", arguments);
    let _ = check_expected_result(result, true).unwrap();

    // Inspect db state
    runtime_context
        .tabledb_state
        .borrow()
        .read_datasource_tables(DATASOURCE_NAME, |ds_tables| {
            assert_eq!(ds_tables.len(), 1);
            assert!(!ds_tables.contains_key("table1"));
            assert!(ds_tables.contains_key("table2"));
            let ds_table2 = ds_tables.get("table2").unwrap();
            assert_eq!(table2, *ds_table2);
            Ok(())
        })
        .unwrap();

    // Try deleting non-existent table (should fail)
    let arguments = ProgramArgumentMap::from([
        (
            "datasource_name".to_owned(),
            PA::from(convert_into_vmdata(DATASOURCE_NAME).unwrap()),
        ),
        (
            "table_name".to_owned(),
            PA::from(convert_into_vmdata("table_missing").unwrap()),
        ),
    ]);

    let result = runtime.execute_program("tests/tabledb_tests/tdb_tbl_delete.sb", arguments);
    let _ = check_expected_result(result, false);
}

#[test]
fn test_tdb_tbl_exists() {
    let mut test_context = TestContext::new();

    // Setup the runtime
    let (mut runtime, runtime_context) = test_context.new_runtime().unwrap();

    // Create test data
    let (table1, table2) = create_two_different_tables();

    // Setup the mock database state
    runtime_context
        .tabledb_state
        .borrow_mut()
        .modify_datasource_tables(DATASOURCE_NAME, true, |ds_tables| {
            assert!(ds_tables.is_empty());
            assert_eq!(ds_tables.insert("table1".to_owned(), table1), None);
            assert_eq!(ds_tables.insert("table2".to_owned(), table2.clone()), None);
            assert!(ds_tables.contains_key("table1"));
            assert!(ds_tables.contains_key("table2"));
            assert!(!ds_tables.contains_key("table_missing"));
            Ok(())
        })
        .unwrap();

    let mut test_with_table = |table_name: &str, expected_to_exist: bool| {
        tracing::info!("Testing tdb_tbl_exists with table: {}", table_name);

        // Execute a bytecode process
        let arguments = ProgramArgumentMap::from([
            (
                "datasource_name".to_owned(),
                PA::from(convert_into_vmdata(DATASOURCE_NAME).unwrap()),
            ),
            (
                "table_name".to_owned(),
                PA::from(convert_into_vmdata(table_name).unwrap()),
            ),
        ]);

        let result = runtime.execute_program("tests/tabledb_tests/tdb_tbl_exists.sb", arguments);
        let results = check_expected_result(result, true).unwrap();

        // Inspect bytecode results
        {
            assert_eq!(results.len(), 1);
            let table_exists = results
                .get("table_exists")
                .unwrap()
                .as_vm_data()
                .unwrap()
                .to_value::<bool>()
                .unwrap();
            assert_eq!(table_exists, expected_to_exist);
        }
    };

    // Test with existing tables (should succeed)
    test_with_table("table1", true);
    test_with_table("table2", true);

    // Test with non-existent table (should fail)
    test_with_table("table_missing", false);
}

#[test]
fn test_tdb_tbl_col_count() {
    let mut test_context = TestContext::new();

    // Setup the runtime
    let (mut runtime, runtime_context) = test_context.new_runtime().unwrap();

    // Create test data
    let (table1, table2) = create_two_different_tables();
    let table1_col_count = table1.column_count() as u64;
    let table2_col_count = table2.column_count() as u64;

    // Setup the mock database state
    runtime_context
        .tabledb_state
        .borrow_mut()
        .modify_datasource_tables(DATASOURCE_NAME, true, |ds_tables| {
            assert!(ds_tables.is_empty());
            assert_eq!(ds_tables.insert("table1".to_owned(), table1), None);
            assert_eq!(ds_tables.insert("table2".to_owned(), table2), None);
            assert!(ds_tables.contains_key("table1"));
            assert!(ds_tables.contains_key("table2"));
            assert!(!ds_tables.contains_key("table_missing"));
            Ok(())
        })
        .unwrap();

    let mut test_with_table = |table_name: &str, expected_col_count: Option<u64>| {
        tracing::info!("Testing tdb_tbl_col_count with table: {}", table_name);

        // Execute a bytecode process
        let arguments = ProgramArgumentMap::from([
            (
                "datasource_name".to_owned(),
                PA::from(convert_into_vmdata(DATASOURCE_NAME).unwrap()),
            ),
            (
                "table_name".to_owned(),
                PA::from(convert_into_vmdata(table_name).unwrap()),
            ),
        ]);

        let result = runtime.execute_program("tests/tabledb_tests/tdb_tbl_col_count.sb", arguments);

        // Inspect bytecode results
        let should_succeed = expected_col_count.is_some();
        if let Some(result) = check_expected_result(result, should_succeed) {
            assert_eq!(result.len(), 1);
            let column_count = result
                .get("column_count")
                .unwrap()
                .as_vm_data()
                .unwrap()
                .to_value::<u64>()
                .unwrap();
            assert_eq!(column_count, expected_col_count.unwrap());
        }
    };

    // Test with existing tables (should succeed)
    test_with_table("table1", Some(table1_col_count));
    test_with_table("table2", Some(table2_col_count));

    // Test with non-existent table (should fail)
    test_with_table("table_missing", None);
}

#[test]
fn test_tdb_tbl_col_names() {
    let mut test_context = TestContext::new();

    // Setup the runtime
    let (mut runtime, runtime_context) = test_context.new_runtime().unwrap();

    // Create test data
    let (table1, table2) = create_two_different_tables();

    // Setup the mock database state
    runtime_context
        .tabledb_state
        .borrow_mut()
        .modify_datasource_tables(DATASOURCE_NAME, true, |ds_tables| {
            assert!(ds_tables.is_empty());
            assert_eq!(ds_tables.insert("table1".to_owned(), table1.clone()), None);
            assert_eq!(ds_tables.insert("table2".to_owned(), table2.clone()), None);
            assert!(ds_tables.contains_key("table1"));
            assert!(ds_tables.contains_key("table2"));
            assert!(!ds_tables.contains_key("table_missing"));
            Ok(())
        })
        .unwrap();

    let mut test_with_table = |table_name: &str, reference_table: Option<&Table>| {
        tracing::info!("Testing tdb_tbl_col_names with table: {}", table_name);

        // Execute a bytecode process
        let arguments = ProgramArgumentMap::from([
            (
                "datasource_name".to_owned(),
                PA::from(convert_into_vmdata(DATASOURCE_NAME).unwrap()),
            ),
            (
                "table_name".to_owned(),
                PA::from(convert_into_vmdata(table_name).unwrap()),
            ),
        ]);

        let result = runtime.execute_program("tests/tabledb_tests/tdb_tbl_col_names.sb", arguments);

        // Inspect bytecode results
        let should_succeed = reference_table.is_some();
        if let Some(results) = check_expected_result(result, should_succeed) {
            let table = reference_table.unwrap();
            let expected_col_count = table.column_count();
            assert_eq!(results.len(), expected_col_count + 1 /* names_count */);

            let names_count = results
                .get("names_count")
                .unwrap()
                .as_vm_data()
                .unwrap()
                .to_value::<u64>()
                .unwrap();
            assert_eq!(names_count, expected_col_count as u64);

            // Parse column names from results
            let mut returned_col_names = HashSet::with_capacity(expected_col_count);
            for i in 0..expected_col_count {
                // Parse next result
                let result_name = format!("name_{}", i);
                let col_name = results
                    .get(&result_name)
                    .unwrap()
                    .as_vm_data()
                    .unwrap()
                    .to_value::<String>()
                    .unwrap();
                assert!(returned_col_names.insert(col_name));
            }

            // Validate that results contain all the tables in datasource
            for column in &table.schema {
                assert!(returned_col_names.contains(column.name.as_str()));
            }
        }
    };

    // Test with existing tables (should succeed)
    test_with_table("table1", Some(&table1));
    test_with_table("table2", Some(&table2));

    // Test with non-existent table (should fail)
    test_with_table("table_missing", None);
}

#[test]
fn test_tdb_tbl_col_types() {
    let mut test_context = TestContext::new();

    // Setup the runtime
    let (mut runtime, runtime_context) = test_context.new_runtime().unwrap();

    // Create test data
    let (table1, table2) = create_two_different_tables();

    // Setup the mock database state
    runtime_context
        .tabledb_state
        .borrow_mut()
        .modify_datasource_tables(DATASOURCE_NAME, true, |ds_tables| {
            assert!(ds_tables.is_empty());
            assert_eq!(ds_tables.insert("table1".to_owned(), table1.clone()), None);
            assert_eq!(ds_tables.insert("table2".to_owned(), table2.clone()), None);
            assert!(ds_tables.contains_key("table1"));
            assert!(ds_tables.contains_key("table2"));
            assert!(!ds_tables.contains_key("table_missing"));
            Ok(())
        })
        .unwrap();

    let mut test_with_table = |table_name: &str, reference_table: Option<&Table>| {
        tracing::info!("Testing tdb_tbl_col_types with table: {}", table_name);

        // Execute a bytecode process
        let arguments = ProgramArgumentMap::from([
            (
                "datasource_name".to_owned(),
                PA::from(convert_into_vmdata(DATASOURCE_NAME).unwrap()),
            ),
            (
                "table_name".to_owned(),
                PA::from(convert_into_vmdata(table_name).unwrap()),
            ),
        ]);

        let result = runtime.execute_program("tests/tabledb_tests/tdb_tbl_col_types.sb", arguments);

        // Inspect bytecode results
        let should_succeed = reference_table.is_some();
        if let Some(results) = check_expected_result(result, should_succeed) {
            let table = reference_table.unwrap();
            let expected_col_count = table.column_count();
            assert_eq!(results.len(), expected_col_count + 1 /* names_count */);

            let types_count = results
                .get("names_count")
                .unwrap()
                .as_vm_data()
                .unwrap()
                .to_value::<u64>()
                .unwrap();
            assert_eq!(types_count, expected_col_count as u64);

            // Parse type names from results
            let mut returned_col_types = HashSet::with_capacity(expected_col_count);
            for i in 0..expected_col_count {
                // Parse next result
                let result_name = format!("name_{}", i);
                let type_name = results
                    .get(&result_name)
                    .unwrap()
                    .as_vm_data()
                    .unwrap()
                    .to_value::<String>()
                    .unwrap();
                assert!(returned_col_types.insert(type_name));
            }

            // Validate that results contain all the tables in datasource
            for column in &table.schema {
                assert!(returned_col_types.contains(column.type_.name.as_str()));
            }
        }
    };

    // Test with existing tables (should succeed)
    test_with_table("table1", Some(&table1));
    test_with_table("table2", Some(&table2));

    // Test with non-existent table (should fail)
    test_with_table("table_missing", None);
}

#[test]
fn test_tdb_tbl_row_count() {
    let mut test_context = TestContext::new();

    // Setup the runtime
    let (mut runtime, runtime_context) = test_context.new_runtime().unwrap();

    // Create test data
    let (mut table1, mut table2) = create_two_different_tables();
    const TABLE1_ROW_COUNT: u64 = 42;
    const TABLE2_ROW_COUNT: u64 = 123;
    table1.row_count = TABLE1_ROW_COUNT;
    table2.row_count = TABLE2_ROW_COUNT;

    // Setup the mock database state
    runtime_context
        .tabledb_state
        .borrow_mut()
        .modify_datasource_tables(DATASOURCE_NAME, true, |ds_tables| {
            assert!(ds_tables.is_empty());
            assert_eq!(ds_tables.insert("table1".to_owned(), table1), None);
            assert_eq!(ds_tables.insert("table2".to_owned(), table2), None);
            assert!(ds_tables.contains_key("table1"));
            assert!(ds_tables.contains_key("table2"));
            assert!(!ds_tables.contains_key("table_missing"));
            Ok(())
        })
        .unwrap();

    let mut test_with_table = |table_name: &str, expected_row_count: Option<u64>| {
        tracing::info!("Testing tdb_tbl_row_count with table: {}", table_name);

        // Execute a bytecode process
        let arguments = ProgramArgumentMap::from([
            (
                "datasource_name".to_owned(),
                PA::from(convert_into_vmdata(DATASOURCE_NAME).unwrap()),
            ),
            (
                "table_name".to_owned(),
                PA::from(convert_into_vmdata(table_name).unwrap()),
            ),
        ]);

        let result = runtime.execute_program("tests/tabledb_tests/tdb_tbl_row_count.sb", arguments);

        // Inspect bytecode results
        let should_succeed = expected_row_count.is_some();
        if let Some(result) = check_expected_result(result, should_succeed) {
            assert_eq!(result.len(), 1);
            let row_count = result
                .get("row_count")
                .unwrap()
                .as_vm_data()
                .unwrap()
                .to_value::<u64>()
                .unwrap();
            assert_eq!(row_count, expected_row_count.unwrap());
        }
    };

    // Test with existing tables (should succeed)
    test_with_table("table1", Some(TABLE1_ROW_COUNT));
    test_with_table("table2", Some(TABLE2_ROW_COUNT));

    // Test with non-existent table (should fail)
    test_with_table("table_missing", None);
}

//------------------------------------------------------------------------------
// Boilerplate for tdb_insert_row tests
//------------------------------------------------------------------------------

fn test_tdb_insert_row_with_type(
    vm_type: VmType,
    num_of_columns: usize,
    num_of_values: usize,
    value_as_column: bool,
    shall_pass: bool,
) {
    let mut test_context = TestContext::new();

    // Setup the runtime
    let (mut runtime, runtime_context) = test_context.new_runtime().unwrap();

    tracing::info!("Testing tdb_insert_row with column type: {:?}", vm_type);

    // Setup the mock database state
    // Requirement: All columns have the same type and are named by their index starting with 0.

    let table1 = Table::from_vec(
        (0..num_of_columns)
            .map(|i| Column::new(i.to_string().as_str(), new_public_column_type(vm_type)))
            .collect(),
    )
    .unwrap();

    runtime_context
        .tabledb_state
        .borrow_mut()
        .modify_datasource_tables(DATASOURCE_NAME, true, |ds_tables| {
            assert!(ds_tables.is_empty());
            assert_eq!(ds_tables.insert("table1".to_owned(), table1), None);
            assert!(ds_tables.contains_key("table1"));
            Ok(())
        })
        .unwrap();

    // Create test data for multiple consecutive insertions.
    const NUM_DATAS: usize = 3;
    let test_datas = (0..NUM_DATAS)
        .map(|_| VmData::random(vm_type, num_of_values))
        .collect::<Vec<_>>();

    // Test with multiple consecutive tdb_insert_row calls.
    // Later reading a column should return all rows for that column.
    for test_data in &test_datas {
        // Prepare program arguments for inserting rows
        let arguments = ProgramArgumentMap::from([
            (
                "datasource_name".to_owned(),
                PA::from(convert_into_vmdata(DATASOURCE_NAME).unwrap()),
            ),
            (
                "table_name".to_owned(),
                PA::from(convert_into_vmdata("table1").unwrap()),
            ),
            (
                "type_name".to_owned(),
                PA::from(convert_into_vmdata(vm_type.type_name()).unwrap()),
            ),
            (
                "value_as_column".to_owned(),
                PA::from(convert_into_vmdata(value_as_column).unwrap()),
            ),
            ("data_values".to_owned(), PA::from(test_data.clone())),
        ]);

        // Execute a bytecode process for inserting data
        let result =
            runtime.execute_program("tests/tabledb_tests/tdb_insert_row.sb", arguments.clone());

        // Inspect bytecode results
        if let Some(results) = check_expected_result(result, shall_pass) {
            assert!(results.is_empty()); // nothing is published
        } else {
            return; // Expected failure
        }
    }

    // Convert test datas from row-major Vec<VmData> to column-major Vec<VmData>
    let expected_row_count = test_datas.len()
        * if vm_type.is_variable_length() {
            1 // whole buffer is a value of a single row
        } else if value_as_column {
            num_of_values // values go to a single column
        } else {
            1 // values go to a signle row
        };

    let mut test_columns_bytes = vec![Vec::with_capacity(expected_row_count); num_of_columns];
    for data in test_datas {
        if value_as_column {
            assert_eq!(1, num_of_columns);
            assert_eq!(1, test_columns_bytes.len());

            let column_bytes = test_columns_bytes.first_mut().unwrap();
            for i in 0..data.len() {
                column_bytes.extend_from_slice(data.get_raw_at(i).unwrap());
            }
        } else {
            assert_eq!(data.len(), num_of_columns);
            assert_eq!(data.len(), test_columns_bytes.len());

            for (i, column_bytes) in test_columns_bytes.iter_mut().enumerate() {
                column_bytes.extend_from_slice(data.get_raw_at(i).unwrap());
            }
        }
    }

    let test_columns = test_columns_bytes
        .into_iter()
        .map(|column_bytes| VmData::new(vm_type, column_bytes).unwrap())
        .collect::<Vec<_>>();

    // Inspect db state (in case insertions succeeded as expected)
    runtime_context
        .tabledb_state
        .borrow()
        .read_table(DATASOURCE_NAME, "table1", |table| {
            assert_eq!(table.row_count, expected_row_count as u64);
            // tracing::info!("db state: {:#?}", table);
            Ok(())
        })
        .unwrap();

    // Read back and validate the columns one by one
    for (column_idx, test_column) in test_columns.into_iter().enumerate() {
        // Prepare program arguments for reading columns
        let arguments = ProgramArgumentMap::from([
            (
                "datasource_name".to_owned(),
                PA::from(convert_into_vmdata(DATASOURCE_NAME).unwrap()),
            ),
            (
                "table_name".to_owned(),
                PA::from(convert_into_vmdata("table1").unwrap()),
            ),
            (
                "column_idx".to_owned(),
                PA::from(convert_into_vmdata(column_idx as u64).unwrap()),
            ),
            (
                "column_type".to_owned(),
                PA::from(convert_into_vmdata(vm_type.type_name()).unwrap()),
            ),
            (
                "column_vlen".to_owned(),
                PA::from(convert_into_vmdata(vm_type.is_variable_length()).unwrap()),
            ),
        ]);

        // Execute a bytecode process for reading a column
        let result =
            runtime.execute_program("tests/tabledb_tests/tdb_read_col.sb", arguments.clone());

        // Inspect bytecode results
        if let Some(results) = check_expected_result(result, shall_pass) {
            if vm_type.is_variable_length() {
                assert!(!results.is_empty());
                let value_count = results
                    .get("value_count")
                    .unwrap()
                    .as_vm_data()
                    .unwrap()
                    .to_value::<u64>()
                    .unwrap();

                assert_eq!(results.len(), 1 + value_count as usize);
                assert_eq!(value_count as usize, expected_row_count);

                // Reconstruct column as bytes
                let mut column_bytes = Vec::with_capacity(expected_row_count);
                for i in 0..value_count {
                    let result_name = format!("value_{}", i);
                    let value = results.get(&result_name).unwrap().as_vm_data().unwrap();
                    column_bytes.extend_from_slice(value.as_bytes());
                }
                let column = VmDataView::new(vm_type, &column_bytes).unwrap();
                assert_eq!(test_column, column);
            } else {
                assert_eq!(results.len(), 1);
                let values = results.get("values").unwrap().as_vm_data().unwrap();
                assert_eq!(expected_row_count, values.len());
                assert_eq!(test_column, values);
            }
        }
    }
}

fn test_tdb_insert_row_fixsize_types(
    num_of_columns: usize,
    num_of_values: usize,
    value_as_column: bool,
    shall_pass: bool,
) {
    init_global_context();
    tracing::info!(
        "Testing tdb_insert_row with FIXED-SIZE TYPES: ncols={}, nvalues={}, VAC={}, shall_pass={}",
        num_of_columns,
        num_of_values,
        value_as_column,
        shall_pass
    );

    // Run tests for each column type individually

    macro_rules! test_with_type {
        ($vm_type:expr) => {
            test_tdb_insert_row_with_type(
                $vm_type,
                num_of_columns,
                num_of_values,
                value_as_column,
                shall_pass,
            );
        };
    }

    // Boolean
    test_with_type!(VmType::Bool);

    // Signed integers
    test_with_type!(VmType::Int8);
    test_with_type!(VmType::Int16);
    test_with_type!(VmType::Int32);
    test_with_type!(VmType::Int64);

    // Unsigned integers
    test_with_type!(VmType::Uint8);
    test_with_type!(VmType::Uint16);
    test_with_type!(VmType::Uint32);
    test_with_type!(VmType::Uint64);

    // Floating point numbers
    test_with_type!(VmType::Float32);
    test_with_type!(VmType::Float64);
}

fn test_tdb_insert_row_varlen_types(
    num_of_columns: usize,
    value_length: usize,
    value_as_column: bool,
    shall_pass: bool,
) {
    init_global_context();
    tracing::info!(
        "Testing tdb_insert_row with VARIABLE-LENGTH TYPES: ncols={}, value_len={}, VAC={}, shall_pass={}",
        num_of_columns,
        value_length,
        value_as_column,
        shall_pass
    );

    macro_rules! test_with_type {
        ($vm_type:expr) => {
            test_tdb_insert_row_with_type(
                $vm_type,
                num_of_columns,
                value_length,
                value_as_column,
                shall_pass,
            );
        };
    }

    test_with_type!(VmType::String);
}

//------------------------------------------------------------------------------
// tdb_insert_row: fixed-size types, value_as_column = false
//
// With fixed-size type, we can pass arrays of values of the same type.
// If VAC=false, then the number of values must match the number of columns.
//  * Inserting N values in N columns shall pass
//  * Inserting <N values in N columns shall fail
//  * Inserting >N values in N columns shall fail
//------------------------------------------------------------------------------

#[test]
fn test_tdb_insert_row_fixsize_novac_nvalues_eq_ncolumns() {
    const N: usize = 5;
    const VALUE_AS_COLUMN: bool = false;
    const SHALL_PASS: bool = true;
    test_tdb_insert_row_fixsize_types(N, N, VALUE_AS_COLUMN, SHALL_PASS);
}

#[test]
fn test_tdb_insert_row_fixsize_novac_nvalues_lt_ncolumns() {
    const N: usize = 5;
    const VALUE_AS_COLUMN: bool = false;
    const SHALL_PASS: bool = false;
    test_tdb_insert_row_fixsize_types(N, N - 1, VALUE_AS_COLUMN, SHALL_PASS);
}

#[test]
fn test_tdb_insert_row_fixsize_novac_nvalues_gt_ncolumns() {
    const N: usize = 5;
    const VALUE_AS_COLUMN: bool = false;
    const SHALL_PASS: bool = false;
    test_tdb_insert_row_fixsize_types(N, N + 1, VALUE_AS_COLUMN, SHALL_PASS);
}

//------------------------------------------------------------------------------
// tdb_insert_row: fixed-size types, value_as_column = true
//
// If VAC=true, all values go in a single column, and thus table must have only 1 column.
//  * Inserting N values in 1 column shall pass
//  * Inserting N values in 2 column shall fail
//  * Inserting 2*N values in 2 column shall fail
//------------------------------------------------------------------------------

#[test]
fn test_tdb_insert_row_fixsize_vac_nvalues_in_1column() {
    const N: usize = 5;
    const VALUE_AS_COLUMN: bool = true;
    const SHALL_PASS: bool = true;
    test_tdb_insert_row_fixsize_types(1, N, VALUE_AS_COLUMN, SHALL_PASS);
}

#[test]
fn test_tdb_insert_row_fixsize_vac_nvalues_in_2columns() {
    const N: usize = 5;
    const VALUE_AS_COLUMN: bool = true;
    const SHALL_PASS: bool = false;
    test_tdb_insert_row_fixsize_types(2, N, VALUE_AS_COLUMN, SHALL_PASS);
}

#[test]
fn test_tdb_insert_row_fixsize_vac_2nvalues_in_2columns() {
    const N: usize = 5;
    const VALUE_AS_COLUMN: bool = true;
    const SHALL_PASS: bool = false;
    test_tdb_insert_row_fixsize_types(2, N * 2, VALUE_AS_COLUMN, SHALL_PASS);
}

//------------------------------------------------------------------------------
// tdb_insert_row: variable-length types
//
// With variable-length data, there is only a single value to insert
// and table must consist of one column. VAC doesn't affect anything.
//  * Inserting 1 value in 1 column shall pass
//  * Inserting 1 value in 2 columns shall fail
//------------------------------------------------------------------------------

#[test]
fn test_tdb_insert_row_varlen_novac_1columns() {
    const N: usize = 5;
    const VALUE_AS_COLUMN: bool = false;
    const SHALL_PASS: bool = true;
    test_tdb_insert_row_varlen_types(1, N, VALUE_AS_COLUMN, SHALL_PASS);
}

#[test]
fn test_tdb_insert_row_varlen_vac_1columns() {
    const N: usize = 5;
    const VALUE_AS_COLUMN: bool = true;
    const SHALL_PASS: bool = true;
    test_tdb_insert_row_varlen_types(1, N, VALUE_AS_COLUMN, SHALL_PASS);
}

#[test]
fn test_tdb_insert_row_varlen_novac_2columns() {
    const N: usize = 5;
    const VALUE_AS_COLUMN: bool = false;
    const SHALL_PASS: bool = false;
    test_tdb_insert_row_varlen_types(2, N, VALUE_AS_COLUMN, SHALL_PASS);
}

#[test]
fn test_tdb_insert_row_varlen_vac_2columns() {
    const N: usize = 5;
    const VALUE_AS_COLUMN: bool = true;
    const SHALL_PASS: bool = false;
    test_tdb_insert_row_varlen_types(2, N, VALUE_AS_COLUMN, SHALL_PASS);
}

//------------------------------------------------------------------------------
// Boilerplate for tdb_insert_row2 tests
//------------------------------------------------------------------------------

#[derive(Debug)]
struct SegmentColumn {
    tdb_type: TdbType,
    vm_type: VmType,
    rows: Vec<VmData>, // variable-length or fixed size data cells
}

/// A table segment in column-major format
#[derive(Debug)]
struct Segment {
    columns: Vec<SegmentColumn>,
}

impl Segment {
    fn row_count(&self) -> Option<usize> {
        self.columns.first().map(|col| col.rows.len())
    }
}

fn generate_table_segment(col_types: &[TdbType], row_count: usize) -> Segment {
    let mut rng = rand::rng();

    // Let variable-length values be between 0 and 20 elements long
    let vlen_range = rand::distr::Uniform::new(0, 21).unwrap();

    // We want at least one vlen column to have at least one empty value
    let mut vlen_empty_value_generated = false;

    let columns = col_types
        .iter()
        .map(|col_type| {
            let vm_type = VmType::from_name(&col_type.name).expect("Unsupported VM data type");

            // Select random row in the first vlen column that will have empty value
            let empty_vlen_value_idx =
                if !vlen_empty_value_generated && col_type.is_variable_length() {
                    rng.random_range(..row_count)
                } else {
                    0
                };
            // Generate rows
            let rows = (0..row_count)
                .map(|row_idx| {
                    let count = if col_type.is_variable_length() {
                        // enforce at least one empty vlen value in the first vlen column
                        if !vlen_empty_value_generated && row_idx == empty_vlen_value_idx {
                            vlen_empty_value_generated = true;
                            0
                        } else {
                            rng.sample(vlen_range)
                        }
                    } else {
                        1
                    };
                    VmData::random(vm_type, count)
                })
                .collect();
            SegmentColumn {
                tdb_type: col_type.clone(),
                vm_type,
                rows,
            }
        })
        .collect();

    Segment { columns }
}

fn generate_table_segments(
    col_types: &[TdbType],
    row_count: usize,
    segment_count: usize,
) -> Vec<Segment> {
    (0..segment_count)
        .map(|_| generate_table_segment(col_types, row_count))
        .collect()
}

#[derive(Debug)]
struct ValueObject {
    data: VmData,
    is_variable_length: bool,
}

#[derive(Debug)]
struct ValueBatch {
    value_objects: Vec<ValueObject>, // these correspond to SharemindTdbValue objects
    value_as_column: bool,
}

/// Generate value batches given the input segment containing the values in
/// individual table cells.
///
/// When multiple adjacent columns have equal fixed-size type, then the values
/// of these adjacent columns are merged row-wise into separate value objects.
/// By defining which adjacent columns have the same fixed-size type we can
/// control how segment row values are distributed among value objects in batches.
/// This allows us to test various distributions of values in batches.
fn segment_to_batches(segment: &Segment, value_as_column: bool) -> Vec<ValueBatch> {
    if segment.columns.is_empty() {
        return Vec::new();
    }

    let has_vlen_columns = segment
        .columns
        .iter()
        .any(|c| c.tdb_type.is_variable_length());
    let row_count = {
        assert!(!segment.columns.is_empty());
        let row_count_first = segment.columns.first().unwrap().rows.len();
        assert!(
            segment
                .columns
                .iter()
                .all(|c| c.rows.len() == row_count_first),
            "inconsistent row count in columns"
        );
        row_count_first
    };

    if !has_vlen_columns && value_as_column {
        // Encode multiple segment rows per batch, when
        // all columns are scalar and VAC is true.

        // Every column becomes a single value object.
        let value_objects = segment
            .columns
            .iter()
            .map(|column| {
                // Merge the column cells
                let merged_bytes = column
                    .rows
                    .iter()
                    .flat_map(|cell| cell.as_bytes().iter().cloned())
                    .collect::<Vec<_>>();
                let data = VmData::new(column.vm_type, merged_bytes)
                    .expect("Merging column cells should succeed");
                ValueObject {
                    data,
                    is_variable_length: column.tdb_type.is_variable_length(),
                }
            })
            .collect();
        vec![ValueBatch {
            value_objects,
            value_as_column,
        }]
    } else {
        // Encode every segment row as a separate batch when either
        //  * at least one column is variable-length, or
        //  * all columns are scalar and VAC is false.
        assert!(has_vlen_columns || !value_as_column);

        // Iterate over rows
        (0..row_count)
            .map(|i| {
                // Convert the next segment row into a batch.
                // Merge cells of two adjacent fixed-size columns
                // into one value object, if columns have the same (public) type.

                let mut is_last_col_varlen = false;
                let value_objects =
                    segment
                        .columns
                        .iter()
                        .fold(Vec::<ValueObject>::new(), |mut acc, column| {
                            let cell_data =
                                column.rows.get(i).expect("There should be row_count rows");

                            if let Some(last) = acc.last_mut() {
                                if last.data.vm_type() == column.vm_type
                                    && !is_last_col_varlen
                                    && !column.tdb_type.is_variable_length()
                                {
                                    last.data.extend_from_bytes(cell_data.as_bytes()).unwrap();
                                    return acc;
                                }
                            }

                            is_last_col_varlen = column.tdb_type.is_variable_length();
                            acc.push(ValueObject {
                                data: cell_data.clone(),
                                is_variable_length: column.tdb_type.is_variable_length(),
                            });
                            acc
                        });

                ValueBatch {
                    value_objects,
                    value_as_column,
                }
            })
            .collect()
    }
}

fn segments_to_batches(segments: &[Segment], value_as_column: bool) -> Vec<Vec<ValueBatch>> {
    segments
        .iter()
        .map(|segment| segment_to_batches(segment, value_as_column))
        .collect()
}

fn total_batch_count(value_batches: &[Vec<ValueBatch>]) -> usize {
    value_batches.iter().map(|batch| batch.len()).sum()
}

// Make a copy of column types with some column changed to a different type.
fn copy_columns_types_and_change_one(col_types: &[TdbType]) -> Vec<TdbType> {
    let mut rng = rand::rng();
    let mut new_col_types = col_types.to_owned(); // copy
    let (idx, random_col_type) = new_col_types
        .iter_mut()
        .enumerate()
        .choose(&mut rng)
        .expect("at least one column must exist");

    let current_type = VmType::from_name(&random_col_type.name).unwrap();
    let different_type = VmType::iter()
        .filter(|&t| t != current_type)
        .choose(&mut rng)
        .expect("There should be at least one alternative VM type");

    *random_col_type = new_public_column_type(different_type);
    assert_ne!(col_types[idx], new_col_types[idx]);
    new_col_types
}

/// Test the tdb_insert_row2 syscall with a specific set of input data and parameters.
///
/// The main steps performed:
///   1. Create the table according to the provided columns schema.
///   2. Pass the batches to the tdb_insert_row2 syscall via a SecreC script for
///      insertion and validate the table state (not the inserted data values).
///   3. Read the columns back from the table via a SecreC script and compare
///      them to the original input segment data.
///   4. Fail at step 2 or 3 if shall_pass is false.
fn test_tdb_insert_row2(
    table_col_types: &[TdbType],
    segments: &[Segment],
    value_batches: Vec<Vec<ValueBatch>>,
    shall_pass: bool,
    description: &str,
) {
    let mut test_context = TestContext::new();

    // Setup the runtime
    let (mut runtime, runtime_context) = test_context.new_runtime().unwrap();

    tracing::info!("Testing tdb_insert_row2: {}", description);

    // Validate inputs
    assert!(
        !table_col_types.is_empty(),
        "no table column types provided"
    );
    assert!(!segments.is_empty(), "no input segments provided");
    assert!(
        segments
            .windows(2)
            .all(|w| w[0].columns.len() == w[1].columns.len()),
        "provided segments have different column counts"
    );

    // Setup the mock database state
    // Requirement: All columns are named by their index starting with 0.

    // Create enumerated table from types
    let table1 = Table::from_vec(
        table_col_types
            .iter()
            .enumerate()
            .map(|(i, type_)| Column::new(i.to_string().as_str(), type_.clone()))
            .collect(),
    )
    .unwrap();

    runtime_context
        .tabledb_state
        .borrow_mut()
        .modify_datasource_tables(DATASOURCE_NAME, true, |ds_tables| {
            assert!(ds_tables.is_empty());
            assert_eq!(ds_tables.insert("table1".to_owned(), table1), None);
            assert!(ds_tables.contains_key("table1"));
            Ok(())
        })
        .unwrap();

    // Insert value batches for each segment separately
    for segment_value_batches in value_batches.into_iter() {
        // Prepare program arguments for inserting segment rows
        let mut arguments = ProgramArgumentMap::from([
            (
                "datasource_name".to_owned(),
                PA::from(convert_into_vmdata(DATASOURCE_NAME).unwrap()),
            ),
            (
                "table_name".to_owned(),
                PA::from(convert_into_vmdata("table1").unwrap()),
            ),
        ]);

        let mut value_object_counter: u64 = 0;
        for (batch_idx, batch) in segment_value_batches.into_iter().enumerate() {
            for value_object in batch.value_objects {
                assert!(arguments
                    .insert(
                        format!("batch_id_{}", value_object_counter),
                        PA::from(convert_into_vmdata(batch_idx as u64).unwrap())
                    )
                    .is_none());

                assert!(arguments
                    .insert(
                        format!("batch_vac_{}", value_object_counter),
                        PA::from(convert_into_vmdata(batch.value_as_column).unwrap())
                    )
                    .is_none());

                assert!(arguments
                    .insert(
                        format!("value_type_{}", value_object_counter),
                        PA::from(
                            convert_into_vmdata(value_object.data.vm_type().type_name()).unwrap()
                        )
                    )
                    .is_none());

                assert!(arguments
                    .insert(
                        format!("value_data_{}", value_object_counter),
                        PA::from(value_object.data)
                    )
                    .is_none());

                assert!(arguments
                    .insert(
                        format!("value_vlen_{}", value_object_counter),
                        PA::from(convert_into_vmdata(value_object.is_variable_length).unwrap())
                    )
                    .is_none());

                value_object_counter += 1;
            }
        }

        assert!(arguments
            .insert(
                "value_object_count".to_owned(),
                PA::from(convert_into_vmdata(value_object_counter).unwrap())
            )
            .is_none());

        // Execute a bytecode process for inserting data
        let result = runtime.execute_program("tests/tabledb_tests/tdb_insert_row2.sb", arguments);

        // Inspect bytecode results
        if let Some(results) = check_expected_result(result, shall_pass) {
            assert!(results.is_empty()); // nothing is published
        } else {
            return; // Expected failure
        }
    }

    // Inspect db state (in case insertions succeeded as expected)
    let expected_row_count: usize = segments
        .iter()
        .map(|segment| segment.row_count().unwrap())
        .sum();

    runtime_context
        .tabledb_state
        .borrow()
        .read_table(DATASOURCE_NAME, "table1", |table| {
            assert_eq!(table.row_count, expected_row_count as u64);
            Ok(())
        })
        .unwrap();

    for segment in segments {
        // At this point we expect segment columns to be consistent with table schema
        assert!(
            segment.columns.len() == table_col_types.len(),
            "segments have different number of columns than the table"
        );

        assert!(
            segment
                .columns
                .iter()
                .zip(table_col_types)
                .all(|(segment_col, table_col_type)| segment_col.tdb_type == *table_col_type),
            "segment columns do not match the table schema"
        );
    }

    // Read back and validate the columns one by one
    for (column_idx, column_type) in table_col_types.iter().enumerate() {
        // Prepare program arguments for reading columns
        let arguments = ProgramArgumentMap::from([
            (
                "datasource_name".to_owned(),
                PA::from(convert_into_vmdata(DATASOURCE_NAME).unwrap()),
            ),
            (
                "table_name".to_owned(),
                PA::from(convert_into_vmdata("table1").unwrap()),
            ),
            (
                "column_idx".to_owned(),
                PA::from(convert_into_vmdata(column_idx as u64).unwrap()),
            ),
            (
                "column_type".to_owned(),
                PA::from(convert_into_vmdata(column_type.name.as_str()).unwrap()),
            ),
            (
                "column_vlen".to_owned(),
                PA::from(convert_into_vmdata(column_type.is_variable_length()).unwrap()),
            ),
        ]);

        // Execute a bytecode process for reading a column
        let result =
            runtime.execute_program("tests/tabledb_tests/tdb_read_col.sb", arguments.clone());

        // Inspect bytecode results
        if let Some(results) = check_expected_result(result, shall_pass) {
            if column_type.is_variable_length() {
                assert!(!results.is_empty());
                let value_count = results
                    .get("value_count")
                    .unwrap()
                    .as_vm_data()
                    .unwrap()
                    .to_value::<u64>()
                    .unwrap();

                assert_eq!(results.len(), 1 + value_count as usize);
                assert_eq!(value_count as usize, expected_row_count);

                // Compare result values to original column values in segments
                for (row_idx, orig_value) in segments
                    .iter()
                    .flat_map(|segment| &segment.columns[column_idx].rows)
                    .enumerate()
                {
                    let read_value = results
                        .get(&format!("value_{}", row_idx))
                        .unwrap()
                        .as_vm_data()
                        .unwrap();
                    assert_eq!(*orig_value, read_value);
                }
            } else {
                assert_eq!(results.len(), 1);
                let values = results.get("values").unwrap().as_vm_data().unwrap();
                assert_eq!(values.len(), expected_row_count);

                // Compare result values to original column values in segments (as raw bytes)
                for (row_idx, orig_value) in segments
                    .iter()
                    .flat_map(|segment| &segment.columns[column_idx].rows)
                    .enumerate()
                {
                    let read_value = values.get_raw_at(row_idx).unwrap();
                    assert_eq!(orig_value.vm_type(), values.vm_type());
                    assert_eq!(orig_value.as_bytes(), read_value);
                }
            }
        }
    }
}

/// A template for testing the tdb_insert_row2 syscall.
///
/// It generates input data segments based on the provided segment column types
/// and some hardcoded parameters like segment and row count, and then
/// converts them to value batches. The table schema, the generated input data,
/// and the additional parameters tested using the test_tdb_insert_row2() method.
///
/// Separate table and segment column types parameters enable testing different
/// input column types and counts wrt. the table schema.
/// The value-as-column flag specifies the value mode in the generated value batches.
/// The shall-pass flag specifies whether the test is expected to pass or fail given the input data.
/// The data hook method is called on the generated value batches to validate them
/// or tailor them for specific test scenario before the insertion takes place.
fn test_tdb_insert_row2_template<F>(
    description: &str,
    table_col_types: &[TdbType],
    segment_col_types: &[TdbType],
    value_as_column: bool,
    shall_pass: bool,
    data_hook: F,
) where
    F: Fn(&mut Vec<Vec<ValueBatch>>, usize, usize),
{
    // Each segment will be inserted with a separate tdb_insert_row2 call
    let segment_counts = [1, 2];
    let row_counts = [1, 5];

    for segment_count in segment_counts {
        for row_count in row_counts {
            // Generate test data
            let segments = generate_table_segments(segment_col_types, row_count, segment_count);
            let mut value_batches = segments_to_batches(&segments, value_as_column);

            data_hook(&mut value_batches, segment_count, row_count);

            // Run test
            let description = format!(
                "{description} - {} column table, {} segments ({} cols, {} rows)",
                table_col_types.len(),
                segment_count,
                segment_col_types.len(),
                row_count
            );
            test_tdb_insert_row2(
                table_col_types,
                &segments,
                value_batches,
                shall_pass,
                &description,
            );
        }
    }
}

//------------------------------------------------------------------------------
// Tests for row insertion using vector maps (tdb_insert_row2)
//------------------------------------------------------------------------------
// Test the insertion of S rectangular table segments (M columns x N rows) into
// a table with K columns. The table columns can have any combination of types,
// both fixed-size and variable-length. The segment cells can contain either
// fixed-size or variable-length data, but segment column types shall match the
// table column types. The segments shall be encoded in vector maps.
// For details on how a vector map translates into table segments please refer
// to the `tdb_insert_row2` syscall documentation.
//
// Parameters to test (where reasonable):
//  Number of segments:             S = {0, 1, 2}
//  Number of columns in a segment: M = {0, K, K-1, K+1}
//  Number of rows in a segment:    N = {0, 1, 5}
// In general tests with S, M or N equal to 0 in any test set shall not pass (empty input).
//------------------------------------------------------------------------------

//------------------------------------------------------------------------------
// tdb_insert_row2: fixed-size data, value-as-column is true
//
// Note:
//  Each segment is encoded as a separate batch.
// Table schema:
//  * K columns with different fixed-size types.
// Tests:
//  * BASELINE: Insert S batches with K value objects (one per column),
//              each containing N values (rows). Shall pass.
//  * Same as BASELINE, but value objects have wrong type. Shall not pass.
//  * Same as BASELINE, but one value object contains N+1 values. Shall not pass (inconsistent number of rows)
//  * Same as BASELINE, but batches have <K value objects. Shall not pass (too few columns).
//  * Same as BASELINE, but batches have >K value objects. Shall not pass (too many columns).
//------------------------------------------------------------------------------

// TODO: separately test S = M = N = 0
//       However, testing 0 batches isnt possible, because new VMAP always contains at least one empty batch.

fn different_fixed_size_columns() -> Vec<TdbType> {
    vec![
        new_public_column_type(VmType::Bool),
        new_public_column_type(VmType::Int8),
        new_public_column_type(VmType::Int16),
        new_public_column_type(VmType::Int32),
        new_public_column_type(VmType::Int64),
        new_public_column_type(VmType::Uint8),
        new_public_column_type(VmType::Uint16),
        new_public_column_type(VmType::Uint32),
        new_public_column_type(VmType::Uint64),
        new_public_column_type(VmType::Float32),
        new_public_column_type(VmType::Float64),
    ]
}

#[test]
fn test_tdb_insert_row2_fixsize_vac_baseline() {
    const DESCRIPTION: &str = "fixsize vac - baseline";
    const VALUE_AS_COLUMN: bool = true;
    const SHALL_PASS: bool = true;

    let table_col_types = different_fixed_size_columns();

    test_tdb_insert_row2_template(
        DESCRIPTION,
        &table_col_types,
        &table_col_types,
        VALUE_AS_COLUMN,
        SHALL_PASS,
        |value_batches, segment_count, _row_count| {
            assert_eq!(
                total_batch_count(value_batches),
                segment_count,
                "with VAC=true each input segment should become a separate batch"
            );
        },
    );
}

#[test]
fn test_tdb_insert_row2_fixsize_vac_values_wrong_type() {
    const DESCRIPTION: &str = "fixsize vac - wrong input types";
    const VALUE_AS_COLUMN: bool = true;
    const SHALL_PASS: bool = false;

    let table_col_types = different_fixed_size_columns();

    // This will be used to generate column data of wrong type
    let segment_col_types = copy_columns_types_and_change_one(&table_col_types);

    test_tdb_insert_row2_template(
        DESCRIPTION,
        &table_col_types,
        &segment_col_types,
        VALUE_AS_COLUMN,
        SHALL_PASS,
        |value_batches, segment_count, _row_count| {
            assert_eq!(
                total_batch_count(value_batches),
                segment_count,
                "with VAC=true each input segment should become a separate batch"
            );
        },
    );
}

#[test]
fn test_tdb_insert_row2_fixsize_vac_inconsistent_row_counts() {
    const VALUE_AS_COLUMN: bool = true;
    const SHALL_PASS: bool = false;

    let table_col_types = different_fixed_size_columns();

    let bad_col_idx = table_col_types.len() - 1;
    let desctiption = format!(
        "fixsize vac - inconsistent input row counts (col {} has 1 extra row)",
        bad_col_idx + 1,
    );

    test_tdb_insert_row2_template(
        &desctiption,
        &table_col_types,
        &table_col_types,
        VALUE_AS_COLUMN,
        SHALL_PASS,
        |value_batches, segment_count, _row_count| {
            assert_eq!(
                total_batch_count(value_batches),
                segment_count,
                "with VAC=true each input segment should become a separate batch"
            );

            // One column in each batch has wrong row count
            for batch in value_batches.iter_mut().flatten() {
                assert_eq!(
                    batch.value_objects.len(),
                    table_col_types.len(),
                    "with VAC=true batches should have one value object per column"
                );

                // Add extra row cell to one column
                let bad_column = batch.value_objects.get_mut(bad_col_idx).unwrap();
                let new_cell = VmData::random(bad_column.data.vm_type(), 1);
                bad_column
                    .data
                    .extend_from_bytes(new_cell.as_bytes())
                    .unwrap();
            }
        },
    );
}

#[test]
fn test_tdb_insert_row2_fixsize_vac_wrong_column_count() {
    const DESCRIPTION: &str = "fixsize vac - wrong input column count";
    const VALUE_AS_COLUMN: bool = true;
    const SHALL_PASS: bool = false;

    let table_col_types_full = {
        let mut col_types = different_fixed_size_columns();
        // Add extra column for testing K+1 columns in data
        col_types.push(new_public_column_type(VmType::Bool));
        col_types
    };

    // K columns for the table schema
    let k = table_col_types_full.len() - 1;
    let table_col_types_k = &table_col_types_full[..k];

    for col_count in [0, k - 1, k + 1] {
        let segment_col_types = &table_col_types_full[..col_count];

        test_tdb_insert_row2_template(
            DESCRIPTION,
            table_col_types_k,
            segment_col_types,
            VALUE_AS_COLUMN,
            SHALL_PASS,
            |value_batches, segment_count, _row_count| {
                // With zero input columns no batches or values are added to the vmap,
                // although in SecreC a new empty vmap already contains the first empty batch.
                if col_count > 0 {
                    assert_eq!(
                        total_batch_count(value_batches),
                        segment_count,
                        "with VAC=true each input segment should become a separate batch"
                    );
                }
            },
        );
    }
}

//------------------------------------------------------------------------------
// tdb_insert_row2: fixed-size data, value-as-column is false
//
// TEST SET 1: Batches that cover a single row.
//  Note:
//   Each row in a segment is encoded as a separate batch.
//   Test that all value objects in a batch combined cover all table columns to
//   create a single row. Value objects contain vector data (scalar if vector
//   size is 1), where each element is intended for one separate column.
//  Table schema:
//   * K columns with different fixed-size types, where some consecutive columns have the same type.
//  Tests:
//   * BASELINE: Insert S*N batches (one per row) with R value objects, each
//               object containing len(R_i) values that cover the next len(R_i)
//               consecutive columns of the same type, so that SUM(len(R_i))=K.
//               Shall pass.
//   * Same as BASELINE, but value object types do not match the column types. Shall not pass.
//   * Same as BASELINE, but SUM(len(R_i)) < K. Shall not pass (too few column values)
//   * Same as BASELINE, but SUM(len(R_i)) > K. Shall not pass (too many column values)
//------------------------------------------------------------------------------

fn different_fixed_size_columns_some_adjacent_same() -> Vec<TdbType> {
    vec![
        new_public_column_type(VmType::Bool),
        new_public_column_type(VmType::Int8),
        new_public_column_type(VmType::Int16),
        new_public_column_type(VmType::Int16),
        new_public_column_type(VmType::Int32),
        new_public_column_type(VmType::Int64),
        new_public_column_type(VmType::Uint8),
        new_public_column_type(VmType::Uint16),
        new_public_column_type(VmType::Uint32),
        new_public_column_type(VmType::Uint64),
        new_public_column_type(VmType::Float32),
        new_public_column_type(VmType::Float32),
        new_public_column_type(VmType::Float32),
        new_public_column_type(VmType::Float64),
    ]
}

#[test]
fn test_tdb_insert_row2_fixsize_novac_baseline_different_columns() {
    const DESCRIPTION: &str = "fixsize novac - baseline with different columns";
    const VALUE_AS_COLUMN: bool = false;
    const SHALL_PASS: bool = true;

    let table_col_types = different_fixed_size_columns_some_adjacent_same();

    test_tdb_insert_row2_template(
        DESCRIPTION,
        &table_col_types,
        &table_col_types,
        VALUE_AS_COLUMN,
        SHALL_PASS,
        |value_batches, segment_count, row_count| {
            assert_eq!(
                total_batch_count(value_batches),
                segment_count * row_count,
                "with VAC=false each input row should become a separate batch"
            );
        },
    );
}

#[test]
fn test_tdb_insert_row2_fixsize_novac_values_wrong_type() {
    const DESCRIPTION: &str = "fixsize novac - wrong input type";
    const VALUE_AS_COLUMN: bool = false;
    const SHALL_PASS: bool = false;

    let table_col_types = different_fixed_size_columns_some_adjacent_same();

    // This will be used to generate column data of wrong type
    let segment_col_types = copy_columns_types_and_change_one(&table_col_types);

    test_tdb_insert_row2_template(
        DESCRIPTION,
        &table_col_types,
        &segment_col_types,
        VALUE_AS_COLUMN,
        SHALL_PASS,
        |value_batches, segment_count, row_count| {
            assert_eq!(
                total_batch_count(value_batches),
                segment_count * row_count,
                "with VAC=false each input row should become a separate batch"
            );
        },
    );
}

#[test]
fn test_tdb_insert_row2_fixsize_novac_wrong_column_count() {
    const DESCRIPTION: &str = "fixsize novac - wrong input column count";
    const VALUE_AS_COLUMN: bool = false;
    const SHALL_PASS: bool = false;

    let table_col_types_full = {
        let mut col_types = different_fixed_size_columns_some_adjacent_same();
        // Add extra column for testing K+1 columns in data
        col_types.push(new_public_column_type(VmType::Bool));
        col_types
    };

    // K columns for the table schema
    let k = table_col_types_full.len() - 1;
    let table_col_types_k = &table_col_types_full[..k];

    for col_count in [0, k - 1, k + 1] {
        let segment_col_types = &table_col_types_full[..col_count];

        test_tdb_insert_row2_template(
            DESCRIPTION,
            table_col_types_k,
            segment_col_types,
            VALUE_AS_COLUMN,
            SHALL_PASS,
            |value_batches, segment_count, row_count| {
                // With zero input columns no batches or values are added to the vmap,
                // although in SecreC a new empty vmap already contains the first empty batch.
                if col_count > 0 {
                    assert_eq!(
                        total_batch_count(value_batches),
                        segment_count * row_count,
                        "with VAC=false each input row should become a separate batch"
                    );
                }
            },
        );
    }
}

//------------------------------------------------------------------------------
// tdb_insert_row2: fixed-size data, value-as-column is false
//
// TEST SET 2: Vectors that cover a single row
//  Note: Special case of TEST SET 1, where all columns have the same type
//  Table schema:
//   * K columns with same fixed-size types
//  Tests:
//   Same as TEST SET 1, but BASELINE test has 1 value object per batch, each containing len(R_i)=K values.
//------------------------------------------------------------------------------

fn ten_equal_fixed_size_columns() -> Vec<TdbType> {
    vec![new_public_column_type(VmType::Uint16); 10]
}

#[test]
fn test_tdb_insert_row2_fixsize_novac_baseline_equal_columns() {
    const DESCRIPTION: &str = "fixsize novac - baseline with equal columns";
    const VALUE_AS_COLUMN: bool = false;
    const SHALL_PASS: bool = true;

    let table_col_types = ten_equal_fixed_size_columns();

    test_tdb_insert_row2_template(
        DESCRIPTION,
        &table_col_types,
        &table_col_types,
        VALUE_AS_COLUMN,
        SHALL_PASS,
        |value_batches, segment_count, row_count| {
            assert_eq!(
                total_batch_count(value_batches),
                segment_count * row_count,
                "with VAC=false each input row should become a separate batch"
            );

            assert!(
                value_batches
                    .iter()
                    .flatten()
                    .all(|b| b.value_objects.len() == 1),
                "with VAC=false and all columns equal, the batches should contain 1 value object"
            );
        },
    );
}

//------------------------------------------------------------------------------
// tdb_insert_row2: variable-length data
//
// Note:
//  Each row in a segment is encoded as a separate batch.
// Table schemas:
//  * K columns with all variable-length column types
//  * K columns with mixed variable-length and fixed-size column types
// Tests:
//  * BASELINE: Insert S*N batches (one per row) with K value objects (one per column),
//              each containing one variable-length value (or fixed-size scalar value, if applicable).
//              Shall pass.
//  * Same as BASELINE, but batches have <K value objects. Shall not pass (too few columns).
//  * Same as BASELINE, but batches have >K value objects. Shall not pass (too many columns).
//------------------------------------------------------------------------------

fn different_varlen_column_types() -> Vec<TdbType> {
    vec![
        new_public_vlen_column_type(VmType::Bool),
        new_public_vlen_column_type(VmType::Int8),
        new_public_vlen_column_type(VmType::Int16),
        new_public_vlen_column_type(VmType::Int32),
        new_public_vlen_column_type(VmType::Int64),
        new_public_vlen_column_type(VmType::Uint8),
        new_public_vlen_column_type(VmType::Uint16),
        new_public_vlen_column_type(VmType::Uint32),
        new_public_vlen_column_type(VmType::Uint64),
        new_public_vlen_column_type(VmType::Float32),
        new_public_vlen_column_type(VmType::Float64),
        new_public_vlen_column_type(VmType::String),
    ]
}

fn mixed_varlen_fixsize_column_types() -> Vec<TdbType> {
    vec![
        new_public_column_type(VmType::Bool),
        new_public_vlen_column_type(VmType::Int8),
        new_public_vlen_column_type(VmType::Int16),
        new_public_column_type(VmType::Int32),
        new_public_vlen_column_type(VmType::Int64),
        new_public_column_type(VmType::Uint8),
        new_public_vlen_column_type(VmType::Uint16),
        new_public_vlen_column_type(VmType::Uint32),
        new_public_column_type(VmType::Uint64),
        new_public_column_type(VmType::Float32),
        new_public_vlen_column_type(VmType::Float64),
        new_public_vlen_column_type(VmType::String),
    ]
}

#[test]
fn test_tdb_insert_row2_varlen_baseline_different_varlen_columns() {
    const DESCRIPTION: &str = "varlen - baseline with different varlen columns";
    const VALUE_AS_COLUMN: bool = false;
    const SHALL_PASS: bool = true;

    let table_col_types = different_varlen_column_types();

    test_tdb_insert_row2_template(
        DESCRIPTION,
        &table_col_types,
        &table_col_types,
        VALUE_AS_COLUMN,
        SHALL_PASS,
        |value_batches, segment_count, row_count| {
            assert_eq!(
                total_batch_count(value_batches),
                segment_count * row_count,
                "with variable-length columns each input row should become a separate batch"
            );
        },
    );
}

#[test]
fn test_tdb_insert_row2_varlen_baseline_mixed_varlen_fixsize_columns() {
    const DESCRIPTION: &str = "varlen - baseline with mixed varlen and fixsize columns";
    const VALUE_AS_COLUMN: bool = false;
    const SHALL_PASS: bool = true;

    let table_col_types = mixed_varlen_fixsize_column_types();

    test_tdb_insert_row2_template(
        DESCRIPTION,
        &table_col_types,
        &table_col_types,
        VALUE_AS_COLUMN,
        SHALL_PASS,
        |value_batches, segment_count, row_count| {
            assert_eq!(
                total_batch_count(value_batches),
                segment_count * row_count,
                "with variable-length columns each input row should become a separate batch"
            );
        },
    );
}

#[test]
fn test_tdb_insert_row2_varlen_wrong_column_count() {
    const DESCRIPTION: &str = "varlen - wrong input column count";
    const VALUE_AS_COLUMN: bool = false;
    const SHALL_PASS: bool = false;

    let table_col_types_full = {
        let mut col_types = different_varlen_column_types();
        // Add extra column for testing K+1 columns in data
        col_types.push(new_public_column_type(VmType::Bool));
        col_types
    };

    // K columns for the table schema
    let k = table_col_types_full.len() - 1;
    let table_col_types_k = &table_col_types_full[..k];

    for col_count in [0, k - 1, k + 1] {
        let segment_col_types = &table_col_types_full[..col_count];

        test_tdb_insert_row2_template(
            DESCRIPTION,
            table_col_types_k,
            segment_col_types,
            VALUE_AS_COLUMN,
            SHALL_PASS,
            |value_batches, segment_count, row_count| {
                // With zero input columns no batches or values are added to the vmap,
                // although in SecreC a new empty vmap already contains the first empty batch.
                if col_count > 0 {
                    assert_eq!(
                        total_batch_count(value_batches),
                        segment_count * row_count,
                        "with variable-length columns each input row should become a separate batch"
                    );
                }
            },
        );
    }
}
