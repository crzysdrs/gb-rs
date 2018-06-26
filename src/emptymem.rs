use std::io::{Read,Write};
use std::io;

struct EmptyMem {}
impl EmptyMem {
    pub fn new() -> EmptyMem {
        EmptyMem {}
    }
}

impl Read for EmptyMem {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = 0;
        }
        Ok(buf.len())
    }
}
impl Write for EmptyMem {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
