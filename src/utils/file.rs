use std::{fs, io};
use std::error::Error;
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use flate2::Compression;
use flate2::write::GzEncoder;

use serde::{Deserialize, Serialize};
use walkdir::{WalkDir};
use zip::write::FileOptions;

use crate::consts;
use crate::errors::CustomError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub file_name: String,
    pub absolute_path: String,
    pub file_data: Box<Vec<u8>>,
}

impl FileInfo {
    pub fn new(file_name: String, absolute_path: String, file_data: Box<Vec<u8>>) -> Self {
        Self {
            file_name,
            absolute_path,
            file_data: file_data.clone(),
        }
    }
}

impl Default for FileInfo {
    fn default() -> Self {
        Self {
            file_name: String::from(""),
            absolute_path: String::from(""),
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

pub fn create_file<P: AsRef<Path>>(path: P) -> Result<File, Box<dyn Error>> {
    let file = File::create(path)?;
    Ok(file)
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
                    file_info.absolute_path = path.as_ref().to_string_lossy().to_string();
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

pub fn read_file_info_without_file_data<P: AsRef<Path>>(path: P) -> Result<FileInfo, Box<dyn Error>> {
    let file_name = get_file_name(path.as_ref())?;
    Ok(FileInfo::new(file_name, path.as_ref().to_string_lossy().to_string(), Box::new(vec![])))
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

pub fn compress_files<P: AsRef<Path>>(paths: Vec<P>, target: P, compress_type: CompressType) -> Result<(), Box<dyn Error>> {
    let compress_file = File::create(target.as_ref())?;
    match compress_type {
        CompressType::Zip => zip_compress(paths, compress_file)?,
        CompressType::Tar => tar_compress(paths, compress_file)?,
    }
    Ok(())
}

fn zip_compress<P: AsRef<Path>, T>(paths: Vec<P>, writer: T) -> io::Result<()> where T: Write + Seek {
    let mut zip_writer = zip::ZipWriter::new(writer);
    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Bzip2)
        .unix_permissions(0o755);
    for src_path in paths.into_iter() {
        if Path::new(src_path.as_ref()).is_dir() {
            let walk_dir = WalkDir::new(src_path.as_ref());
            let mut buffer = Vec::new();
            let it = &mut walk_dir.into_iter().filter_map(|e| e.ok());
            for entry in it {
                let path = entry.path();
                let tail = Path::new(src_path.as_ref()).file_name().unwrap();
                let name = path.strip_prefix(Path::new(src_path.as_ref())).unwrap();

                let prefix: String;
                if tail.is_empty() {
                    prefix = format!("archive/{}", name.to_str().unwrap());
                } else {
                    prefix = format!("archive/{}/{}", tail.to_str().unwrap(), name.to_str().unwrap());
                }
                let archive = Path::new(prefix.as_str());
                if path.is_file() {
                    #[allow(deprecated)]
                    zip_writer.start_file_from_path(archive, options)?;
                    let mut f = File::open(path)?;

                    f.read_to_end(&mut buffer)?;
                    zip_writer.write_all(&*buffer)?;
                    buffer.clear();
                } else {
                    #[allow(deprecated)]
                    zip_writer.add_directory_from_path(archive, options)?;
                }
            }
        } else if Path::new(src_path.as_ref()).is_file() {
            let path = Path::new(src_path.as_ref());
            let file_name = get_file_name(path).unwrap();
            let prefix = format!("archive/{}", file_name);
            let archive = Path::new(prefix.as_str());

            let mut buffer = Vec::new();
            #[allow(deprecated)]
            zip_writer.start_file_from_path(archive, options)?;
            let mut f = File::open(path)?;

            f.read_to_end(&mut buffer)?;
            zip_writer.write_all(&*buffer)?;
        }
    }
    zip_writer.finish()?;
    Ok(())
}

fn tar_compress<P: AsRef<Path>, T>(paths: Vec<P>, writer: T) -> io::Result<()> where T: Write + Seek {
    let enc = GzEncoder::new(writer, Compression::default());
    let mut tar = tar::Builder::new(enc);

    for path in paths.into_iter() {
        if is_dir(path.as_ref()) {
            let p = Path::new(path.as_ref());
            let suffix_path = p.file_name().unwrap().to_str().unwrap().to_string();
            tar.append_dir_all(format!("archive/{}", suffix_path), path)?;
        } else if is_file(path.as_ref()) {
            let file_name = get_file_name(path.as_ref()).unwrap();
            let mut f = File::open(path.as_ref()).unwrap();
            tar.append_file(format!("archive/{}", file_name), &mut f)?;
        }
    }
    tar.finish()?;
    Ok(())
}