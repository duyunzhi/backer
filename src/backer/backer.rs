use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
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
use crate::packet::message::{FileBuffer, Message, Protocol};
use crate::packet::tcp_packet::{Dispatch, Handler, TcpClient};
use crate::utils::file;

const MAX_BUFFER_LENGTH: usize = 20480;

pub enum State {
    Running,
    Terminated,
}

pub type BackerState = Arc<Mutex<State>>;
pub type CompletedState = Arc<AtomicBool>;


pub struct Backer {
    state: BackerState,
    completed_state: CompletedState,
    rt: Runtime,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl Backer {
    pub fn new() -> Result<Backer> {
        let state = Arc::new(Mutex::new(State::Running));
        let completed_state = Arc::new(AtomicBool::new(false));
        let rt = Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let threads = Default::default();
        Ok(Backer { state, completed_state, rt, threads })
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
            self.backup_job(thread_cfg);
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

    fn backup_job(&self, cfg: Arc<BackerConfig>) {
        info!("Executing backup job.");
        let mut compress_mode = file::CompressType::Zip;
        let now = chrono::Local::now().format("%F_%T").to_string();

        let mode = cfg.compress_mode.clone();
        if mode == consts::COMPRESS_MODE_TAR {
            compress_mode = file::CompressType::Tar;
        }
        let archive_file_name = String::from(format!("Archive-{}.{}", now, mode));
        let target_path = file::get_archive_dir_path().join(archive_file_name).to_str().unwrap().to_string();

        let res = file::compress_files(cfg.backup_files.clone(), target_path.clone(), compress_mode);
        info!("Compress files success.");
        match res {
            Ok(_) => {
                let archive_file = file::read_file_info_without_file_data(target_path.clone());
                match archive_file {
                    Ok(archive_file_info) => {
                        for target in cfg.backup_target.clone() {
                            match target.as_str() {
                                consts::BACKUP_TARGET_BACKER_SERVER => {
                                    let archive_file = file::read_file_info(target_path.clone());
                                    match archive_file {
                                        Ok(archive_file_info) => {
                                            let file_info = file::FileInfo::new(archive_file_info.file_name.clone(), archive_file_info.absolute_path.clone(), archive_file_info.file_data.clone());
                                            let backer_server = cfg.backer_server.clone();
                                            let completed = self.completed_state.clone();
                                            self.threads.lock().unwrap().push(self.rt.spawn(async move {
                                                Self::backup_file_to_backer_server(backer_server.clone(), file_info, completed).await;
                                            }));
                                        }
                                        Err(e) => error!("read archive file failed: {}", e)
                                    }
                                }
                                consts::BACKUP_TARGET_QINIU => {
                                    let file_info = file::FileInfo::new(archive_file_info.file_name.clone(), archive_file_info.absolute_path.clone(), Default::default());
                                    let qiniu = cfg.qiniu.clone();
                                    self.threads.lock().unwrap().push(self.rt.spawn(async move {
                                        Self::backup_file_to_qiniu(qiniu.clone(), file_info).await;
                                    }));
                                }
                                consts::BACKUP_TARGET_ALIYUN_OSS => {
                                    let file_info = file::FileInfo::new(archive_file_info.file_name.clone(), archive_file_info.absolute_path.clone(), Default::default());
                                    let aliyun = cfg.aliyun_oss.clone();
                                    self.threads.lock().unwrap().push(self.rt.spawn(async move {
                                        Self::backup_file_to_aliyun_oss(aliyun.clone(), file_info).await;
                                    }));
                                }
                                consts::BACKUP_TARGET_TENCENT_OSS => {
                                    let file_info = file::FileInfo::new(archive_file_info.file_name.clone(), archive_file_info.absolute_path.clone(), Default::default());
                                    let tencent = cfg.tencent_oss.clone();
                                    self.threads.lock().unwrap().push(self.rt.spawn(async move {
                                        Self::backup_file_to_tencent_oss(tencent.clone(), file_info).await;
                                    }));
                                }
                                _ => {
                                    error!("can't find target server: [{}]", target.as_str())
                                }
                            }
                        }
                        self.rt.block_on(async move {
                            for t in self.threads.lock().unwrap().drain(..) {
                                let _ = t.await;
                            }
                        });
                    }
                    Err(e) => error!("read archive file failed: {}", e)
                }
                // remove compress file
                file::rm_file(target_path.clone()).unwrap();
                info!("remove archive file");
            }
            Err(e) => {
                error!("compress files failed: {}", e);
            }
        }
    }

    async fn backup_file_to_backer_server(cfg: BackerServer, archive_file: file::FileInfo, completed: Arc<AtomicBool>) {
        info!("start backup_file_to_backer_server");
        let addr: SocketAddr = format!("{}:{}", cfg.ip, cfg.port).parse().unwrap();
        let tcp_handler = Dispatch::new_for_client();
        let handle_completed = completed.clone();
        tcp_handler.add_handle(String::from("backer_handle"), Box::new(BackerHandle::new(archive_file, handle_completed)));
        let mut client = TcpClient::new(addr, tcp_handler);
        client.start();
        client.send_message(Message::Auth(cfg.secret));
        loop {
            if completed.load(Ordering::Relaxed) {
                break;
            }
        }
        info!("end backup_file_to_backer_server");
    }

    async fn backup_file_to_qiniu(cfg: QiniuServer, archive_file: file::FileInfo) {
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
    async fn backup_file_to_aliyun_oss(_cfg: AliyunOssServer, _archive_file: file::FileInfo) {
        info!("start backup_file_to_aliyun_oss");
        info!("end backup_file_to_aliyun_oss");
    }

    // TODO
    async fn backup_file_to_tencent_oss(_cfg: TencentOssServer, _archive_file: file::FileInfo) {
        info!("start backup_file_to_tencent_oss");
        info!("end backup_file_to_tencent_oss");
    }
}


struct BackerHandle {
    archive_file: file::FileInfo,
    completed: Arc<AtomicBool>,
}

impl BackerHandle {
    pub fn new(archive_file: file::FileInfo, completed: Arc<AtomicBool>) -> Self {
        Self { archive_file, completed }
    }
}

impl Handler for BackerHandle {
    fn handel(&self, message: &Message, protocol: &mut Protocol) {
        match message {
            Message::Phrase(echo) => {
                info!("receive phrase message: {}", echo);
            }
            Message::Authorize(authorize) => {
                if *authorize {
                    info!("Authorize success, start sync file.");
                    let fb = FileBuffer::new(self.archive_file.file_name.clone(), self.archive_file.file_data.to_vec());
                    let fb_size = fb.get_buffer_length() as f64;
                    let mut completed_buf_size: f64 = 0.0;
                    let buffers = fb.cut_file_buff(MAX_BUFFER_LENGTH);
                    for buffer in buffers {
                        completed_buf_size += buffer.get_buffer_length() as f64;
                        let msg = Message::FileBuffer(buffer);
                        let res = protocol.send_message(msg);
                        if let Err(e) = res {
                            error!("send file buffer failed: {}", e);
                            return;
                        }
                        let percents = format!("{:.0}", (completed_buf_size / fb_size) * 100.0);
                        print!("\rback up file: {}%", percents);
                    }
                    println!();
                    info!("end sync file.");
                } else {
                    error!("Authorize failed!");
                }
                self.completed.store(true, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

