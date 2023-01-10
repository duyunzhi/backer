use std::{io::{self, Read, Write}, thread, time};
use std::net::{TcpListener, TcpStream};

use crate::utils::file;

pub struct Server {
    port: String,
    backer_dir: String,
}

impl Server {
    pub fn new(port: String, backup_dir: String) -> Self {
        Self {
            port: port.clone(),
            backer_dir: backup_dir.clone(),
        }
    }

    pub fn start(&self) -> io::Result<()> {
        self.init_backup_dir();
        self.run()
    }

    fn init_backup_dir(&self) {
        if !file::is_exist(&self.backer_dir) {
            // create backup dir
            let res = file::create_dir(&self.backer_dir);
            match res {
                Ok(_) => {}
                Err(e) => panic!("Create backup dir error: {}", e),
            }
        } else {
            if !file::is_dir(&self.backer_dir) {
                panic!("Backer dir path is not dir")
            }
        }
    }

    fn run(&self) -> io::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:8080")?;
        //定义一个listener，bind函数里面填写的是监听的的ip与端口号,?是一种简写，等价于except,unwrap
        let mut thread_vec: Vec<thread::JoinHandle<()>> = Vec::new();
        //创建一个容器，用来放线程的句柄

        for stream in listener.incoming() {
            println!("tcp incoming");
            let stream = stream.expect("failed");
            //转换一下stream流，出现问题，提示“失败”，没有问题，继续下面的操作
            let backer_dir = self.backer_dir.clone();
            let handle = thread::spawn(move || {
                Self::handle_client(backer_dir, stream).unwrap_or_else(|error| eprintln!("{:?}", error));
            });
            //对输入的每一个流来创建一个线程，利用必包进行一个处理
            thread_vec.push(handle);
            //把handle加到容器里面
        }

        for handle in thread_vec {
            //此循环为了等待线程的结束
            handle.join().unwrap();
            //等待结束的具体实现
        }
        Ok(())
    }

    fn handle_client(backer_dir: String, mut stream: TcpStream) -> io::Result<()> {
        let mut buffer: Vec<u8> = Vec::new();
        let n = stream.read_to_end(&mut buffer).unwrap();
        let file_info: file::FileInfo = bincode::deserialize(&buffer.as_slice()).unwrap();
        println!("receive file: {:?}", file_info);
        let path = format!("{}/{}", backer_dir, file_info.file_name);
        let res = file::create_write_file(path, file_info.file_data.as_slice());
        Ok(())
    }
}