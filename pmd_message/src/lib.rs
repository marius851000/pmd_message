use binread::{BinRead, BinReaderExt, NullWideString};
use binwrite::BinWrite;
use byteorder::{WriteBytesExt, LE};
use pmd_sir0::{write_sir0_footer, write_sir0_header, Sir0, Sir0Error, Sir0WriteFooterError};
use std::{
    collections::BTreeMap,
    convert::TryInto,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    num::TryFromIntError,
};
use thiserror::Error;

/// An error that may occur when reading a [`MessageBin`] file via [`MessageBin::load_file`]
#[derive(Error, Debug)]
pub enum MessageBinReadError {
    #[error("an input/output error occured")]
    IOError(#[from] io::Error),
    #[error("an error occured when reding the Sir0 part of the file")]
    Sir0Error(#[from] Sir0Error),
    #[error("a binread error occured")]
    BinReadError(#[from] binread::Error),
}

/// An error that may occur when writing a [`MessageBin`] file via [`Messagebin::write`]
#[derive(Error, Debug)]
pub enum MessageBinWriteError {
    #[error("an input/output error occured")]
    IOError(#[from] io::Error),
    #[error("the target file is too big (more that 4^32 bytes)")]
    TooBigError(#[from] TryFromIntError),
    #[error("an error occured writing the sir0 footer")]
    Sir0WriteFooterError(#[from] Sir0WriteFooterError),
}

#[derive(BinRead, Debug)]
#[br(little)]
struct MessageBinSir0Header {
    string_count: u32,
    string_info_pointer: u32,
}

#[derive(BinRead, Debug, BinWrite)]
#[br(little)]
#[binwrite(little)]
struct MessageBinStringData {
    string_pointer: u32,
    string_hash: u32,
    unk: u32,
}

#[derive(BinRead, Debug)]
#[br(little)]
struct MessageBinText {
    text: NullWideString,
}

/// A structure representing a translation (message) file in 3ds pokemon mystery dungeon games.
///
/// Each text have an associated (32bit, probably crc32) hash associated with them as a key.
#[derive(Debug)]
pub struct MessageBin {
    /// Contain the messages stored in this file, indexed by the id (an hash)
    messages: BTreeMap<u32, String>,
}

impl MessageBin {
    /// Load a MessageBin file from the reader.
    pub fn load_file<T: Read + Seek>(mut file: &mut T) -> Result<Self, MessageBinReadError> {
        file.seek(SeekFrom::Start(0))?;
        // read sir0
        let sir0 = Sir0::new(&mut file)?;

        let mut sir0_header_cursor = Cursor::new(sir0.get_header());
        let sir0_header: MessageBinSir0Header = sir0_header_cursor.read_le()?;

        // read string data
        file.seek(SeekFrom::Start(sir0_header.string_info_pointer as u64))?;

        let mut strings_data: Vec<MessageBinStringData> =
            Vec::with_capacity(sir0_header.string_count as usize);
        for _ in 0..sir0_header.string_count {
            strings_data.push(file.read_le()?);
        }

        let mut messages = BTreeMap::new();
        for string_data in strings_data {
            file.seek(SeekFrom::Start(string_data.string_pointer as u64))?;
            let text: MessageBinText = file.read_le()?;
            let text = text.text.to_string();
            messages.insert(string_data.string_hash, text);
        }

        Ok(Self { messages })
    }

    // Write a MessageBin to the given writer.
    pub fn write<T: Seek + Write>(&self, file: &mut T) -> Result<(), MessageBinWriteError> {
        let mut sir0_offsets: Vec<u32> = vec![4, 8];

        file.write_all(&[0; 18])?; //sir0 header and padding

        let mut strings_data = Vec::new();
        for (hash, text) in self.messages.iter() {
            let mut to_write = text
                .encode_utf16()
                .flat_map(|v| v.to_le_bytes().to_vec())
                .collect::<Vec<u8>>();
            to_write.push(0);
            to_write.push(0);
            strings_data.push(MessageBinStringData {
                string_hash: *hash,
                string_pointer: file.seek(SeekFrom::Current(0))?.try_into()?,
                unk: 0,
            });
            file.write_all(&to_write)?;
        }

        let string_meta_position = file.seek(SeekFrom::Current(0))?.try_into()?;
        sir0_offsets.push(string_meta_position);
        strings_data.write(file)?; // * macro magic * !!!
        for count in 0..strings_data.len() {
            sir0_offsets.push(string_meta_position + (count as u32) * 12);
        }

        let last_one = sir0_offsets.len() - 1;
        sir0_offsets[last_one] += 4;

        let sir0_header_position = file.seek(SeekFrom::Current(0))?;
        file.write_u32::<LE>(strings_data.len().try_into()?)?;
        file.write_u32::<LE>(string_meta_position)?;

        let current_position = file.seek(SeekFrom::Current(0))?;
        //TODO: this might need some magic :)
        if current_position % 32 != 0 {
            file.write_all(&vec![0; 32 - (current_position as usize % 32)])?;
        };

        let sir0_footer_position = file.seek(SeekFrom::Current(0))?;

        write_sir0_footer(file, &sir0_offsets)?;

        file.write_all(&[0; 11])?; //TODO: this doesn't look like a padding ...

        file.seek(SeekFrom::Start(0))?;
        write_sir0_header(
            file,
            sir0_header_position.try_into()?,
            sir0_footer_position.try_into()?,
        )?;
        Ok(())
    }
}
