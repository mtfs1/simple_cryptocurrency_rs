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

    pub fn ask_for_peers(&self, ip: IpAddr) -> Result<Vec<IpAddr>> {
        let mut conn = TcpStream::connect(format!("{ip}:1234"))?;

        MessageHeader::new()
            .set_type(MessageType::ListPeers)
            .send_to(&mut conn)?;

        let res = MessageHeader::receive_from(&mut conn)?;

        if !res.is_ack() {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                "Node did not send peer list"
            ));
        }

        let mut num_peers = [0u8];
        conn.read_exact(&mut num_peers)?;
        let num_peers = num_peers[0];

        let mut peers = Vec::<IpAddr>::new();
        for _ in 0..num_peers {
            let mut ip_ver = [0u8];
            conn.read_exact(&mut ip_ver)?;
            let ip_ver = ip_ver[0];

            if ip_ver == 4 {
                let mut ip = [0u8; 4];
                conn.read_exact(&mut ip)?;

                let ip_addr = IpAddr::from(ip);
                peers.push(ip_addr);
            }

            if ip_ver == 6 {
                let mut ip = [0u8; 16];
                conn.read_exact(&mut ip)?;

                let ip_addr = IpAddr::from(ip);
                peers.push(ip_addr);
            }
        }

        Ok(peers)
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

            if let MessageType::ListPeers = message.message_type {
                let res = MessageHeader::new()
                    .set_type(MessageType::Ack)
                    .send_to(&mut conn);

                if let Err(_) = res {
                    continue;
                }

                if let Err(_) = self.list_peers(&mut conn) {
                    continue;
                }
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

    fn list_peers(&self, conn: &mut TcpStream) -> Result<()> {
        println!("[LIST PEERS][{}:{}]",
            conn.peer_addr().unwrap().ip(),
            conn.peer_addr().unwrap().port());

        let peers = self.peers.lock().unwrap();
        conn.write_all(&[peers.len() as u8])?;

        for peer in &*peers {
            let address = peer.peer_addr().unwrap().ip();

            let mut ip_ver = [4u8];
            if address.is_ipv6() {
                ip_ver[0] = 6u8;
            }
            conn.write(&ip_ver)?;
            let address = peer.peer_addr().unwrap().ip();

            match address {
                IpAddr::V4(ref ip) => {
                    let ip = ip.octets();
                    conn.write_all(&ip[..])?;
                }
                IpAddr::V6(ref ip) => {
                    let ip = ip.octets();
                    conn.write_all(&ip[..])?;
                }
            };

        }

        Ok(())
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

