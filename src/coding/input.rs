use std::fmt;
use std::fs::File;
use std::io;
use std::io::Read;
use std::rc::Rc;

struct MemReader {
    data: Rc<Vec<u8>>,
    pos: usize,
}

impl MemReader {
    pub fn new(data: Rc<Vec<u8>>) -> Self {
        Self { data, pos: 0 }
    }
}

impl io::Read for MemReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let available = self.data.len() - self.pos;
        let to_read = buf.len().min(available);
        buf[..to_read].copy_from_slice(&self.data[self.pos..self.pos + to_read]);
        self.pos += to_read;
        Ok(to_read)
    }
}
enum InputSourceType {
    File,
    Memory,
}

pub struct InputSource {
    source_type: InputSourceType,

    // For file input.
    filename: String,

    // For memory input.
    data: Rc<Vec<u8>>,
}

impl InputSource {
    pub fn file(filename: &str) -> Self {
        Self {
            source_type: InputSourceType::File,
            filename: filename.to_string(),
            data: Default::default(),
        }
    }

    pub fn memory(data: Vec<u8>) -> Self {
        Self {
            source_type: InputSourceType::Memory,
            filename: Default::default(),
            data: Rc::new(data),
        }
    }

    pub fn take_memory(self) -> Vec<u8> {
        Rc::into_inner(self.data).unwrap()
    }

    pub fn frequencies(&self) -> Vec<u32> {
        let mut frequencies: Vec<u32> = vec![0; 256];
        match &self.source_type {
            InputSourceType::File => {
                let mut file = File::open(&self.filename).unwrap();
                let mut buffer = [0; 1024];
                while let Ok(bytes_read) = file.read(&mut buffer) {
                    if bytes_read == 0 {
                        break;
                    }
                    for byte in buffer[..bytes_read].iter() {
                        frequencies[*byte as usize] += 1;
                    }
                }
            }
            InputSourceType::Memory => {
                for byte in self.data.iter() {
                    frequencies[*byte as usize] += 1;
                }
            }
        }
        frequencies
    }

    pub fn reader(&mut self) -> Box<dyn io::Read> {
        match &self.source_type {
            InputSourceType::File => {
                let file = File::open(&self.filename).unwrap();
                Box::new(file)
            }
            InputSourceType::Memory => Box::new(MemReader::new(self.data.clone())),
        }
    }
}

impl fmt::Display for InputSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.source_type {
            InputSourceType::File => write!(f, "Input file: {}", self.filename),
            InputSourceType::Memory => write!(f, "Input memory: {:?} bytes", self.data.len()),
        }
    }
}
