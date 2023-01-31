use std::sync::Arc;
use std::thread;

use clap::{ArgAction, Parser};
use home;
use log::{error, info};
use signal_hook::{consts::TERM_SIGNALS, iterator::Signals};

use backer::init::init::init;
use backer::packet::message::{Message, Protocol};
use backer::packet::tcp_packet::{Handler, TcpHandler, TcpServer};
use backer::utils::file;
use backer::version;

#[derive(Parser)]
struct Opts {
    /// files backup server port
    #[clap(short = 'p', long, default_value = "9618")]
    port: String,

    /// files backup dir
    #[clap(short = 'b', long, default_value = "")]
    backup_dir: String,

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

struct BackerServerHandle {
    backup_dir: String,
}

impl BackerServerHandle {
    pub fn new(backup_dir: String) -> Self {
        Self { backup_dir }
    }
}

impl Handler for BackerServerHandle {
    fn handel(&self, message: &Message, _protocol: &mut Protocol) {
        match message {
            Message::Echo(echo) => {
                info!("receive echo message: {}", echo);
            }
            Message::FilesInfoMessage(files_info_message) => {
                for file in &files_info_message.files {
                    let path = format!("{}/{}", self.backup_dir, file.file_name);
                    let res = file::create_write_file(path.clone(), file.file_data.as_slice());
                    match res {
                        Ok(_) => {
                            info!("backup file success!  file name: '{}', file path: [{}].", file.file_name, path);
                        }
                        Err(e) => { error!("save file failed! file name: '{}', file path: [{}]. error: {}", file.file_name, path, e); }
                    }
                }
            }
        }
    }
}

fn main() {
    let mut opts = Opts::parse();

    if opts.version {
        println!("{}", VERSION_INFO);
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

    let tcp_handler = TcpHandler::new();
    tcp_handler.add_handle(String::from("backer_server_handle"), Box::new(BackerServerHandle::new(opts.backup_dir.clone())));

    let server = TcpServer::new(addr.parse().unwrap(), tcp_handler);
    let server = Arc::new(server);
    let thread_server = server.clone();
    thread::spawn(move || {
        thread_server.start();
    });
    info!("backer server started! port is: {}, backup dir is: {}", opts.port, opts.backup_dir);
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