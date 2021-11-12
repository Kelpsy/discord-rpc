use std::{
    env,
    ffi::OsStr,
    io::{self, Read, Write},
    os::unix::net::UnixStream,
};

pub struct BaseConnection {
    stream: UnixStream,
}

impl BaseConnection {
    pub fn open() -> io::Result<Self> {
        let temp_path = env::var_os("XDG_RUNTIME_DIR")
            .or_else(|| env::var_os("TMPDIR"))
            .or_else(|| env::var_os("TMP"))
            .or_else(|| env::var_os("TEMP"))
            .unwrap_or_else(|| OsStr::new("/tmp").to_os_string());
        for i in 0..10 {
            let mut path = temp_path.clone();
            path.push(&format!("/discord-ipc-{}", i));
            if let Ok(stream) = UnixStream::connect(&path) {
                let _ = stream.set_nonblocking(true);
                return Ok(BaseConnection { stream });
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
        self.stream.read(buf)
    }
}

impl Write for BaseConnection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}
