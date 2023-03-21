use std::io;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};

use anyhow::Result;
use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};
use log::error;
use serde::{Deserialize, Serialize};

use crate::utils::file::FileInfo;

pub trait BaseMessage {
    // Encode message to bytes stream
    fn encode(&self) -> Result<Vec<u8>>;
    // Decode message from bytes stream
    fn decode(&mut self, buf: &[u8]) -> Result<()>;
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesInfoMessage {
    pub files: Vec<FileInfo>,
}

impl FilesInfoMessage {
    pub fn new(files: Vec<FileInfo>) -> Self {
        Self {
            files
        }
    }
}

impl BaseMessage for FilesInfoMessage {
    fn encode(&self) -> Result<Vec<u8>> {
        let serialize: Vec<u8> = bincode::serialize(&self).unwrap();
        Ok(serialize)
    }

    fn decode(&mut self, buf: &[u8]) -> Result<()> {
        let msg = bincode::deserialize::<FilesInfoMessage>(&buf)?;
        self.files = msg.files;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileBuffer {
    pub is_begin: bool,
    pub is_end: bool,
    pub file_name: String,
    pub buffer: Vec<u8>,
}

impl FileBuffer {
    pub fn new(file_name: String, buffer: Vec<u8>) -> Self {
        Self {
            is_begin: false,
            is_end: false,
            file_name,
            buffer,
        }
    }

    pub fn get_buffer_length(&self) -> usize {
        self.buffer.len()
    }

    pub fn cut_file_buff(&self, max_buffer_length: usize) -> Vec<Self> {
        let mut buffers = vec![];
        let chunks = self.buffer.chunks(max_buffer_length);
        let chunks_size = chunks.len();
        let mut i = 0;
        for chunk in chunks {
            let part = Self::new_part(i == 0, i == chunks_size - 1, self.file_name.clone(), chunk.to_vec());
            buffers.push(part);
            i += 1;
        }
        buffers
    }

    fn new_part(is_begin: bool, is_end: bool, file_name: String, buffer: Vec<u8>) -> Self {
        Self {
            is_begin,
            is_end,
            file_name,
            buffer,
        }
    }
}

impl BaseMessage for FileBuffer {
    fn encode(&self) -> Result<Vec<u8>> {
        let serialize: Vec<u8> = bincode::serialize(&self).unwrap();
        Ok(serialize)
    }

    fn decode(&mut self, buf: &[u8]) -> Result<()> {
        let buffer = bincode::deserialize::<FileBuffer>(&buf)?;
        self.is_begin = buffer.is_begin;
        self.is_end = buffer.is_end;
        self.file_name = buffer.file_name;
        self.buffer = buffer.buffer;
        Ok(())
    }
}

impl Default for FileBuffer {
    fn default() -> Self {
        Self {
            is_begin: false,
            is_end: false,
            file_name: String::from(""),
            buffer: vec![],
        }
    }
}

#[derive(Debug)]
pub enum Message {
    Phrase(String),
    Auth(String),
    Authorize(bool),
    FileBuffer(FileBuffer),
    Complete(bool),
}

impl Message {
    pub fn read_message(mut buf: &mut impl Read) -> io::Result<Message> {
        match buf.read_u8()? {
            0 => Ok(Message::Phrase(extract_string(&mut buf)?)),
            1 => Ok(Message::Auth(extract_string(&mut buf)?)),
            2 => {
                let message_len = buf.read_u16::<NetworkEndian>()?;
                let mut bytes = vec![0u8; message_len as usize];
                buf.read_exact(&mut bytes)?;
                if bytes[0] == 1 {
                    Ok(Message::Authorize(true))
                } else if bytes[0] == 0 {
                    Ok(Message::Authorize(true))
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "decode message failed",
                    ))
                }
            }
            3 => {
                let message_len = buf.read_u16::<NetworkEndian>()?;
                let mut bytes = vec![0u8; message_len as usize];
                buf.read_exact(&mut bytes)?;
                let mut buffer = FileBuffer::default();
                let res = buffer.decode(&bytes);
                match res {
                    Ok(_) => {
                        Ok(Message::FileBuffer(buffer))
                    }
                    Err(e) => {
                        error!("decode message failed: {}", e);
                        Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "decode message failed",
                        ))
                    }
                }
            }
            4 => {
                let message_len = buf.read_u16::<NetworkEndian>()?;
                let mut bytes = vec![0u8; message_len as usize];
                buf.read_exact(&mut bytes)?;
                if bytes[0] == 1 {
                    Ok(Message::Complete(true))
                } else if bytes[0] == 0 {
                    Ok(Message::Complete(true))
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "decode message failed",
                    ))
                }
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid Message Type",
            )),
        }
    }

    pub fn write_message(&self, buf: &mut impl Write) -> io::Result<usize> {
        buf.write_u8(self.into())?; // Message Type byte
        let mut bytes_written: usize = 1;
        match self {
            Message::Phrase(message) => {
                // Write the variable length message string, preceded by it's length
                let message = message.as_bytes();
                buf.write_u16::<NetworkEndian>(message.len() as u16)?;
                buf.write_all(&message)?;
                bytes_written += 2 + message.len();
            }
            Message::Auth(message) => {
                // Write the variable length message string, preceded by it's length
                let message = message.as_bytes();
                buf.write_u16::<NetworkEndian>(message.len() as u16)?;
                buf.write_all(&message)?;
                bytes_written += 2 + message.len();
            }
            Message::Authorize(message) => {
                let bytes: [u8; 1];
                if *message { bytes = [1]; } else { bytes = [0]; }
                buf.write_u16::<NetworkEndian>(bytes.len() as u16)?;
                buf.write_all(&bytes)?;
                bytes_written += 2 + bytes.len();
            }
            Message::FileBuffer(message) => {
                let message_bytes = message.encode().unwrap();
                buf.write_u16::<NetworkEndian>(message_bytes.len() as u16)?;
                buf.write_all(&message_bytes)?;
                bytes_written += 2 + message_bytes.len();
            }
            Message::Complete(message) => {
                let bytes: [u8; 1];
                if *message { bytes = [1]; } else { bytes = [0]; }
                buf.write_u16::<NetworkEndian>(bytes.len() as u16)?;
                buf.write_all(&bytes)?;
                bytes_written += 2 + bytes.len();
            }
        }
        Ok(bytes_written)
    }
}

impl From<&Message> for u8 {
    fn from(req: &Message) -> Self {
        match req {
            Message::Phrase(_) => 0,
            Message::Auth(_) => 1,
            Message::Authorize(_) => 2,
            Message::FileBuffer(_) => 3,
            Message::Complete(_) => 4,
        }
    }
}

fn extract_string(buf: &mut impl Read) -> io::Result<String> {
    // byte order ReadBytesExt
    let length = buf.read_u16::<NetworkEndian>()?;
    // Given the length of our string, only read in that quantity of bytes
    let mut bytes = vec![0u8; length as usize];
    buf.read_exact(&mut bytes)?;
    // And attempt to decode it as UTF8
    String::from_utf8(bytes).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid utf8"))
}

pub struct Protocol {
    reader: io::BufReader<TcpStream>,
    stream: TcpStream,
}

impl Protocol {
    /// Wrap a TcpStream with Protocol
    pub fn with_stream(stream: TcpStream) -> io::Result<Self> {
        Ok(Self {
            reader: io::BufReader::new(stream.try_clone()?),
            stream,
        })
    }

    /// Serialize a message to the server and write it to the TcpStream
    pub fn send_message(&mut self, message: Message) -> io::Result<()> {
        message.write_message(&mut self.stream)?;
        self.stream.flush()
    }

    /// Read a message from the inner TcpStream
    ///
    /// NOTE: Will block until there's data to read (or deserialize fails with io::ErrorKind::Interrupted)
    ///       so only use when a message is expected to arrive
    pub fn read_message(&mut self) -> io::Result<Message> {
        Message::read_message(&mut self.reader)
    }

    pub fn shutdown(&self) -> io::Result<()> {
        self.stream.shutdown(Shutdown::Both)
    }
}