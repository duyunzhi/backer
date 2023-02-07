use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use job_scheduler::{Job, JobScheduler};
use log::{error, info};
use qiniu_upload_manager::{AutoUploader, AutoUploaderObjectParams, UploadManager, UploadTokenSigner};
use qiniu_upload_manager::apis::credential::Credential;
use tokio::{runtime::{Builder, Runtime}, task::JoinHandle};

use crate::config::config::{AliyunOssServer, BackerConfig, BackerServer, QiniuServer, TencentOssServer};
use crate::consts;
use crate::packet::message::{FilesInfoMessage, Message};
use crate::packet::tcp_packet::TcpClient;
use crate::utils::file;

pub enum State {
    Running,
    Terminated,
}

pub type BackerState = Arc<Mutex<State>>;

pub struct Backer {
    state: BackerState,
    rt: Runtime,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl Backer {
    pub fn new() -> Result<Backer> {
        let state = Arc::new(Mutex::new(State::Running));
        let rt = Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let threads = Default::default();
        Ok(Backer { state, rt, threads })
    }

    pub fn start<P: AsRef<Path>>(&self, config_path: P) -> Result<()> {
        let config = match BackerConfig::load_from_file(config_path.as_ref()) {
            Ok(config) => config,
            Err(e) => {
                return Err(e.into());
            }
        };
        let thread_state = self.state.clone();
        self.run(thread_state, config).unwrap();
        Ok(())
    }

    fn run(&self, state: BackerState, cfg: BackerConfig) -> Result<()> {
        info!("==================== Running Backer ====================");
        let cron = cfg.clone().job_cron;
        let mut sched = JobScheduler::new();

        sched.add(Job::new(cron.parse().unwrap(), || {
            let thread_cfg = Arc::new(cfg.clone());
            self.threads.lock().unwrap().push(self.rt.spawn(async move {
                Self::backup_job(thread_cfg);
            }));
        }));

        loop {
            let state = &*state;
            let state_guard = state.lock().unwrap();
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

    pub fn stop(&self) {
        info!("Gracefully stopping");
        let state = &*self.state;

        let mut state_guard = state.lock().unwrap();
        *state_guard = State::Terminated;
        drop(state_guard);
        // self.job.take().unwrap().join().unwrap();
        self.rt.block_on(async move {
            for t in self.threads.lock().unwrap().drain(..) {
                let _ = t.await;
            }
        });
        info!("Gracefully stopped");
    }

    fn backup_job(cfg: Arc<BackerConfig>) {
        info!("Executing backup job.");
        let rt = Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let mut threads: Vec<JoinHandle<()>> = Default::default();

        // let backup_files = cfg.backup_files;
        let mut compress_mode = file::CompressType::Zip;
        let now = chrono::Local::now().format("%F_%T").to_string();

        let mode = cfg.compress_mode.clone();
        if mode == consts::COMPRESS_MODE_TAR {
            compress_mode = file::CompressType::Tar;
        }
        let archive_file_name = String::from(format!("Archive-{}.{}", now, mode));
        let target_path = file::get_archive_dir_path().join(archive_file_name).to_str().unwrap().to_string();

        let res = file::compress_files(cfg.backup_files.clone(), target_path.clone(), compress_mode);
        match res {
            Ok(_) => {
                let archive_file = file::read_file_info(target_path.clone());
                match archive_file {
                    Ok(archive_file_info) => {
                        for target in cfg.backup_target.clone() {
                            match target.as_str() {
                                consts::BACKUP_TARGET_BACKER_SERVER => {
                                    let file_info = file::FileInfo::new(archive_file_info.file_name.clone(), archive_file_info.absolute_path.clone(), archive_file_info.file_data.clone());
                                    let backer_server = cfg.backer_server.clone();
                                    threads.push(rt.spawn(async move {
                                        Self::backup_file_to_backer_server(backer_server.clone(), file_info);
                                    }));
                                }
                                consts::BACKUP_TARGET_QINIU => {
                                    let qiniu = cfg.qiniu.clone();
                                    let file_info = file::FileInfo::new(archive_file_info.file_name.clone(), archive_file_info.absolute_path.clone(), Default::default());
                                    threads.push(rt.spawn(async move {
                                        Self::backup_file_to_qiniu(qiniu.clone(), file_info);
                                    }));
                                }
                                consts::BACKUP_TARGET_ALIYUN_OSS => {
                                    let aliyun = cfg.aliyun_oss.clone();
                                    let file_info = file::FileInfo::new(archive_file_info.file_name.clone(), archive_file_info.absolute_path.clone(), Default::default());
                                    threads.push(rt.spawn(async move {
                                        Self::backup_file_to_aliyun_oss(aliyun.clone(), file_info);
                                    }));
                                }
                                consts::BACKUP_TARGET_TENCENT_OSS => {
                                    let tencent = cfg.tencent_oss.clone();
                                    let file_info = file::FileInfo::new(archive_file_info.file_name.clone(), archive_file_info.absolute_path.clone(), Default::default());
                                    threads.push(rt.spawn(async move {
                                        Self::backup_file_to_tencent_oss(tencent.clone(), file_info);
                                    }));
                                }
                                _ => {
                                    error!("can't find target server: [{}]", target.as_str())
                                }
                            }
                        }
                        rt.block_on(async move {
                            for t in threads.drain(..) {
                                let _ = t.await;
                            }
                        });
                    }
                    Err(e) => error!("read archive file failed: {}", e)
                }
                // remove compress file
                file::rm_file(target_path.clone()).unwrap();
            }
            Err(e) => {
                error!("compress files failed: {}", e)
            }
        }
    }

    fn backup_file_to_backer_server(cfg: BackerServer, archive_file: file::FileInfo) {
        info!("start backup_file_to_backer_server");
        let addr: SocketAddr = format!("{}:{}", cfg.ip, cfg.port).parse().unwrap();
        let message = Message::FilesInfoMessage(FilesInfoMessage::new(vec![archive_file]));
        TcpClient::send_one(addr, message);
        info!("end backup_file_to_backer_server");
    }

    fn backup_file_to_qiniu(cfg: QiniuServer, archive_file: file::FileInfo) {
        info!("start backup_file_to_qiniu");
        let upload_manager = UploadManager::builder(UploadTokenSigner::new_credential_provider(
            Credential::new(cfg.access_key.as_str(), cfg.secret_key.as_str()),
            cfg.bucket_name.as_str(),
            Duration::from_secs(3600),
        )).build();
        let params = AutoUploaderObjectParams::builder().object_name(archive_file.file_name.clone()).file_name(archive_file.file_name.clone()).build();
        let uploader: AutoUploader = upload_manager.auto_uploader();
        let res = uploader.upload_path(archive_file.absolute_path.clone(), params).unwrap();
        info!("end backup_file_to_qiniu. response: {:?}", res);
    }

    // TODO
    fn backup_file_to_aliyun_oss(_cfg: AliyunOssServer, _archive_file: file::FileInfo) {
        info!("start backup_file_to_aliyun_oss");
        info!("end backup_file_to_aliyun_oss");
    }

    // TODO
    fn backup_file_to_tencent_oss(_cfg: TencentOssServer, _archive_file: file::FileInfo) {
        info!("start backup_file_to_tencent_oss");
        info!("end backup_file_to_tencent_oss");
    }
}

