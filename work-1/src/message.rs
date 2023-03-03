use anyhow::{bail, Result};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Message {
    // Connection request
    TryConnect,
    // Answer to TryConnect if you want to connect
    SuccesfullyConnected,
    // Client is not connected
    Unexpected,
    // Close chat
    Disconnect,
    // Answer to Disconnect
    SuccesfullyDisonnected,
    // Text message
    Text(String),
}

impl<S> From<S> for Message
where
    S: Into<String>,
{
    fn from(value: S) -> Self {
        Self::Text(value.into())
    }
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::TryConnect => write!(f, "Connection request"),
            Message::SuccesfullyConnected => write!(f, "Succesfully connected"),
            Message::Unexpected => write!(f, "Unexpected message"),
            Message::Disconnect => write!(f, "Disconnect"),
            Message::SuccesfullyDisonnected => write!(f, "Succesfully disconnected"),
            Message::Text(text) => write!(f, "Message: {text}"),
        }
    }
}

impl Message {
    pub fn into_bytes(self) -> Vec<u8> {
        match self {
            Message::TryConnect => vec![0, 1],
            Message::SuccesfullyConnected => vec![0, 2],
            Message::Unexpected => vec![0, 3],
            Message::Disconnect => vec![0, 4],
            Message::SuccesfullyDisonnected => vec![0, 5],
            Message::Text(message) => {
                let mut vector = vec![1];
                vector.append(&mut message.as_bytes().to_owned());
                vector.push(0);
                vector
            }
        }
    }

    pub fn from_bytes(value: &[u8]) -> Result<Self> {
        let message = match value[0] {
            0 => match value[1] {
                1 => Self::TryConnect,
                2 => Self::SuccesfullyConnected,
                3 => Self::Unexpected,
                4 => Self::Disconnect,
                5 => Self::SuccesfullyDisonnected,
                _ => bail!("Wrong service message type"),
            },
            1 => Self::Text(String::from_utf8_lossy(&value[1..value.len() - 1]).into()),
            _ => bail!("Wrong message type"),
        };
        Ok(message)
    }
}
