use std::collections::BTreeMap;

use borsh::{BorshDeserialize, BorshSerialize};
use message::Message;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Sender;

pub mod server;
pub mod client;
pub mod message;

pub const DEFAULT_APPLICATION: &str = "<--DEFAULT-->";

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

    pub fn configure_default_pods(&self, configuration: &BTreeMap<Applicatiton, Sender<Message>>) -> Self {
        return match self {
            Applicatiton::SinglePod(_) => self.clone(),
            Applicatiton::MultiPod(multi_pod_application) => {
                if multi_pod_application.pod_name.eq_ignore_ascii_case(DEFAULT_APPLICATION) {
                    let mut pod_id = 0;
                    return loop {
                        let pod_name = format!("Pod {}", pod_id);
                        let application = Applicatiton::MultiPod(MultiPodApplication {
                            application: multi_pod_application.application.clone(),
                            pod_name
                        });

                        if !configuration.contains_key(&application) {
                            break application;
                        }
                        pod_id += 1;
                    }
                } else {
                    self.clone()
                }
            },
        }
    } 
}

impl ToString for Applicatiton {
    fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}