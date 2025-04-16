use std::fmt;
use std::fs::File;
use std::io;
use std::io::Write;
use std::rc::Rc;
use std::cell::RefCell;

struct MemWriter {
    data: Rc<RefCell<Vec<u8>>>,
}

impl MemWriter {
    pub fn new(data: Rc<RefCell<Vec<u8>>>) -> Self {
        Self { data }
    }
}

impl io::Write for MemWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.data.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

enum OutputSinkType {
    File,
    Memory,
}

pub struct OutputSink {
    sink_type: OutputSinkType,

    filename: String,

    // Output bytes for memory sink.
    data: Rc<RefCell<Vec<u8>>>,
}

impl OutputSink {
    pub fn file(filename: &str) -> Self {
        Self {
            sink_type: OutputSinkType::File,
            filename: filename.to_string(),
            data: Default::default(),
        }
    }

    pub fn memory(data: Vec<u8>) -> Self {
        Self {
            sink_type: OutputSinkType::Memory,
            filename: String::new(),
            data: Rc::new(RefCell::new(data)),
        }
    }

    pub fn writer(&mut self) -> Box<dyn Write> {
        match &self.sink_type {
            OutputSinkType::File => {
                let file = File::create(&self.filename).unwrap();
                Box::new(file)
            }
            OutputSinkType::Memory => {
                let writer = MemWriter::new(self.data.clone());
                Box::new(writer)
            }
        }
    }

    pub fn take_memory(&mut self) -> Vec<u8> {
        self.data.take()
    }
}

impl fmt::Display for OutputSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.sink_type {
            OutputSinkType::File => write!(f, "Output file: {}", self.filename),
            OutputSinkType::Memory => write!(f, "Output memory"),
        }
    }
}
