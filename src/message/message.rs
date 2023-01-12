use std::fmt;

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub trait Message {
    // Encode message to bytes stream
    fn encode(&self) -> Result<Vec<u8>>;
    // The message type
    fn message_type(&self) -> MessageType;
    // Decode message from bytes stream
    fn decode(buf: &Vec<u8>) -> Self;
}

// pub type MessageType = u8;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum MessageType {
    None = 0,
    Syslog = 1,
    Statsd = 2,
    Metrics = 3,
    TaggedFlow = 4,
    ProtocolLog = 5,
    OpenTelemetry = 6,
    Prometheus = 7,
    Telegraf = 8,
    PacketSequenceBlock = 9,
    // Enterprise Edition Feature: packet-sequence
    DeepflowStats = 10,
    OpenTelemetryCompressed = 11,
    RawPcap = 12, // Enterprise Edition Feature: pcap
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BackerMessage<T> {
    tp: MessageType,
    data: T,
}

impl<T: fmt::Debug + Serialize + for<'a> Deserialize<'a>> BackerMessage<T> {
    pub fn new(tp: MessageType, data: T) -> BackerMessage<T> {
        Self {
            tp,
            data,
        }
    }
}


impl<T: fmt::Debug + Serialize + for<'a> Deserialize<'a>> Message for BackerMessage<T> {
    fn encode(&self) -> Result<Vec<u8>> {
        let serialize: Vec<u8> = bincode::serialize(&self).unwrap();
        Ok(serialize)
    }

    fn message_type(&self) -> MessageType {
        self.tp
    }

    fn decode(buf: &Vec<u8>) -> BackerMessage<T> {
        bincode::deserialize(&buf).unwrap()
    }
}

impl<T: fmt::Debug + Serialize + for<'a> Deserialize<'a>> fmt::Display for BackerMessage<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BackerMessage: [tp: {:?}, data: {:?}]", self.tp, self.data)
    }
}

impl<T: fmt::Debug + Serialize + for<'a> Deserialize<'a>> fmt::Debug for BackerMessage<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

