#
# Copyright (C) 2026 Cybernetica
#
# Research/Commercial License Usage
# Licensees holding a valid Research License or Commercial License
# for the Software may use this file according to the written
# agreement between you and Cybernetica.
#
# GNU General Public License Usage
# Alternatively, this file may be used under the terms of the GNU
# General Public License version 3.0 as published by the Free Software
# Foundation and appearing in the file LICENSE included in the
# packaging of this file.  Please review the following information to
# ensure the GNU General Public License version 3.0 requirements will be
# met: http://www.gnu.org/copyleft/gpl-3.0.html.
#
# For further information, please contact us at sharemind@cyber.ee.
#

@0xf9615f74cb5757f0;

struct TableMetadata {
  struct Schema {
    struct Column {
      struct Type {
        domain @0 :Text;
        name @1 :Text;
        size @2 :UInt64;
      }
      name @0 :Text;
      type @1 :Type;
    }
    columns @0 :List(Column);
  }
  schema @0 :Schema;
  rowCount @1 :UInt64;
}

struct Table {
  name @0 :Text;
  datasource @1 :Text;
}

struct ColumnData {
  values @0 :List(Value);
  struct Value {
    buffer @0 :Data;
  }
}

struct BoolValue {
  value @0 :Bool;
}

interface Database {

  struct Result(T) {
    # All responses are wrapped in a result. It shouldn't be necessary to
    # provide the runtime with any more information about the specific error
    # reason.
    status :union {
      statusOk @0 :T;
      statusErr @1 :Void;
    }
  }

  write @0 (table :Table, columns :List(ColumnData), rowsAddedCount :UInt64)
        -> (result :Result);
  # Writing a rectangular table segment.
  # Corresponds to syscalls:
  # - `tdb_insert_row`
  # - `tdb_insert_row2`
  # The `columns` parameter is a list of structs, each corresponding to the data
  # that is meant to be written into a column. The contents of these structs
  # shouldn't concern Ephemeral, only the Runtime. This means that Ephemeral
  # could just repackage each `ColumnData` struct into a Cap'n Proto message and
  # pass them to Sahver. See the stream experiment's server component for hints
  # on how to do that.
  # No schema is provided on write. A table has to exist beforehand.

  read @1 (table :Table, columnIdx :UInt64)
       -> (result :Result(List(ColumnData)));
  # Reading a column from a table. Corresponds to syscall:
  # - `tdb_read_col`
  # The result is a list of `ColumnData` structs, but this time it corresponds
  # to a single column -- more precisely, the list contents are the individual
  # blobs written into that table with `Database.write`.
  # At first, let's only support reading from a `columnIdx` since Sahver does not
  # implement reading by column name (yet). If we also want reading by the column's
  # name, then it's possible to either add the respective endpoint to Sahver, or
  # make a call to `getTableMetadata` before and extract the index from there.

  createTable @2 (table :Table, schema :TableMetadata.Schema)
              -> (result :Result);
  # Creating a table based on a schema, table's name and datasource name.
  # Corresponds to syscall:
  # - `tdb_tbl_create`

  deleteTable @3 (table :Table)
              -> (result :Result);
  # Deleting a table based on table's name and datasource name. Corresponds to
  # syscall:
  # - `tdb_tbl_delete`

  getTableNames @4 (datasource :Text)
                -> (result :Result(List(Text)));
  # Listing existing tables in a datasource. Corresponds to syscall:
  # - `tdb_tbl_names`

  getTableExists @5 (table :Table)
                 -> (result :Result(BoolValue));
  # Checking if a table exists based on table's name and datasource name.
  # Corresponds to syscall:
  # - `tdb_tbl_exists`

  getTableMetadata @6 (table :Table)
                   -> (result :Result(TableMetadata));
  # Fetching a table's metadata based on table's name and datasource name.
  # This combines many syscalls into one for conciseness. It is also implemented
  # in the Sahver API as one call. It's okay if the runtime calls this multiple
  # times for each field. It shouldn't cache this response for subsequent
  # 'metadata' syscalls.
  # Corresponds to syscall:
  # - `tdb_tbl_col_count`
  # - `tdb_tbl_col_names`
  # - `tdb_tbl_col_types`
  # - `tdb_tbl_row_count`

}
