use std::fs;
use std::net::{IpAddr, ToSocketAddrs};
use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

use crate::consts;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("backup-files is empty")]
    BackupFilesEmpty,
    #[error("job-cron is empty")]
    JobCronEmpty,
    #[error("target is empty")]
    TargetEmpty,
    #[error("backer server ip invalid")]
    BackerServerIpInvalid,
    #[error("runtime config invalid: {0}")]
    RuntimeConfigInvalid(String),
    #[error("yaml config invalid: {0}")]
    YamlConfigInvalid(String),
    #[error("qiniu access key is empty")]
    QiniuAccessKeyEmpty,
    #[error("qiniu secret key is empty")]
    QiniuSecretKeyEmpty,
    #[error("qiniu bucket name is empty")]
    QiniuBucketNameEmpty,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, rename_all = "kebab-case")]
pub struct BackerConfig {
    pub backup_files: Vec<String>,
    pub compress_mode: String,
    pub archive_prefix: String,
    pub job_cron: String,
    pub backup_target: Vec<String>,
    pub backer_server: BackerServer,
    pub qiniu: QiniuServer,
    pub aliyun_oss: AliyunOssServer,
    pub tencent_oss: TencentOssServer,
}

impl BackerConfig {
    pub fn load_from_file<T: AsRef<Path>>(path: T) -> Result<Self, ConfigError> {
        let contents =
            fs::read_to_string(path).map_err(|e| ConfigError::YamlConfigInvalid(e.to_string()))?;
        Self::load(&contents)
    }

    pub fn load<C: AsRef<str>>(contents: C) -> Result<Self, ConfigError> {
        let contents = contents.as_ref();
        if contents.len() == 0 {
            // parsing empty string leads to EOF error
            Ok(Self::default())
        } else {
            let mut cfg: Self = serde_yaml::from_str(contents)
                .map_err(|e| ConfigError::YamlConfigInvalid(e.to_string()))?;
            if cfg.backup_target.len() <= 0 {
                return Err(ConfigError::TargetEmpty);
            }
            if cfg.compress_mode.len() == 0 {
                cfg.compress_mode = consts::COMPRESS_MODE_ZIP.to_string();
            }
            if cfg.archive_prefix.len() == 0 {
                cfg.archive_prefix = consts::DEFAULT_ARCHIVE_PREFIX.to_string();
            }
            if cfg.job_cron.len() == 0 {
                cfg.job_cron = consts::DEFAULT_CRON.to_string();
            }
            for i in 0..cfg.backup_target.len() {
                if cfg.backup_target[i] == consts::TARGET_BACKER_SERVER {
                    if cfg.backer_server.ip.parse::<IpAddr>().is_err() {
                        let ip = resolve_domain(&cfg.backer_server.ip);
                        if ip.is_none() {
                            return Err(ConfigError::BackerServerIpInvalid);
                        }
                    }
                } else if cfg.backup_target[i] == consts::TARGET_QINIU {
                    if cfg.qiniu.access_key.len() == 0 {
                        return Err(ConfigError::QiniuAccessKeyEmpty);
                    }
                    if cfg.qiniu.secret_key.len() == 0 {
                        return Err(ConfigError::QiniuSecretKeyEmpty);
                    }
                    if cfg.qiniu.bucket_name.len() == 0 {
                        return Err(ConfigError::QiniuBucketNameEmpty);
                    }
                }
            }

            Ok(cfg)
        }
    }
}

impl Default for BackerConfig {
    fn default() -> Self {
        Self {
            backup_files: vec![],
            compress_mode: String::from("tar.gz"),
            archive_prefix: String::from("Archive"),
            job_cron: String::from("0 0 0 * * *"),
            backup_target: vec![],
            backer_server: BackerServer::default(),
            qiniu: QiniuServer::default(),
            aliyun_oss: AliyunOssServer::default(),
            tencent_oss: TencentOssServer::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, rename_all = "kebab-case")]
pub struct BackerServer {
    pub ip: String,
    pub port: u16,
}

impl Default for BackerServer {
    fn default() -> Self {
        Self {
            ip: String::from(""),
            port: 0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, rename_all = "kebab-case")]
pub struct QiniuServer {
    pub access_key: String,
    pub secret_key: String,
    pub bucket_name: String,
}

impl Default for QiniuServer {
    fn default() -> Self {
        Self {
            access_key: String::from(""),
            secret_key: String::from(""),
            bucket_name: String::from(""),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, rename_all = "kebab-case")]
pub struct AliyunOssServer {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket_name: String,
}

impl Default for AliyunOssServer {
    fn default() -> Self {
        Self {
            endpoint: String::from(""),
            access_key: String::from(""),
            secret_key: String::from(""),
            bucket_name: String::from(""),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, rename_all = "kebab-case")]
pub struct TencentOssServer {}

impl Default for TencentOssServer {
    fn default() -> Self {
        Self {}
    }
}

// resolve domain name (without port) to ip address
fn resolve_domain(addr: &str) -> Option<String> {
    match format!("{}:1", addr).to_socket_addrs() {
        Ok(mut addr) => match addr.next() {
            Some(addr) => Some(addr.ip().to_string()),
            None => None,
        },
        Err(e) => {
            eprintln!("{:?}", e);
            None
        }
    }
}