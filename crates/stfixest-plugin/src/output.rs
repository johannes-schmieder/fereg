use std::sync::Mutex;

pub trait OutputSink {
    fn display(&self, message: &str);
    fn error(&self, message: &str);
}

#[derive(Debug, Default)]
pub struct BufferSink {
    display: Mutex<String>,
    error: Mutex<String>,
}

impl BufferSink {
    #[must_use]
    pub fn displayed(&self) -> String {
        self.display
            .lock()
            .expect("display buffer poisoned")
            .clone()
    }

    #[must_use]
    pub fn errors(&self) -> String {
        self.error.lock().expect("error buffer poisoned").clone()
    }
}

impl OutputSink for BufferSink {
    fn display(&self, message: &str) {
        self.display
            .lock()
            .expect("display buffer poisoned")
            .push_str(message);
    }

    fn error(&self, message: &str) {
        self.error
            .lock()
            .expect("error buffer poisoned")
            .push_str(message);
    }
}
