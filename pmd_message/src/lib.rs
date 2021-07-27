use binread::{BinRead, BinReaderExt, NullWideString};
use binwrite::BinWrite;
use byteorder::{WriteBytesExt, LE};
use pmd_code_table::{CodeToText, CodeToTextError, TextToCode, TextToCodeError};
use pmd_sir0::{write_sir0_footer, write_sir0_header, Sir0, Sir0Error, Sir0WriteFooterError};
use std::{
    collections::BTreeMap,
    convert::TryInto,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    num::TryFromIntError,
    u32,
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
    #[error("can't decode the string {1:?}")]
    CantDecodeString(#[source] CodeToTextError, String),
}

/// An error that may occur when writing a [`MessageBin`] file via [`Messagebin::write`]
#[derive(Error, Debug)]
pub enum MessageBinWriteError {
    #[error("an input/output error occured")]
    IOError(#[from] io::Error),
    #[error("the target file is too big (more that 4^32 bytes) (int conversion failed)")]
    TooBigError(#[from] TryFromIntError),
    #[error("the target file is too big (more that 4^32 bytes) (overflow)")]
    Overflow,
    #[error("an error occured writing the sir0 footer")]
    Sir0WriteFooterError(#[from] Sir0WriteFooterError),
    #[error("Can't transform a human text into a encoded string (may be related to invalid label in the source text). Source text : {1:?}")]
    CantEncodeText(#[source] TextToCodeError, String),
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
#[derive(Debug, Default)] //TODO: maybe there is a library for this kind of data structure (map sorted with addition order)
pub struct MessageBin {
    /// Contain a reference to the index of an image stored in this file, indexed by the id (an hash)
    hash_to_id: BTreeMap<u32, usize>,
    /// Contain the list of message, in the order of the file, with it's hash, an unknown value and content
    message: Vec<(u32, u32, String)>,
}

impl MessageBin {
    /// Return all the hash, the unk value and message content stored in the file
    pub fn messages(&self) -> &Vec<(u32, u32, String)> {
        &self.message
    }

    /// Return the message content with the given hash if it exist.
    pub fn message_by_hash(&self, hash: u32) -> Option<&String> {
        match self.hash_to_id.get(&hash) {
            None => None,
            Some(key) => Some(&self.message[*key].2),
        }
    }

    /// If the hash is already present, update the message content and unknown value, otherwise, add a new message at the end of the messages list.
    /// Return the old string if it exist.
    pub fn insert(&mut self, hash: u32, unk: u32, message: String) {
        match self.hash_to_id.get(&hash) {
            None => {
                let position = self.message.len();
                self.message.push((hash, unk, message));
                self.hash_to_id.insert(hash, position);
            }
            Some(position) => {
                self.message[*position].1 = unk;
                self.message[*position].2 = message;
            }
        }
    }

    /// Load a MessageBin file from the reader.
    pub fn load_file<T: Read + Seek>(
        mut file: &mut T,
        code_to_text: Option<&CodeToText>,
    ) -> Result<Self, MessageBinReadError> {
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

        strings_data.sort_unstable_by_key(|e| e.string_pointer);

        let mut message_bin = MessageBin::default();
        for string_data in strings_data {
            file.seek(SeekFrom::Start(string_data.string_pointer as u64))?;
            let text: MessageBinText = file.read_le()?;
            let text = if let Some(code_to_text) = code_to_text {
                code_to_text.decode(&text.text).map_err(|err| {
                    MessageBinReadError::CantDecodeString(err, text.text.to_string())
                })?
            } else {
                text.text.to_string()
            };
            message_bin.insert(string_data.string_hash, string_data.unk, text);
        }

        Ok(message_bin)
    }

    // Write a MessageBin to the given writer.
    //TODO: ugly, rewrite & cleanup
    pub fn write<T: Seek + Write>(
        &self,
        file: &mut T,
        text_to_code: Option<&TextToCode>,
    ) -> Result<(), MessageBinWriteError> {
        let mut sir0_offsets: Vec<u32> = vec![4, 8];

        file.write_all(&[0; 16])?; //sir0 header and padding

        let mut strings_data = Vec::new();
        let mut text_current_offset: u32 = 16;
        for (hash, unk, text) in self.messages().iter() {
            let text_to_write = if let Some(text_to_code) = text_to_code {
                text_to_code
                    .encode(text)
                    .map_err(|err| MessageBinWriteError::CantEncodeText(err, text.to_string()))?
            } else {
                text.encode_utf16().collect()
            };
            let mut binary_text_to_write = text_to_write
                .iter()
                .flat_map(|v| v.to_le_bytes().to_vec())
                .collect::<Vec<u8>>();
            binary_text_to_write.push(0);
            binary_text_to_write.push(0);
            file.write_all(&binary_text_to_write)?;
            strings_data.push(MessageBinStringData {
                string_pointer: text_current_offset,
                string_hash: *hash,
                unk: *unk,
            });

            text_current_offset = text_current_offset
                .checked_add(binary_text_to_write.len().try_into()?)
                .map_or_else(|| Err(MessageBinWriteError::Overflow), Ok)?;
        }

        // padding of 4
        #[allow(unused_assignments)]
        if text_current_offset % 4 != 0 {
            let nb_to_seek = 4 - text_current_offset % 4;
            file.write_all(&vec![0; nb_to_seek as usize])?;
            text_current_offset += nb_to_seek;
        }

        strings_data.sort_unstable_by_key(|e| e.string_hash);

        let string_meta_position: u32 = file.seek(SeekFrom::Current(0))?.try_into()?;
        strings_data.write(file)?; // * macro magic * !!!
        for count in 0..strings_data.len() {
            sir0_offsets.push(string_meta_position + (count as u32) * 12);
        }

        let number_of_strings: u32 = strings_data.len().try_into()?;
        let string_relative_end_offset = number_of_strings
            .checked_mul(12)
            .map_or_else(|| Err(MessageBinWriteError::Overflow), Ok)?;
        let string_absolute_end_offset = string_meta_position
            .checked_add(string_relative_end_offset)
            .map_or_else(|| Err(MessageBinWriteError::Overflow), Ok)?;
        sir0_offsets.push(
            string_absolute_end_offset
                .checked_add(4)
                .map_or_else(|| Err(MessageBinWriteError::Overflow), Ok)?,
        );

        let sir0_header_position = file.seek(SeekFrom::Current(0))?;
        file.write_u32::<LE>(strings_data.len().try_into()?)?;
        file.write_u32::<LE>(string_meta_position)?;

        let current_position = file.seek(SeekFrom::Current(0))?;
        //TODO: this might need some magic :)
        if current_position % 16 != 0 {
            file.write_all(&vec![0; 16 - (current_position as usize % 16)])?;
        };

        let sir0_footer_position = file.seek(SeekFrom::Current(0))?;

        write_sir0_footer(file, &sir0_offsets)?;

        /*if current_position % 16 != 0 {
            file.write_all(&vec![0; 16 - (current_position as usize % 16)])?;
        };*/

        //file.write_all(&[0; 11])?; //TODO: this doesn't look like a padding ...

        file.seek(SeekFrom::Start(0))?;
        write_sir0_header(
            file,
            sir0_header_position.try_into()?,
            sir0_footer_position.try_into()?,
        )?;
        Ok(())
    }
}
