use anyhow::{anyhow, bail, ensure, Result};
use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
    sync::Mutex,
};

use crate::message::Message;

#[derive(Debug)]
pub struct Client {
    socket: UdpSocket,
    chat_history: Mutex<HashMap<SocketAddr, Vec<(bool, String)>>>,
    peer_addr: Mutex<Option<SocketAddr>>,
}

impl Client {
    pub fn new(socket: UdpSocket) -> Self {
        Self {
            socket,
            peer_addr: Default::default(),
            chat_history: Default::default(),
        }
    }

    pub fn address(&self) -> SocketAddr {
        self.socket.local_addr().unwrap()
    }

    pub fn peer_addr(&self) -> Result<SocketAddr> {
        self.peer_addr
            .lock()
            .unwrap()
            .ok_or(anyhow!("Disconnected"))
    }

    fn set_peer_addr(&self, addr: Option<SocketAddr>) {
        *self.peer_addr.lock().unwrap() = addr;
    }

    pub fn is_connected(&self) -> bool {
        self.peer_addr().is_ok()
    }

    fn save_to_history(&self, outgoing: bool, message: Message) {
        if let Message::Text(text) = message {
            if let Ok(mut history) = self.chat_history.lock() {
                let key = self.peer_addr().unwrap();
                if let Some(entry) = history.get_mut(&key) {
                    entry.push((outgoing, text));
                } else {
                    history.insert(key, vec![(outgoing, text)]);
                }
            }
        };
    }

    pub fn history(&self) -> Option<Vec<(bool, String)>> {
        let history = self.chat_history.lock().ok()?;
        let key = self.peer_addr().ok()?;
        Some(history.get(&key)?[..].to_owned())
    }

    fn send_to(&self, message: Message, addr: SocketAddr) -> Result<()> {
        self.save_to_history(true, message.clone());
        self.socket.send_to(&message.into_bytes(), addr)?;
        Ok(())
    }

    pub fn send<M: Into<Message>>(&self, message: M) -> Result<()> {
        self.send_to(message.into(), self.peer_addr()?)?;
        Ok(())
    }

    fn recv(&self) -> Result<Message> {
        let mut buf = [0; 65535];

        let (size, addr) = self.socket.recv_from(&mut buf)?;

        if addr != self.peer_addr()? {
            self.send_to(Message::Unexpected, addr)?;
            self.recv()
        } else {
            let message = Message::from_bytes(&buf[..size])?;
            self.save_to_history(false, message.clone());

            Ok(message)
        }
    }

    pub fn recv_text(&self) -> Result<String> {
        let message = self.recv()?;

        match message {
            Message::Text(text) => Ok(text),
            Message::Disconnect | Message::SuccesfullyDisonnected => {
                if message == Message::Disconnect {
                    _ = self.send(Message::SuccesfullyDisonnected);
                };
                self.set_peer_addr(None);
                bail!("Disconnected");
            }
            _ => bail!("Unexpected message: {message}"),
        }
    }

    pub fn wait_for_connection(&self) -> Result<()> {
        let mut buf = [0; 2];

        let (number_of_bytes, addr) = self.socket.recv_from(&mut buf)?;

        match Message::from_bytes(&buf[..number_of_bytes])? {
            Message::TryConnect => {
                self.send_to(Message::SuccesfullyConnected, addr)?;
                self.set_peer_addr(Some(addr));
                Ok(())
            }
            other => bail!("Expected TryConnect, but got {other:?}"),
        }
    }

    pub fn connect<A: Into<SocketAddr>>(&self, addr: A) -> Result<()> {
        let addr = addr.into();

        ensure!(
            addr != self.address(),
            "Peer address can't be local address"
        );

        self.send_to(Message::TryConnect, addr)?;

        let mut buf = [0; 2];

        loop {
            let (number_of_bytes, src_addr) = self.socket.recv_from(&mut buf)?;

            if src_addr == addr {
                return match Message::from_bytes(&buf[..number_of_bytes])? {
                    Message::SuccesfullyConnected => {
                        self.set_peer_addr(Some(addr));
                        Ok(())
                    }
                    Message::Unexpected => {
                        bail!("Server isn't waiting for connection")
                    }
                    other => bail!("Expected SuccesfullyConnected, but got {other:?}"),
                };
            } else {
                _ = self.send_to(Message::Unexpected, src_addr);
            }
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        Self::new(socket)
    }
}
