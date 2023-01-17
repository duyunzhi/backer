use std::{io};
use std::io::{Read, Write};
use std::net::TcpStream;
use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::utils::file::FileInfo;

pub trait BaseMessage {
    // Encode message to bytes stream
    fn encode(&self) -> Result<Vec<u8>>;
    // Decode message from bytes stream
    fn decode(buf: &[u8]) -> Self;
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

    fn decode(buf: &[u8]) -> FilesInfoMessage {
        bincode::deserialize(&buf).unwrap()
    }
}

#[derive(Debug)]
pub enum Message {
    Echo(String),
    FilesInfoMessage(FilesInfoMessage),
}

impl Message {

    pub fn read_message(mut buf: &mut impl Read) -> io::Result<Message> {
        match buf.read_u8()? {
            0 => Ok(Message::Echo(extract_string(&mut buf)?)),
            1 => {
                let message_len = buf.read_u16::<NetworkEndian>()?;
                let mut bytes = vec![0u8; message_len as usize];
                buf.read_exact(&mut bytes)?;
                Ok(Message::FilesInfoMessage(BaseMessage::decode(&bytes)))
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
            Message::Echo(message) => {
                // Write the variable length message string, preceded by it's length
                let message = message.as_bytes();
                buf.write_u16::<NetworkEndian>(message.len() as u16)?;
                buf.write_all(&message)?;
                bytes_written += 2 + message.len();
            },
            Message::FilesInfoMessage(message) => {
                let message_bytes = message.encode().unwrap();
                buf.write_u16::<NetworkEndian>(message_bytes.len() as u16)?;
                buf.write_all(&message_bytes)?;
                bytes_written += 2 + message_bytes.len();
            }
        }
        Ok(bytes_written)
    }
}

impl From<&Message> for u8 {
    fn from(req: &Message) -> Self {
        match req {
            Message::Echo(_) => 0,
            Message::FilesInfoMessage { .. } => 1,
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
}