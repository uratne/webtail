use std::{fs, path::{Path, PathBuf}, time::{Duration, SystemTime}};

use log::{error, info};
use tokio::{fs::File, io::{AsyncBufReadExt, AsyncSeekExt, BufReader}, sync::mpsc::Sender, time::{self, sleep}};

use crate::message::{Message, SystemMessage, SystemMessages};

use super::{configuration::LogConfiguration, process::{match_file_name, process_line}};

pub struct FileTailer {
    reader: BufReader<File>,
    path: PathBuf,
    regex: String,
    dir: String,
    created_date_time: SystemTime
}

impl FileTailer {
    pub async fn new(regex: String, dir: String) -> Option<Self> {
        let mut files = match fs::read_dir(dir.clone()) {
            Ok(files) => files,
            Err(e) => {
                error!("Error reading directory: {}", e);
                return None;
            }
        };
        loop {
            let file = files.next();
            match file {
                Some(file) => {
                    let file = match file {
                        Ok(file) => file,
                        Err(e) => {
                            error!("Error reading file: {}", e);
                            return None;
                        }
                    };
                    let file_name = match file.file_name().into_string() {
                        Ok(file_name) => file_name,
                        Err(e) => {
                            error!("Error reading file name: {:?}", e);
                            return None;
                        }
                    };
                    if match_file_name(&file_name, &regex) {
                        let path = file.path();
                        info!("Found new file: {:?}", path);
                        let file = match File::open(&path).await {
                            Ok(file) => file,
                            Err(e) => {
                                error!("Error opening file: {}", e);
                                return None;
                            }
                        };
                        let created_date_time = match file.metadata().await {
                            Ok(metadata) => match metadata.created() {
                                Ok(created_date_time) => created_date_time,
                                Err(e) => {
                                    error!("Error reading file creation date: {}", e);
                                    return None;
                                }
                            },
                            Err(e) => {
                                error!("Error reading file metadata: {}", e);
                                return None;
                            }
                        };
                        let reader = BufReader::new(file);
                        return Some(Self { reader, path, regex, dir, created_date_time })
                    }
                }
                None => {
                    break;
                }
            }
        }
        None
    }


    pub async fn tail(&mut self, tx: Sender<Message>, config: LogConfiguration) {

        self.reader.seek(std::io::SeekFrom::End(0)).await.map_err(|err| error!("Error seeking to end of file: {}", err)).unwrap();

        let sys_message = Message::System(SystemMessage::new(config.get_application(), SystemMessages::TailingStarted));
        tx.send(sys_message).await.map_err(|err| error!("Error sending tailing start message: {}", err)).unwrap();

        info!("Tailing file: {:?}", self.path);

        let mut last_line = String::new();
        let mut end_by_new_line = true;


        'OUTER: loop {
            loop {
                if tx.is_closed() {
                    break 'OUTER;
                }

                if !self.read_line(&tx, &mut last_line, &mut end_by_new_line, &config).await {
                    last_line.clear();
                    end_by_new_line = true;
                    let sys_message = Message::System(SystemMessage::new(config.get_application(), SystemMessages::FileRemoved));
                    if tx.is_closed() {
                        break 'OUTER;
                    }   
                    tx.send(sys_message).await.map_err(|err| error!("Error sending File Removed system message: {}", err)).unwrap();
                    break;
                }
            }

            loop {
                if tx.is_closed() {
                    break 'OUTER;
                }

                if self.find_next_file().await {
                    let sys_message = Message::System(SystemMessage::new(config.get_application(), SystemMessages::NewFileFound));
                    tx.send(sys_message).await.map_err(|err| error!("Error sending New File Found system message: {}", err)).unwrap();
                    break;
                }
                time::sleep(Duration::from_millis(100)).await;
            }
        }

        info!("Tailing stopped");
    }

    async fn read_line(&mut self, tx: &Sender<Message>, last_line: &mut String, end_by_new_line: &mut bool, config: &LogConfiguration) -> bool {
        let mut line = String::new();
        let bytes_read = match self.reader.read_line(&mut line).await {
            Ok(bytes_read) => bytes_read,
            Err(e) => {
                error!("Error reading line: {}", e);
                return false
            }
        };
    
        if bytes_read == 0 {
            sleep(Duration::from_millis(100)).await;
            let path = Path::new(&self.path);
            let exists = Path::exists(path);
            if !exists {
                info!("File removed: {:?}", self.path);
                return false
            }
            let file = match File::open(&self.path).await {
                Ok(file) => file,
                Err(e) => {
                    error!("Error opening file: {}", e);
                    return false
                }
            };
            let same_file = match file.metadata().await {
                Ok(metadata) => match metadata.created() {
                    Ok(created_date_time) => created_date_time == self.created_date_time,
                    Err(e) => {
                        error!("Error reading file metadata: {}", e);
                        return false
                    }
                },
                Err(e) => {
                    error!("Error reading file metadata: {}", e);
                    return false
                }
            };
            
            if !same_file {
                info!("File replaced: {:?}", self.path);
                return false
            }
        } else {
            process_line(line, &mut *last_line, &mut *end_by_new_line, &tx, &config).await;
        }    
        true
    }

    async fn find_next_file(&mut self) -> bool {
        let mut files = match fs::read_dir(self.dir.clone()) {
            Ok(files) => files,
            Err(e) => {
                error!("Error reading directory: {}", e);
                return false
            }
        };
        loop {
            let file = files.next();
            match file {
                Some(file) => {
                    let file = match file {
                        Ok(file) => file,
                        Err(e) => {
                            error!("Error reading file: {}", e);
                            return false
                        }
                    };
                    let file_name = match file.file_name().into_string() {
                        Ok(file_name) => file_name,
                        Err(e) => {
                            error!("Error reading file name: {:?}", e);
                            return false
                        }
                    };
                    if match_file_name(&file_name, &self.regex) {
                        let path = file.path();
                        info!("Found file: {:?}", path);
                        let file = match File::open(&path).await {
                            Ok(file) => file,
                            Err(e) => {
                                error!("Error opening file: {}", e);
                                return false
                            }
                        };
                        let created_date_time = match file.metadata().await {
                            Ok(metadata) => match metadata.created() {
                                Ok(created_date_time) => created_date_time,
                                Err(e) => {
                                    error!("Error reading file creation date: {}", e);
                                    return false
                                }
                            },
                            Err(e) => {
                                error!("Error reading file metadata: {}", e);
                                return false
                            }
                        };
                        self.created_date_time = created_date_time;
                        self.reader = BufReader::new(file);
                        self.path = path;
                        return true
                    }
                }
                None => {
                    break;
                }
            }
        }
        false
    }
}