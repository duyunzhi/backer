use std::{fs, io};
use std::error::Error;
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::{WalkDir};
use zip::write::FileOptions;

use crate::consts;
use crate::errors::CustomError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub file_name: String,
    pub file_data: Box<Vec<u8>>,
}

impl FileInfo {
    pub fn new(file_name: String, file_data: Box<Vec<u8>>) -> Self {
        Self {
            file_name: file_name.clone(),
            file_data: file_data.clone(),
        }
    }
}

impl Default for FileInfo {
    fn default() -> Self {
        Self {
            file_name: String::from(""),
            file_data: Box::new(vec![]),
        }
    }
}

pub enum CompressType {
    Zip,
    Tar,
}

pub fn is_exist<P: AsRef<Path>>(path: P) -> bool {
    Path::new(path.as_ref()).exists()
}

pub fn is_dir<P: AsRef<Path>>(path: P) -> bool {
    Path::new(path.as_ref()).is_dir()
}

pub fn is_file<P: AsRef<Path>>(path: P) -> bool {
    Path::new(path.as_ref()).is_file()
}

pub fn is_empty_dir<P: AsRef<Path>>(path: P) -> bool {
    let mut count = 0;
    let walk_dir = WalkDir::new(path);
    for _ in walk_dir {
        count += 1;
    }
    if count > 1 {
        return false;
    }
    true
}

pub fn get_file_name<P: AsRef<Path>>(path: P) -> Result<String, Box<dyn Error>> {
    if !is_file(path.as_ref()) {
        Err(Box::new(CustomError::new(format!("path [{}] is not a file", path.as_ref().to_str().unwrap()))))
    } else {
        Ok(Path::new(path.as_ref()).file_name().unwrap().to_str().unwrap().to_string())
    }
}

pub fn create_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    fs::create_dir_all(path.as_ref())
}

pub fn read_file<P: AsRef<Path>>(path: P) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut file = File::open(path)?;
    let _ = file.read_to_end(&mut buffer);
    Ok(buffer)
}

pub fn create_write_file<P: AsRef<Path>>(path: P, buf: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut file = File::create(path)?;
    let _ = file.write_all(buf)?;
    Ok(())
}

pub fn read_file_info<P: AsRef<Path>>(path: P) -> Result<FileInfo, Box<dyn Error>> {
    let mut file_info = FileInfo::default();
    let file_bytes = read_file(path.as_ref());
    match file_bytes {
        Ok(file_bytes) => {
            let file_name = get_file_name(path.as_ref());
            match file_name {
                Ok(file_name) => {
                    file_info.file_name = file_name;
                    file_info.file_data = Box::new(file_bytes);
                }
                Err(e) => {
                    println!("Read file [{}] failed: {}", path_to_string(path), e);
                }
            }
        }
        Err(e) => {
            println!("Read file [{}] failed: {}", path_to_string(path), e);
        }
    }
    Ok(file_info)
}

pub fn rm_file<P: AsRef<Path>>(path: P) -> io::Result<()> {
    fs::remove_file(path)
}

fn path_to_string<P: AsRef<Path>>(path: P) -> String {
    path.as_ref().to_str().unwrap().to_string()
}

pub fn get_archive_dir_path() -> PathBuf {
    let user_home_dir = home::home_dir().unwrap();
    user_home_dir.join(consts::ARCHIVE_DIR_SUFFIX)
}

pub fn compress_files<P: AsRef<Path>>(paths: Box<Vec<P>>, target: P, compress_type: &CompressType) -> Result<(), Box<dyn Error>> {
    let mut walk_dirs: Box<Vec<WalkDir>> = Box::new(Vec::new());
    for path in paths.into_iter() {
        let walk_dir = WalkDir::new(path.as_ref());
        walk_dirs.push(walk_dir);
    }
    let compress_file = File::create(target.as_ref())?;
    match compress_type {
        CompressType::Zip => zip_compress(walk_dirs, compress_file)?,
        CompressType::Tar => tar_compress(walk_dirs, compress_file)?,
    }
    Ok(())
}

fn zip_compress<T>(its: Box<Vec<WalkDir>>, writer: T) -> zip::result::ZipResult<()> where T: Write + Seek {
    let mut zip = zip::ZipWriter::new(writer);
    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Bzip2)
        .unix_permissions(0o755);
    let mut buffer = Vec::new();
    for walk_dir in its.into_iter() {
        let it = &mut walk_dir.into_iter().filter_map(|e| e.ok());
        for entry in it {
            let path = entry.path();
            let prefix = path.parent().map_or_else(|| "/", |p| p.to_str().unwrap());
            let name = path.strip_prefix(Path::new(prefix)).unwrap();
            let parent = path.parent().unwrap();
            let name = parent.strip_prefix(parent.parent().unwrap()).unwrap().join(name);
            if path.is_file() {
                // println!("adding file {:?} as {:?} ...", path, name);
                zip.start_file(name.to_string_lossy(), options)?;
                let mut f = File::open(path)?;
                f.read_to_end(&mut buffer)?;
                zip.write_all(&buffer)?;
                buffer.clear();
            }
        }
    }

    zip.finish()?;
    Ok(())
}

fn tar_compress<T>(_its: Box<Vec<WalkDir>>, _writer: T) -> zip::result::ZipResult<()> where T: Write + Seek {
    todo!()
}