use std::thread;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};

use clap::{ArgAction, Parser};
use home;
use log::{debug, error, info};
use signal_hook::{consts::TERM_SIGNALS, iterator::Signals};

use backer::init::init::init;
use backer::packet::message::{Message, Protocol};
use backer::packet::tcp_packet::{Dispatch, Handler, TcpServer};
use backer::utils::file;
use backer::version;

/// backer server
#[derive(Parser)]
struct Opts {
    /// files backup server port
    #[clap(short = 'p', long, default_value = "9618")]
    port: String,

    /// files backup dir
    #[clap(short = 'b', long, default_value = "")]
    backup_dir: String,

    /// backer server secret
    #[clap(long, default_value = "backer")]
    secret: String,

    /// Display the version
    #[clap(short, long, action = ArgAction::SetTrue)]
    version: bool,
}

const VERSION_INFO: &'static version::VersionInfo = &version::VersionInfo {
    name: "Backer Server",
    version: version::BACKER_SERVER_VERSION,
    compiler: env!("RUSTC_VERSION"),
    compile_time: env!("COMPILE_TIME"),
};

#[cfg(unix)]
fn wait_on_signals() {
    let mut signals = Signals::new(TERM_SIGNALS).unwrap();
    signals.forever().next();
    signals.handle().close();
}

// const CURRENT_FILE: Mutex<Option<File>> = Mutex::new(None);

struct BackerServerHandle {
    backup_dir: String,
    secret: String,
    backup_files: Mutex<HashMap<String, File>>,
}

impl BackerServerHandle {
    pub fn new(backup_dir: String, secret: String) -> Self {
        Self { backup_dir, secret, backup_files: Mutex::new(HashMap::new()) }
    }
}

impl Handler for BackerServerHandle {
    fn handel(&self, message: &Message, protocol: &mut Protocol) {
        match message {
            Message::Auth(secret) => {
                debug!("receive echo message: {}", secret);
                if self.secret.eq(secret) {
                    debug!("Authorize success!");
                    let _ = protocol.send_message(Message::Authorize(true));
                } else {
                    debug!("Authorize failed!");
                    let _ = protocol.send_message(Message::Authorize(false));
                    protocol.shutdown().unwrap();
                }
            }
            Message::FileBuffer(file_buff) => {
                if file_buff.is_begin {
                    let path = format!("{}/{}", self.backup_dir, file_buff.file_name);
                    let file = file::create_file(path.clone());
                    match file {
                        Ok(file) => {

                            info!("start backup file!  file name: '{}', file path: [{}]", file_buff.file_name, path);
                            let mut ref_file = &file;
                            let _ = ref_file.write(file_buff.buffer.as_slice());
                            self.backup_files.lock().unwrap().insert(file_buff.file_name.clone(), file);
                        }
                        Err(e) => { error!("create file failed! file name: '{}', file path: [{}]. error: {}", file_buff.file_name, path, e); }
                    }
                } else if file_buff.is_end {
                    let mut file_map = self.backup_files.lock().unwrap();
                    if let Some(mut file) = file_map.remove(file_buff.file_name.as_str()) {
                        let _ = file.write(file_buff.buffer.as_slice());
                        info!("success backup file!  file name: '{}'", file_buff.file_name);
                    } else {
                        error!("write [{}] file end failed.", file_buff.file_name.as_str())
                    }
                } else {
                    if let Some(mut file) = self.backup_files.lock().unwrap().get(file_buff.file_name.as_str()) {
                        let _ = file.write(file_buff.buffer.as_slice());
                    } else {
                        error!("write file failed. not fond [{}] file.", file_buff.file_name.as_str())
                    }
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let mut opts = Opts::parse();

    if opts.version {
        println!("{}", VERSION_INFO);
        return;
    }

    if opts.secret.len() == 0 {
        println!("backer server secret can't empty!");
        return;
    }

    init();

    if opts.backup_dir.is_empty() {
        match home::home_dir() {
            Some(path) => {
                let path = path.join("backer_dir");
                let path_str = path.to_str();
                match path_str {
                    Some(home_path) => {
                        println!("Use user home dir for backup dir");
                        opts.backup_dir = home_path.to_string()
                    }
                    None => {
                        panic!("Not parse your home dir!");
                    }
                }
            }
            None => panic!("Impossible to get your home dir!"),
        }
    }

    init_backup_dir(opts.backup_dir.clone());

    let addr = format!("0.0.0.0:{}", opts.port.clone());

    let tcp_handler = Dispatch::new_for_server();
    tcp_handler.add_handle(String::from("backer_server_handle"), Box::new(BackerServerHandle::new(opts.backup_dir.clone(), opts.secret.clone())));

    let server = TcpServer::new(addr.parse().unwrap(), tcp_handler);
    let server = Arc::new(server);
    let thread_server = server.clone();
    thread::spawn(move || {
        thread_server.start();
    });
    info!("backer server started! port is: {}, backup dir is: {}, secret is: {}", opts.port, opts.backup_dir, opts.secret);
    wait_on_signals();
    server.stop();
}

fn init_backup_dir(backup_dir: String) {
    if !file::is_exist(backup_dir.clone()) {
        // create backup dir
        let res = file::create_dir(backup_dir);
        match res {
            Ok(_) => {}
            Err(e) => panic!("Create backup dir error: {}", e),
        }
    } else {
        if !file::is_dir(backup_dir) {
            panic!("Backup dir path is not dir")
        }
    }
}