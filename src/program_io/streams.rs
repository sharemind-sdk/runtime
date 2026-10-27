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

use std::io::{self, Read, Write};
use std::mem;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::program_io::{ProgramArgument, ProgramArgumentMap, ProgramResult, ProgramResultMap};

const_assert!(mem::size_of::<usize>() >= mem::size_of::<u64>());

impl ProgramArgument {
    pub fn from_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        // Read the data length
        let data_len = reader.read_u64::<LittleEndian>()? as usize;

        // Read the data bytes
        let mut data = vec![0u8; data_len];
        reader.read_exact(&mut data)?;

        Ok(ProgramArgument { data })
    }
}

/// Deserialize the program arguments from a byte stream into the map.
pub fn read_program_argument_stream<T: Read>(reader: &mut T) -> io::Result<ProgramArgumentMap> {
    // Read the number of arguments
    let num_of_args = reader.read_u64::<LittleEndian>()? as usize;

    let mut arguments = ProgramArgumentMap::with_capacity(num_of_args);
    for _ in 0..num_of_args {
        // Read length of the name
        let name_len = reader.read_u64::<LittleEndian>()? as usize;

        // Read the name itself
        let mut name_bytes = vec![0u8; name_len];
        reader.read_exact(&mut name_bytes)?;
        let name = String::from_utf8(name_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        // Read the argument structure
        let argument = ProgramArgument::from_reader(reader)?;

        arguments.insert(name, argument);
    }

    Ok(arguments)
}

impl ProgramResult {
    pub fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        // Write protection domain
        writer.write_u64::<LittleEndian>(self.pd.len() as u64)?;
        writer.write_all(self.pd.as_bytes())?;

        // Write typename
        writer.write_u64::<LittleEndian>(self.type_.len() as u64)?;
        writer.write_all(self.type_.as_bytes())?;

        // Write data vector
        writer.write_u64::<LittleEndian>(self.data.len() as u64)?;
        writer.write_all(&self.data)?;

        Ok(())
    }
}

/// Serialize the program results from the map into a byte stream.
pub fn write_program_result_stream<W: Write>(
    writer: &mut W,
    results: &ProgramResultMap,
) -> io::Result<()> {
    // Write the number of results
    writer.write_u64::<LittleEndian>(results.len() as u64)?;

    for (name, result) in results {
        // Write the result name
        writer.write_u64::<LittleEndian>(name.len() as u64)?;
        writer.write_all(name.as_bytes())?;

        // Write the result structure
        result.serialize(writer)?;
    }

    Ok(())
}
