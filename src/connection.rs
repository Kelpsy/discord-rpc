#[cfg(target_family = "unix")]
mod unix;
#[cfg(target_family = "unix")]
pub use unix::*;
#[cfg(target_family = "windows")]
mod windows;
#[cfg(target_family = "windows")]
pub use windows::*;

use super::{messages, User};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

pub mod opcode {
    pub const HANDSHAKE: u32 = 0;
    pub const FRAME: u32 = 1;
    pub const CLOSE: u32 = 2;
    pub const PING: u32 = 3;
    pub const PONG: u32 = 4;
}

pub struct Connection {
    connection: Option<BaseConnection>,
    is_connected: bool,
    pub on_connect: Option<Box<dyn FnMut(Option<User>)>>,
    pub on_disconnect: Option<Box<dyn FnMut(Option<&StreamError>)>>,
    pub app_id: String,
}

pub mod error_code {
    pub const PIPE_CLOSED: u8 = 1;
    pub const READ_CORRUPT: u8 = 2;
}

#[derive(Debug)]
pub enum OpenError {
    Stream(io::Error),
    HandshakeSend(JsonWriteError),
    HandshakeReceive(JsonReadError),
    InvalidHandshake(messages::HandshakeReply),
}

#[derive(Clone, Debug, Deserialize)]
pub struct StreamError {
    pub message: String,
    pub code: u8,
}

#[derive(Debug)]
pub enum JsonReadError {
    Json(serde_json::Error),
    Io(io::Error),
    Stream(Option<StreamError>),
    Disconnected,
}

#[derive(Debug)]
pub enum RawWriteError {
    Io(io::Error),
    Disconnected,
}

#[derive(Debug)]
pub enum JsonWriteError {
    Json(serde_json::Error),
    Raw(RawWriteError),
}

fn write_raw_message(
    connection: &mut BaseConnection,
    opcode: u32,
    message: &[u8],
) -> Result<(), RawWriteError> {
    connection
        .write_all(&opcode.to_le_bytes())
        .map_err(RawWriteError::Io)?;
    connection
        .write_all(&(message.len() as u32).to_le_bytes())
        .map_err(RawWriteError::Io)?;
    connection.write_all(message).map_err(RawWriteError::Io)?;
    Ok(())
}

fn write_json_message<T: Serialize>(
    connection: &mut BaseConnection,
    opcode: u32,
    message: &T,
) -> Result<(), JsonWriteError> {
    let message = serde_json::to_vec(message).map_err(JsonWriteError::Json)?;
    write_raw_message(connection, opcode, &message).map_err(JsonWriteError::Raw)
}

impl Connection {
    pub fn new(app_id: String) -> Self {
        Connection {
            connection: None,
            is_connected: false,
            on_connect: None,
            on_disconnect: None,
            app_id,
        }
    }

    pub fn open(&mut self) -> Result<(), OpenError> {
        if self.connection.is_some() {
            if self.is_connected {
                return Ok(());
            }
            if let Some(handshake) = self
                .read_json::<messages::HandshakeReply>()
                .map_err(OpenError::HandshakeReceive)?
            {
                if handshake.command != "DISPATCH" || handshake.event != "READY" {
                    return Err(OpenError::InvalidHandshake(handshake));
                }
                self.is_connected = true;
                if let Some(on_connect) = &mut self.on_connect {
                    on_connect(handshake.data.user);
                }
            }
        } else {
            let mut connection = BaseConnection::open().map_err(OpenError::Stream)?;
            write_json_message(
                &mut connection,
                opcode::HANDSHAKE,
                &messages::Handshake {
                    version: 1,
                    app_id: &self.app_id,
                },
            )
            .map_err(OpenError::HandshakeSend)?;
            self.connection = Some(connection);
        }
        Ok(())
    }

    fn close_with_error(&mut self, error: Option<&StreamError>) {
        self.connection = None;
        self.is_connected = false;
        if let Some(on_disconnect) = &mut self.on_disconnect {
            on_disconnect(error);
        }
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    pub fn read_json<T: for<'a> Deserialize<'a>>(&mut self) -> Result<Option<T>, JsonReadError> {
        let connection = self
            .connection
            .as_mut()
            .ok_or(JsonReadError::Disconnected)?;
        loop {
            let mut header = [0; 8];
            if let Err(err) = connection.read_exact(&mut header) {
                match err.kind() {
                    io::ErrorKind::WouldBlock => return Ok(None),
                    _ => {
                        let error = StreamError {
                            message: "Pipe closed".to_string(),
                            code: error_code::PIPE_CLOSED,
                        };
                        self.close_with_error(Some(&error));
                        return Err(JsonReadError::Stream(Some(error)));
                    }
                }
            }

            let opcode = u32::from_le_bytes((&header[0..4]).try_into().unwrap());
            let len = u32::from_le_bytes((&header[4..8]).try_into().unwrap());

            let mut message = Vec::new();
            if len != 0 {
                message.resize(len as usize, 0);
                if connection.read_exact(&mut message).is_err() {
                    let error = StreamError {
                        message: "Partial data in frame".to_string(),
                        code: error_code::READ_CORRUPT,
                    };
                    self.close_with_error(Some(&error));
                    return Err(JsonReadError::Stream(Some(error)));
                }
            }

            match opcode {
                opcode::CLOSE => {
                    let error = serde_json::from_slice::<StreamError>(&message).ok();
                    self.close_with_error(error.as_ref());
                    return Err(JsonReadError::Stream(error));
                }
                
                opcode::FRAME => {
                    return serde_json::from_slice(&message).map_err(JsonReadError::Json);
                }

                opcode::PING => {
                    if let Err(RawWriteError::Io(err)) =
                        write_raw_message(connection, opcode::PONG, &[])
                    {
                        self.close_with_error(None);
                        return Err(JsonReadError::Io(err));
                    }
                }

                opcode::PONG => {}

                _ => {
                    let error = StreamError {
                        message: "Bad frame".to_string(),
                        code: error_code::READ_CORRUPT,
                    };
                    self.close_with_error(Some(&error));
                    return Err(JsonReadError::Stream(Some(error)));
                }
            }
        }
    }

    pub fn write_raw(&mut self, message: &[u8]) -> Result<(), RawWriteError> {
        if let Some(connection) = &mut self.connection {
            write_raw_message(connection, opcode::FRAME, message)
        } else {
            Err(RawWriteError::Disconnected)
        }
    }
}
