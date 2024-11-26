use std::collections::BTreeMap;

use tokio::sync::{broadcast::Sender, Mutex};

use crate::{message::Message, Applicatiton};

pub type Broadcasters = Mutex<BTreeMap<Applicatiton, Sender<Message>>>;

pub fn new_broadcasters() -> Broadcasters {
    Mutex::new(BTreeMap::new())
}