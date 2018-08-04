use std::io;
use std::io::{Read, Write};

#[allow(dead_code)]
pub struct EmptyMem {}
#[allow(dead_code)]
impl EmptyMem {
    pub fn new() -> EmptyMem {
        EmptyMem {}
    }
}
#[allow(dead_code)]
impl Read for EmptyMem {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        for (_, b) in buf.iter_mut().enumerate() {
            *b = 0;
        }
        Ok(buf.len())
    }
}
#[allow(dead_code)]
impl Write for EmptyMem {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
