use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    net::TcpStream,
    sync::{Arc, Mutex, RwLock},
    thread,
};

use bus::Bus;
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
    state: Server,
    is_alive: bool,
    receiver: bus::BusReader<MutexID>,
}

impl Connection {
    fn new(state: Server, id: usize, stream: TcpStream, receiver: bus::BusReader<MutexID>) -> Self {
        Self {
            id,
            stream,
            state,
            is_alive: true,
            locks: Default::default(),
            receiver,
        }
    }
    fn cycle(&mut self) {
        while self.is_alive {
            let mut buffer = [0; 1024];
            let count = self.stream.read(&mut buffer).unwrap();
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
    fn lock(&mut self, mutex_id: &MutexID) -> bool {
        if self.locks.contains(mutex_id) {
            return true;
        }

        loop {
            // Try to acquire the lock
            let ok = self.state.lock(self.id, mutex_id);
            if ok.is_ok() {
                self.locks.insert(mutex_id.clone());
                break;
            }

            // If mutex is already locked by someone else, wait for the release signal
            loop {
                let released_mutex = self.receiver.recv().unwrap();
                if &released_mutex == mutex_id {
                    break;
                }
            }
        }
        true
    }
}

type MutexID = String;

#[derive(Clone)]
struct Server {
    mutex_to_conn: Arc<RwLock<HashMap<MutexID, ConnectionID>>>,
    next_id: Arc<Mutex<ConnectionID>>,
    bus: Arc<Mutex<Bus<MutexID>>>,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            next_id: Arc::new(Mutex::new(1)),
            mutex_to_conn: Default::default(),
            bus: Arc::new(Mutex::new(Bus::new(1000))),
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

            // println!(
            //     "New connection [{}] from {}",
            //     id,
            //     stream.peer_addr().unwrap()
            // );
            let mut conn = Connection::new(state, id, stream, self.bus.lock().unwrap().add_rx());

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
        self.bus.lock().unwrap().broadcast(mutex.clone());
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
