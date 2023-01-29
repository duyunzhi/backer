use log::{info, LevelFilter};
use time::UtcOffset;

use crate::utils::file;

pub fn init() {
    init_log();
    init_archive_dir();
}

fn init_log() {
    let utc = UtcOffset::current_local_offset().unwrap();
    simple_logger::SimpleLogger::new().with_level(LevelFilter::Info).with_utc_offset(utc).init().unwrap()
}

fn init_archive_dir() {
    let archive_dir = file::get_archive_dir_path();
    if !file::is_exist(archive_dir.as_path()) {
        info!("init archive dir: {}", archive_dir.to_str().unwrap());
        let _ = file::create_dir(archive_dir.as_path());
    }
}