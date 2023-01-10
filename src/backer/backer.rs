use std::{mem, process, thread};
use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::Result;
use futures::executor::block_on;
use job_scheduler::{Job, JobScheduler};
use log::{debug, error, info, warn};

use crate::config::config::{AliyunOssServer, BackerConfig, BackerServer, QiniuServer, TencentOssServer};
use crate::consts;
use crate::utils::file;

pub enum State {
    Running,
    Terminated,
}

pub type BackerState = Arc<Mutex<State>>;

pub struct Backer {
    state: BackerState,
    job: Option<JoinHandle<()>>,
}

impl Backer {
    pub fn start<P: AsRef<Path>>(config_path: P) -> Result<Backer> {
        let config = match BackerConfig::load_from_file(config_path.as_ref()) {
            Ok(config) => config,
            Err(e) => {
                return Err(e.into());
            }
        };
        let state = Arc::new((Mutex::new(State::Running)));
        let thread_state = state.clone();
        let job = Some(thread::spawn(move || {
            if let Err(e) = Self::run(thread_state, config) {
                warn!("backer exited: {}", e);
                process::exit(1);
            }
        }));
        Ok(Backer { state, job })
    }

    fn run(state: BackerState, cfg: BackerConfig) -> Result<()> {
        info!("==================== Running Backer ====================");
        let cron = cfg.clone().job_cron;
        let mut sched = JobScheduler::new();
        sched.add(Job::new(cron.parse().unwrap(), || {
            block_on(Self::backup_job(cfg.clone()));
        }));


        loop {
            let state = &*state;
            let mut state_guard = state.lock().unwrap();
            match &*state_guard {
                State::Running => {}
                State::Terminated => break
            }
            drop(state_guard);
            sched.tick();
            thread::sleep(Duration::from_millis(1000));
        }
        Ok(())
    }

    pub fn stop(&mut self) {
        info!("Gracefully stopping");
        let state = &*self.state;

        let mut state_guard = state.lock().unwrap();
        *state_guard = State::Terminated;
        drop(state_guard);
        self.job.take().unwrap().join().unwrap();
        info!("Gracefully stopped");
    }

    async fn backup_job(cfg: BackerConfig) {
        info!("Executing backup job.");
        let backup_files = cfg.backup_files;
        let mut compress_mode = file::CompressType::Zip;
        let now = chrono::Local::now().format("%F_%T").to_string();

        let mode = cfg.compress_mode;
        if mode == consts::COMPRESS_MODE_TAR {
            compress_mode = file::CompressType::Tar;
        }
        let archive_file_name = String::from(format!("Archive-{}.{}", now, mode));
        let target_path = file::get_archive_dir_path().join(archive_file_name).to_str().unwrap().to_string();

        let res = file::compress_files(Box::new(backup_files), target_path.clone(), &compress_mode);
        match res {
            Ok(_) => {
                let archive_file = file::read_file_info(target_path.clone());
                match archive_file {
                    Ok(archive_file_info) => {
                        futures::join!(
                            Self::backup_file_to_backer_server(cfg.backup_target.clone(), cfg.backer_server.clone(), archive_file_info.clone()),
                            Self::backup_file_to_qiniu(cfg.backup_target.clone(), cfg.qiniu_server.clone(), archive_file_info.clone()),
                            Self::backup_file_to_aliyun_oss(cfg.backup_target.clone(), cfg.aliyun_oss_server.clone(), archive_file_info.clone()),
                            Self::backup_file_to_tencent_oss(cfg.backup_target.clone(), cfg.tencent_oss_server.clone(), archive_file_info.clone()),
                        );
                        debug!("Executed! delete archive file");
                        let _ = file::rm_file(target_path.clone());
                    }
                    Err(e) => error!("read archive file failed: {}", e)
                }
            }
            Err(e) => {
                error!("compress files failed: {}", e)
            }
        }
    }

    async fn backup_file_to_backer_server(backup_target: Vec<String>, cfg: BackerServer, archive_file: file::FileInfo) {
        if backup_target.contains(&consts::BACKUP_TARGET_BACKER_SERVER.to_string()) {
            info!("start backup_file_to_backer_server");
            let addr: SocketAddr = format!("{}:{}", cfg.ip, cfg.port).parse().unwrap();
            BackerClient::new(addr, archive_file).send();
            info!("end backup_file_to_backer_server");
        }
    }

    // TODO
    async fn backup_file_to_qiniu(backup_target: Vec<String>, cfg: QiniuServer, archive_file: file::FileInfo) {
        if backup_target.contains(&consts::BACKUP_TARGET_QINIU.to_string()) {
            info!("start backup_file_to_qiniu");
            // thread::sleep(Duration::from_secs(4));
            info!("end backup_file_to_qiniu");
        }
    }

    // TODO
    async fn backup_file_to_aliyun_oss(backup_target: Vec<String>, cfg: AliyunOssServer, archive_file: file::FileInfo) {
        if backup_target.contains(&consts::BACKUP_TARGET_ALIYUN_OSS.to_string()) {
            info!("start backup_file_to_aliyun_oss");
            // thread::sleep(Duration::from_secs(3));
            info!("end backup_file_to_aliyun_oss");
        }
    }

    // TODO
    async fn backup_file_to_tencent_oss(backup_target: Vec<String>, cfg: TencentOssServer, archive_file: file::FileInfo) {
        if backup_target.contains(&consts::BACKUP_TARGET_TENCENT_OSS.to_string()) {
            info!("start backup_file_to_tencent_oss");
            // thread::sleep(Duration::from_secs(2));
            info!("end backup_file_to_tencent_oss");
        }
    }
}

struct BackerClient {
    addr: SocketAddr,
    file_info: file::FileInfo,
}

impl BackerClient {
    pub fn new(addr: SocketAddr, file_info: file::FileInfo) -> Self {
        Self {
            addr,
            file_info,
        }
    }

    pub fn send(&self) {
        let stream = TcpStream::connect("127.0.0.1:8080");
        match stream {
            Ok(mut stream) => {
                let bin_data: Vec<u8> = bincode::serialize(&self.file_info).unwrap();
                let n = stream.write_all(bin_data.as_slice());
            }
            Err(e) => {
                error!("connect to backer server failed: {}", e)
            }
        }
    }
}

