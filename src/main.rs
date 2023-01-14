use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    net::TcpStream,
    sync::{Arc, Mutex, RwLock},
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use structopt::StructOpt;

type ConnectionID = usize;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "command", content = "params")]
enum Command {
    Lock {
        id: MutexID,
        timeout_ms: Option<u64>,
    },
    Check {
        id: MutexID,
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
    CheckResponse { id: MutexID, is_locked: bool },
}
struct Connection {
    pub id: ConnectionID,
    pub stream: TcpStream,
    pub locks: HashSet<MutexID>,
    state: Server,
    is_alive: bool,
}

impl Connection {
    fn new(state: Server, id: usize, stream: TcpStream) -> Self {
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
            let count = self.stream.read(&mut buffer).unwrap_or(0);
            if count == 0 {
                // println!("Connection closed");
                break;
            }
            let command = serde_json::from_slice::<Command>(&buffer[0..count]);
            match command {
                Ok(command) => {
                    // println!("Got command: {:?}", command);
                    match command {
                        Command::Lock { id, timeout_ms } => {
                            let success = self.lock(&id, timeout_ms);
                            self.send(&Response::LockResponse { id, success });
                        }
                        Command::Release { id } => {
                            let success = self.release(&id);
                            self.send(&Response::ReleaseResponse { id, success });
                        }
                        Command::Check { id } => {
                            self.send(&Response::CheckResponse {
                                is_locked: self.locks.contains(&id),
                                id,
                            });
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
        let payload = serde_json::to_string(payload).unwrap() + "\n";

        self.stream.write(payload.as_bytes()).unwrap();
        let result = self.stream.flush();

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
    fn lock(&mut self, mutex_id: &MutexID, timeout_ms: Option<u64>) -> bool {
        if self.locks.contains(mutex_id) {
            return true;
        }

        let t_start = Instant::now();
        let timeout = if let Some(ms) = timeout_ms {
            Some(Duration::from_millis(ms))
        } else {
            None
        };
        loop {
            // Try to acquire the lock
            let ok = self.state.lock(self.id, mutex_id);
            if ok.is_ok() {
                self.locks.insert(mutex_id.clone());
                return true;
            }

            // Check if timeout has elapsed
            if let Some(t) = timeout {
                if t_start.elapsed() >= t {
                    return false;
                }
            }

            // Sleep to prevent busy waiting to hog the cpu
            thread::sleep(Duration::from_millis(10));
        }
    }
}

type MutexID = String;

#[derive(Clone)]
struct Server {
    mutex_to_conn: Arc<RwLock<HashMap<MutexID, ConnectionID>>>,
    next_id: Arc<Mutex<ConnectionID>>,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            next_id: Arc::new(Mutex::new(1)),
            mutex_to_conn: Default::default(),
        }
    }
}
impl Server {
    fn generate_id(&self) -> usize {
        let mut guard = self.next_id.lock().unwrap();
        if *guard == usize::MAX {
            *guard = 1;
        } else {
            *guard += 1;
        }
        *guard
    }
    fn serve(&mut self, bind: &str) {
        use std::net::TcpListener;

        let listener = TcpListener::bind(bind).unwrap();

        for stream in listener.incoming() {
            let stream = stream.unwrap();
            stream.set_nodelay(true).unwrap();
            let state = self.clone();

            let id = self.generate_id();

            let mut conn = Connection::new(state, id, stream);

            thread::spawn(move || conn.cycle());
        }
    }

    pub fn lock(&self, conn: ConnectionID, mutex: &MutexID) -> Result<(), ()> {
        let mut locks = self.mutex_to_conn.write().unwrap();
        if locks.contains_key(mutex) {
            Err(())
        } else {
            locks.insert(mutex.clone(), conn);
            println!("{} active, acquired [{}]", locks.len(), mutex);
            Ok(())
        }
    }
    pub fn release(&self, mutex: &MutexID) -> bool {
        let mut locks = self.mutex_to_conn.write().unwrap();
        locks.remove(mutex);
        println!("{} active, released [{}]", locks.len(), mutex);
        true
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
