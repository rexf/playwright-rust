use crate::imp::core::*;
use std::{
    convert::TryInto,
    io,
    io::{Read, Write},
    process::{ChildStdin, ChildStdout}
};
use thiserror::Error;

#[derive(Debug)]
pub(super) struct Reader {
    stdout: ChildStdout,
    buf: Vec<u8>
}

#[derive(Debug)]
pub(super) struct Writer {
    stdin: ChildStdin
}

#[derive(Error, Debug)]
pub enum TransportError {
    #[error(transparent)]
    Serde(#[from] serde_json::error::Error),
    #[error(transparent)]
    Io(#[from] io::Error)
}

impl Reader {
    const BUFSIZE: usize = 30000;

    pub(super) fn new(stdout: ChildStdout) -> Self {
        Self {
            stdout,
            buf: Vec::with_capacity(Self::BUFSIZE)
        }
    }

    // TODO: heap efficiency
    pub(super) fn try_read(&mut self) -> Result<Option<Res>, TransportError> {
        // Read length-prefixed (u32 LE) JSON string.
        {
            if self.buf.len() >= 4 {
                let len = u32::from_le_bytes(self.buf[..4].try_into().unwrap()) as usize;
                if self.buf.len() >= 4 + len {
                    let bytes = self.buf[4..4 + len].to_vec();
                    self.buf = self.buf[4 + len..].to_vec();
                    log::debug!("RECV {}", unsafe { std::str::from_utf8_unchecked(&bytes) });
                    let msg: Res = serde_json::from_slice(&bytes)?;
                    return Ok(Some(msg));
                }
            }
        }
        let mut buf = [0; Self::BUFSIZE];
        let n = self.stdout.read(&mut buf)?;
        self.buf.extend(&buf[..n]);
        Ok(None)
    }
}

impl Writer {
    pub(super) fn new(stdin: ChildStdin) -> Self { Self { stdin } }

    pub(super) fn send(&mut self, req: &Req<'_, '_>) -> Result<(), TransportError> {
        log::debug!("SEND {:?}", &req);
        let serialized = serde_json::to_vec(&req)?;
        let length = serialized.len() as u32;
        let mut bytes = length.to_le_bytes().to_vec();
        bytes.extend(serialized);
        self.stdin.write_all(&bytes)?;
        Ok(())
    }
}
