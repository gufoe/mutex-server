use std::{
    collections::{HashMap, HashSet},
    io::Read,
    net::TcpStream,
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use structopt::StructOpt;

type ConnectionID = usize;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "command", content = "params")]
enum Command {
    Lock {
        id: MutexID,
        timeout_ms: Option<usize>,
    },
    Release {
        id: MutexID,
    },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "command", content = "params")]
enum Response {
    LockResponse { id: MutexID, success: bool },
    ReleaseResponse { id: MutexID, success: bool },
}
struct Connection {
    pub id: ConnectionID,
    pub stream: TcpStream,
    pub locks: HashSet<MutexID>,
    state: ServerState,
    is_alive: bool,
}

impl Connection {
    fn new(state: ServerState, id: usize, stream: TcpStream) -> Self {
        Self {
            id,
            stream,
            state,
            is_alive: true,
            locks: Default::default(),
        }
    }
    fn cycle(&mut self) {
        while self.is_alive {
            let mut buffer = [0; 1024];
            let count = self.stream.read(&mut buffer).unwrap();
            let command = serde_json::from_slice::<Command>(&buffer[0..count]);
            match command {
                Ok(command) => {
                    println!("Got command: {:?}", command);
                    match command {
                        Command::Lock { id, timeout_ms } => {
                            if timeout_ms.is_some() {
                                eprintln!("Timeout is not implemented yet");
                                self.send(&Response::LockResponse { id, success: false });
                            } else {
                                let success = self.lock(&id);
                                self.send(&Response::LockResponse { id, success });
                            }
                        }
                        Command::Release { id } => {
                            let success = self.release(&id);
                            self.send(&Response::ReleaseResponse { id, success });
                        }
                    }
                }
                Err(e) => {
                    self.is_alive = false;
                    eprintln!("Cannot deserialize: {:?}", e);
                }
            }
        }
        self.release_all();
    }
    fn send<T: ?Sized + Serialize>(&mut self, payload: &T) -> bool {
        let result = serde_json::to_writer(&mut self.stream, payload);
        match result {
            Ok(_) => true,
            Err(e) => {
                eprintln!("Cannot send response: {:?}", e);
                self.is_alive = false;
                false
            }
        }
    }
    fn release_all(&mut self) {
        self.locks.iter().for_each(|mutex_id| {
            self.state.release(mutex_id);
        });
        self.locks.clear();
    }
    fn release(&mut self, mutex_id: &MutexID) -> bool {
        if !self.locks.contains(mutex_id) {
            false
        } else {
            self.locks.remove(mutex_id);
            self.state.release(mutex_id);
            true
        }
    }
    fn lock(&mut self, mutex_id: &MutexID) -> bool {
        if self.locks.contains(mutex_id) {
            return true;
        }
        loop {
            let ok = self.state.lock(self.id, mutex_id);
            if ok.is_ok() {
                self.locks.insert(mutex_id.clone());
                break;
            } else {
                thread::sleep(Duration::from_millis(10));
            }
        }
        true
    }
}

type MutexID = String;
#[derive(Default, Clone)]
struct ServerState {
    mutex_to_conn: Arc<RwLock<HashMap<MutexID, ConnectionID>>>,
}
impl ServerState {
    pub fn lock(&self, conn: ConnectionID, mutex: &MutexID) -> Result<(), ()> {
        let mut locks = self.mutex_to_conn.write().unwrap();
        if locks.contains_key(mutex) {
            Err(())
        } else {
            locks.insert(mutex.clone(), conn);
            Ok(())
        }
    }
    pub fn release(&self, mutex: &MutexID) -> bool {
        let mut locks = self.mutex_to_conn.write().unwrap();
        if !locks.contains_key(mutex) {
            false
        } else {
            locks.remove(mutex);
            true
        }
    }
}

struct Server {
    next_id: ConnectionID,
    state: ServerState,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            next_id: 1,
            state: ServerState::default(),
        }
    }
}
impl Server {
    fn serve(&mut self, bind: &str) {
        use std::net::TcpListener;

        let listener = TcpListener::bind(bind).unwrap();

        for stream in listener.incoming() {
            let stream = stream.unwrap();
            println!("Connection established! {}", stream.peer_addr().unwrap());
            let state = self.state.clone();
            let id = self.next_id;
            self.next_id += 1;
            let mut conn = Connection::new(state, id, stream);
            thread::spawn(move || {
                conn.cycle();
            });
        }
    }
}



#[derive(Debug, StructOpt)]
#[structopt(name = "Mutex Server", about = "Starts a mutex server.")]
struct Opt {
    #[structopt(short, long)]
    bind: String,
}

fn main() {
    let opt = Opt::from_args();

    println!("Starting server {}", &opt.bind);
    let mut server = Server::default();
    server.serve(&opt.bind);
}
