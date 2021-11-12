use std::{
    fs,
    io::{self, Read, Write},
    path::PathBuf,
};

pub struct BaseConnection {
    file: fs::File,
}

impl BaseConnection {
    pub fn open() -> io::Result<Self> {
        for i in 0..10 {
            let path = PathBuf::from(format!(r"\\?\pipe\discord-ipc-{}", i));
            if let Ok(file) = std::fs::OpenOptions::new()
                .read(true)
                .append(true)
                .open(&path)
            {
                return Ok(BaseConnection { file });
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "Could not find a free IPC path",
        ))
    }
}

impl Read for BaseConnection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }
}

impl Write for BaseConnection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
