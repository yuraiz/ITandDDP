mod client;
mod message;

use anyhow::Result;
use client::Client;
use message::Message::Disconnect;

use std::{net::SocketAddr, str::FromStr, sync::Arc};

fn clear() {
    print!("\x1B[2J\x1B[1;1H");
}

fn connect_or_listen(client: &Client, input: &str) -> Result<()> {
    if let "listen" = input {
        client.wait_for_connection()
    } else {
        let addr = if let Ok(port) = u16::from_str(input) {
            let mut addr = client.address();
            addr.set_port(port);
            addr
        } else {
            SocketAddr::from_str(input)?
        };
        client.connect(addr)
    }
}

fn create_client() -> Client {
    println!("Write a port number you want to use, or click enter to use default");
    loop {
        let mut buf = String::new();

        _ = std::io::stdin().read_line(&mut buf);
        buf.remove(buf.len() - 1);

        dbg!(&buf);

        if buf.is_empty() {
            return Client::default();
        } else {
            match std::net::UdpSocket::bind(format!("127.0.0.1:{buf}")) {
                Ok(socket) => return Client::new(socket),
                Err(e) => println!("Can't use port {buf}: {e}"),
            }
        }
    }
}

fn greeting(client: &Client) {
    clear();
    println!("Local address: {:?}", client.address());
    println!("Write a target address or port you want to chat with or type 'listen'");
}

fn open_chat(client: &Client) {
    clear();
    println!("connected to {}", client.peer_addr().unwrap());

    if let Some(history) = client.history() {
        history.iter().for_each(|(outgoing, text)| {
            if *outgoing {
                println!("{text}");
            } else {
                println!("Message: {text}");
            }
        })
    }
}

fn spawn_input_message_thread(client: &Arc<Client>) {
    let client = client.clone();
    std::thread::spawn(move || loop {
        match client.recv_text() {
            Ok(message) => {
                println!("Message: {}", message);
            }
            Err(err) => {
                if !client.is_connected() {
                    greeting(&client);
                    break;
                } else {
                    println!("{}", err);
                }
            }
        }
    });
}

fn main() {
    let client = Arc::new(create_client());

    let (sender, receiver) = std::sync::mpsc::channel::<String>();

    {
        let client = client.clone();

        std::thread::spawn(move || {
            while let Ok(text) = receiver.recv() {
                if client.is_connected() {
                    if let Err(e) = client.send(text) {
                        println!("Can't send message: {e}")
                    }
                } else {
                    match connect_or_listen(&client, text.as_str()) {
                        Ok(_) => {
                            open_chat(&client);
                            spawn_input_message_thread(&client);
                        }
                        Err(e) => println!("{e}"),
                    }
                }
            }
        })
    };

    loop {
        greeting(&client);

        for line in std::io::stdin().lines() {
            sender.send(line.unwrap()).unwrap()
        }

        if client.is_connected() {
            client.send(Disconnect).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect() {
        let client1 = Client::default();
        let client2 = Client::default();

        let addr1 = client1.address();

        let handle = std::thread::spawn(move || {
            client2.connect(addr1).unwrap();
        });
        client1.wait_for_connection().unwrap();

        handle.join().unwrap();
    }

    #[test]
    fn communicate() {
        let client1 = Client::default();
        let client2 = Client::default();

        let addr1 = client1.address();
        let handle = std::thread::spawn(move || {
            client2.connect(addr1).unwrap();

            client2.send("Hello").unwrap();
            assert_eq!(&client2.recv_text().unwrap(), "Bye");
        });

        client1.wait_for_connection().unwrap();
        assert_eq!(&client1.recv_text().unwrap(), "Hello");
        client1.send("Bye").unwrap();

        handle.join().unwrap();
    }

    #[test]
    fn disconnect() {
        let client1 = Client::default();
        let client2 = Client::default();

        let addr1 = client1.address();
        let handle = std::thread::spawn(move || {
            client2.connect(addr1).unwrap();

            client2.send("Hello").unwrap();

            assert!(client2.recv_text().is_err());
            assert!(!client2.is_connected());
        });

        client1.wait_for_connection().unwrap();
        assert_eq!(&client1.recv_text().unwrap(), "Hello");

        client1.send(Disconnect).unwrap();

        assert!(client1.recv_text().is_err());
        assert!(!client1.is_connected());

        handle.join().unwrap();
    }

    #[test]
    fn history() {
        let client1 = Client::default();
        let client2 = Client::default();

        let addr1 = client1.address();
        let addr2 = client2.address();
        let handle = std::thread::spawn(move || {
            client2.connect(addr1).unwrap();

            client2.send("Hello").unwrap();
            assert_eq!(&client2.recv_text().unwrap(), "Bye");

            assert!(client2.recv_text().is_err());
            assert!(!client2.is_connected());

            client2.wait_for_connection().unwrap();

            let history = client2.history().unwrap();
            assert_eq!(*history, [(true, "Hello".into()), (false, "Bye".into())]);
        });

        client1.wait_for_connection().unwrap();
        assert_eq!(&client1.recv_text().unwrap(), "Hello");

        client1.send("Bye").unwrap();

        client1.send(Disconnect).unwrap();

        assert!(client1.recv_text().is_err());
        assert!(!client1.is_connected());

        client1.connect(addr2).unwrap();

        let history = client1.history().unwrap();
        assert_eq!(*history, [(false, "Hello".into()), (true, "Bye".into())]);

        handle.join().unwrap();
    }

    #[test]
    fn three_clients() {
        let client1 = Client::default();
        let client2 = Client::default();
        let client3 = Client::default();

        let addr1 = client1.address();
        let addr2 = client2.address();
        let addr3 = client3.address();

        let handles = [
            std::thread::spawn(move || {
                client1.wait_for_connection().unwrap();
                client1.recv_text().unwrap_err();
                client1.connect(addr2).unwrap();
            }),
            std::thread::spawn(move || {
                client2.connect(addr3).unwrap();
                client2.recv_text().unwrap_err();

                client2.connect(addr1).unwrap();
                client2.send(Disconnect).unwrap();
                client2.recv_text().unwrap_err();

                client2.wait_for_connection().unwrap();
            }),
        ];

        client3.wait_for_connection().unwrap();
        client3.send(Disconnect).unwrap();
        client3.recv_text().unwrap_err();
        assert!(!client3.is_connected());

        handles.map(|handle| handle.join().unwrap());
    }
}
