use std::collections::HashMap;
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use log::{error, info};
use tokio::runtime::{Builder, Runtime};
use tokio::task::JoinHandle;

use crate::packet::message::{Message, Protocol};

const READ_TIMEOUT: Duration = Duration::from_secs(6);
const WRITE_TIMEOUT: Duration = Duration::from_secs(6);

pub trait Handler: Send + Sync {
    fn handel(&self, message: &Message, protocol: &mut Protocol);
}

pub enum PacketType {
    Server,
    Client,
}

pub struct TcpHandler {
    running: Arc<AtomicBool>,
    handler: Arc<Mutex<HashMap<String, Box<dyn Handler>>>>,
}

impl TcpHandler {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            handler: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_handle(&self, handle_name: String, handle: Box<dyn Handler>) {
        self.handler.lock().unwrap().insert(handle_name, handle);
    }

    pub fn remove_handle(&self, handle_name: &str) {
        self.handler.lock().unwrap().remove(handle_name);
    }

    pub fn handle_message(&self, tcp_stream: TcpStream) {
        let mut protocol = Protocol::with_stream(tcp_stream).unwrap();
        while self.running.load(Ordering::Relaxed) {
            let message = protocol.read_message();
            match message {
                Ok(message) => {
                    for (_, h) in self.handler.lock().unwrap().iter() {
                        h.handel(&message, &mut protocol);
                    }
                }
                Err(e) => {
                    match e.kind() {
                        io::ErrorKind::WouldBlock => {
                            continue;
                        }
                        _ => {
                            break;
                        }
                    }
                }
            }
        }
    }
}

pub struct TcpServer {
    addr: Arc<SocketAddr>,
    running: Arc<AtomicBool>,
    rt: Runtime,
    threads: Mutex<Vec<JoinHandle<()>>>,
    handler: Arc<TcpHandler>,
}

impl TcpServer {
    pub fn new(addr: SocketAddr, mut handler: TcpHandler) -> Self {
        let running = Arc::new(AtomicBool::new(false));
        handler.running = running.clone();
        Self {
            addr: Arc::new(addr),
            running,
            rt: Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap(),
            threads: Default::default(),
            handler: Arc::new(handler),
        }
    }

    fn run(&self) {
        let running = self.running.clone();
        let addr = self.addr.clone();

        let listener = TcpListener::bind(addr.as_ref());
        match listener {
            Ok(listener) => {
                for stream in listener.incoming() {
                    if !running.load(Ordering::Relaxed) {
                        break;
                    }
                    let handler = self.handler.clone();
                    match stream {
                        Ok(stream) => {
                            let peer_addr = stream.peer_addr().expect("stream has peer_addr");
                            info!("tcp client incoming socketAddr: [{:?}]", peer_addr);
                            stream.set_read_timeout(Some(READ_TIMEOUT)).unwrap();
                            stream.set_write_timeout(Some(WRITE_TIMEOUT)).unwrap();
                            self.threads.lock().unwrap().push(self.rt.spawn(async move {
                                handler.handle_message(stream);
                            }));
                        }
                        Err(e) => {
                            error!("parsing stream error: {}", e)
                        }
                    }
                }
            }
            Err(e) => {
                running.store(false, Ordering::Relaxed);
                error!("start tcp server failed: {}", e);
            }
        }
    }

    /// will block
    pub fn start(&self) {
        if self.running.swap(true, Ordering::Relaxed) {
            return;
        }
        self.run();
    }

    pub fn stop(&self) {
        if !self.running.swap(false, Ordering::Relaxed) {
            return;
        }
        self.rt.block_on(async move {
            for t in self.threads.lock().unwrap().drain(..) {
                let _ = t.await;
            }
        });
    }
}

pub struct TcpClient {
    addr: Arc<SocketAddr>,
    running: Arc<AtomicBool>,
    rt: Runtime,
    threads: Mutex<Vec<JoinHandle<()>>>,
    handler: Arc<TcpHandler>,
    stream: Option<TcpStream>,
}

impl TcpClient {
    pub fn new(addr: SocketAddr, mut handler: TcpHandler) -> Self {
        let running = Arc::new(AtomicBool::new(false));
        handler.running = running.clone();
        Self {
            addr: Arc::new(addr),
            running,
            rt: Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .unwrap(),
            threads: Default::default(),
            handler: Arc::new(handler),
            stream: None,
        }
    }

    fn run(&mut self) {
        let addr = self.addr.clone();
        let stream = TcpStream::connect(addr.as_ref());

        match stream {
            Ok(stream) => {
                self.stream.replace(stream.try_clone().unwrap());
                let handler = self.handler.clone();
                self.threads.lock().unwrap().push(self.rt.spawn(async move {
                    handler.handle_message(stream);
                }));
            }
            Err(e) => {
                error!("parsing stream error: {}", e)
            }
        }
    }

    pub fn send_message(&self, message: Message) {
        match &self.stream {
            Some(s) => {
                let mut s = s.try_clone().unwrap();
                message.write_message(&mut s).unwrap();
            }
            None => {}
        }
    }

    pub fn start(&mut self) {
        if self.running.swap(true, Ordering::Relaxed) {
            return;
        }
        self.run();
    }

    pub fn stop(&self) {
        if !self.running.swap(false, Ordering::Relaxed) {
            return;
        }
        self.rt.block_on(async move {
            for t in self.threads.lock().unwrap().drain(..) {
                let _ = t.await;
            }
        });
    }

    pub fn send_one(addr: SocketAddr, message: Message) {
        let stream = TcpStream::connect(addr);
        match stream {
            Ok(mut stream) => {
                message.write_message(&mut stream).unwrap();
            }
            Err(e) => {
                error!("parsing stream error: {}", e)
            }
        }
    }
}