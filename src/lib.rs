use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

pub mod server;
pub mod client;
pub mod message;

pub fn hello_world(from: &str) {
    println!("Hello, world! I'm a {}!", from);
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, BorshSerialize, BorshDeserialize)]
pub struct MultiPodApplication {
    application: String,
    pod_name: String
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, BorshSerialize, BorshDeserialize)]
pub enum Applicatiton {
    SinglePod(String),
    MultiPod(MultiPodApplication)
}

impl Applicatiton {
    pub fn name(&self) -> String {
        match self {
            Applicatiton::SinglePod(name) => name.clone(),
            Applicatiton::MultiPod(multi_pod) => multi_pod.application.clone()
        }
    }
}

impl ToString for Applicatiton {
    fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}