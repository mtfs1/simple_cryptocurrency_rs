use std::io::{Result, Error, ErrorKind, Write, Read};
use std::net::{TcpListener, TcpStream, ToSocketAddrs, IpAddr};
use std::sync::Mutex;
use std::thread;

use crate::networking::message::{MessageHeader, MessageType};


pub struct NetworkInterface {
    peers: Mutex<Vec<TcpStream>>
}

impl NetworkInterface {
    pub fn new() -> Self {
        NetworkInterface {
            peers: Mutex::new(Vec::new())
        }
    }

    pub fn connect_to_peer(&self, ip: IpAddr) -> Result<()> {
        let mut conn = TcpStream::connect(format!("{ip}:1234"))?;

        MessageHeader::new()
            .set_type(MessageType::StartPeering)
            .send_to(&mut conn)?;

        let res = MessageHeader::receive_from(&mut conn)?;

        if res.is_ack() {
            self.add_peer(conn);
            return Ok(());
        }

        Err(Error::new(
            ErrorKind::PermissionDenied,
            "Node did not acknoledge peering"
        ))
    }

    pub fn listen_for_connections(&self) {
        let listener = TcpListener::bind("0.0.0.0:1234").unwrap();
        for conn in listener.incoming() {
            let mut conn = {
                match conn {
                    Ok(val) => val,
                    Err(_) => continue
                }
            };

            let message = match MessageHeader::receive_from(&mut conn) {
                Ok(val) => val,
                Err(_) => continue
            };

            if let MessageType::StartPeering = message.message_type {
                let res = MessageHeader::new()
                    .set_type(MessageType::Ack)
                    .send_to(&mut conn);

                if let Err(_) = res {
                    continue;
                }

                self.add_peer(conn.try_clone().unwrap());
            }
        }
    }

    fn add_peer(&self, conn: TcpStream) {
        println!("[ADDED PEER][{}:{}]",
            conn.peer_addr().unwrap().ip(),
            conn.peer_addr().unwrap().port());
        self.peers.lock().unwrap().push(conn.try_clone().unwrap());

        thread::spawn(|| listen_to_messages(conn));
    }
}

fn listen_to_messages(conn: TcpStream) -> Result<()> {
    let mut conn = conn;
    loop {
        let message = MessageHeader::receive_from(&mut conn)?;

        println!("[{}:{}][MESSAGE]",
            conn.peer_addr().unwrap().ip(),
            conn.peer_addr().unwrap().port()
        );
    }
}

pub fn resolve_address(address: &str) -> Result<IpAddr> {
    let mut address = address.trim().to_owned();

    let mut iter = address.split(":");
    let _ = iter.next();
    if let None = iter.next() {
        address = format!("{address}:1234");
    }

    let resolved_addresses = address.to_socket_addrs()?;

    if let Some(addr) = resolved_addresses.map(|a| a.ip()).next() {
        return Ok(addr);
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::AddrNotAvailable,
        "Could not resolve to a valid IP address",
    ))
}

