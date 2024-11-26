use borsh::{BorshDeserialize, BorshSerialize};
use chrono::{DateTime, NaiveDateTime};
use serde::{Deserialize, Serialize};

use crate::Applicatiton;


#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone)]
pub enum SystemMessages {
    FileFound,
    FileRemoved,
    NewFileFound,
    TailingStarted,
    Start,
    Stop,
    Pause,
    Resume
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DataMessage {
    #[serde(rename = "type")]
    message_type: String,
    row: String,
    application: Applicatiton,
    replace_last_row: bool,
    timestamp: NaiveDateTime
}

impl From<BinaryDataMessage> for DataMessage {
    fn from(value: BinaryDataMessage) -> Self {
        Self {
            message_type: value.message_type,
            row: value.row,
            application: value.application,
            replace_last_row: value.replace_last_row,
            timestamp: DateTime::from_timestamp_nanos(value.timestamp).naive_utc(),
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct BinaryDataMessage {
    message_type: String,
    row: String,
    application: Applicatiton,
    replace_last_row: bool,
    timestamp: i64
}

impl From<DataMessage> for BinaryDataMessage {
    fn from(value: DataMessage) -> Self {
        Self {
            message_type: value.message_type,
            row: value.row,
            application: value.application,
            replace_last_row: value.replace_last_row,
            timestamp: value.timestamp.and_utc().timestamp_nanos_opt().expect("Out of bound timestamp for unix time"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemMessage {
    #[serde(rename = "type")]
    message_type: String,
    application: Applicatiton,
    message: SystemMessages,
    timestamp: NaiveDateTime
}

impl From<BinarySystemMessage> for SystemMessage {
    fn from(value: BinarySystemMessage) -> Self {
        Self {
            message_type: value.message_type,
            application: value.application,
            message: value.message,
            timestamp: DateTime::from_timestamp_nanos(value.timestamp).naive_utc(),
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct BinarySystemMessage {
    message_type: String,
    application: Applicatiton,
    message: SystemMessages,
    timestamp: i64
}

impl From<SystemMessage> for BinarySystemMessage {
    fn from(value: SystemMessage) -> Self {
        Self {
            message_type: value.message_type,
            application: value.application,
            message: value.message,
            timestamp: value.timestamp.and_utc().timestamp_nanos_opt().expect("Out of bound timestamp for unix time"),
        }
    }
}

impl DataMessage {
    pub fn new(row: String, application: Applicatiton, replace_last_row: bool) -> Self {
        Self { message_type: "Data".to_string(), row, application, replace_last_row, timestamp: chrono::Utc::now().naive_utc() }
    }

    pub fn row(&self) -> &str {
        &self.row
    }
}

impl SystemMessage {
    pub fn new(application: Applicatiton, message: SystemMessages) -> Self {
        Self { message_type: "System".to_string(), application, message, timestamp: chrono::Utc::now().naive_utc() }
    }

    pub fn message(&self) -> &SystemMessages {
        &self.message
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Message {
    Data(DataMessage),
    System(SystemMessage),
    ClientDisconnect
}

impl From<BinaryMessage> for Message {
    fn from(value: BinaryMessage) -> Self {
        match value {
            BinaryMessage::Data(binary_data_message) => Message::Data(DataMessage::from(binary_data_message)),
            BinaryMessage::System(binary_system_message) => Message::System(SystemMessage::from(binary_system_message)),
            BinaryMessage::ClientDisconnect => Message::ClientDisconnect,
        }
    }
}

impl Message {
    pub fn data(&self) -> Option<&DataMessage> {
        match self {
            Message::Data(data) => Some(data),
            _ => None
        }
    }
    
    pub fn system(&self) -> Option<&SystemMessage> {
        match self {
            Message::System(system) => Some(system),
            _ => None
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub enum BinaryMessage {
    Data(BinaryDataMessage),
    System(BinarySystemMessage),
    ClientDisconnect
}

impl From<Message> for BinaryMessage {
    fn from(value: Message) -> Self {
        match value {
            Message::Data(data_message) => BinaryMessage::Data(BinaryDataMessage::from(data_message)),
            Message::System(system_message) => BinaryMessage::System(BinarySystemMessage::from(system_message)),
            Message::ClientDisconnect => BinaryMessage::ClientDisconnect,
        }
    }
}
