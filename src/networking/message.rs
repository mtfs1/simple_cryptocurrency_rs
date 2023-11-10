use std::io::{Read, Error, ErrorKind, Result, Write};
use std::net::TcpStream;

use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, Debug)]
pub enum MessageType {
    StartPeering,
    Ack
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MessageHeader {
    pub message_type: MessageType
}

impl MessageHeader {
    pub fn new() -> Self {
        MessageHeader {
            message_type: MessageType::StartPeering
        }
    }

    pub fn set_type(mut self, message_type: MessageType) -> Self {
        self.message_type = message_type;
        self
    }

    pub fn is_ack(&self) -> bool {
        if let MessageType::Ack = self.message_type {
            return true;
        }

        false
    }

    pub fn send_to(&self, conn: &mut TcpStream) -> Result<()> {
        conn.write_all(b"rusty")?;
        let message = bincode::serialize(self).unwrap();
        conn.write_all(&message)?;
        Ok(())
    }

    pub fn receive_from(conn: &mut TcpStream) -> Result<MessageHeader> {
        wait_for_magic(conn)?;
        bincode::deserialize_from(conn).map_err(|_| Error::new(
            ErrorKind::InvalidData,
            "Invalid message from peer"
        ))
    }
}

fn wait_for_magic(conn: &mut TcpStream) -> Result<()> {
    let magic = b"rusty";
    let mut rest: &[u8] = magic;

    loop {
        let mut buff = [0u8];
        let res = conn.read(&mut buff);

        if let Err(_) = res {
            continue;
        }

        if res.unwrap() == 0 {
            return Err(Error::new(
                ErrorKind::Interrupted,
                "The connection was interrupted"
            ));
        }

        if buff[0] != rest[0] {
            rest = magic;
            continue;
        }

        rest = &rest[1..];
        if rest.len() == 0 {
            return Ok(());
        }
    }
}

