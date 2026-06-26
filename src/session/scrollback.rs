use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Scrollback {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Debug)]
struct Inner {
    limit: usize,
    bytes: VecDeque<u8>,
}

impl Scrollback {
    pub fn new(limit: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                limit,
                bytes: VecDeque::with_capacity(limit.min(8192)),
            })),
        }
    }

    pub fn push(&self, bytes: &[u8]) {
        let mut inner = self.inner.lock().expect("scrollback mutex poisoned");
        for byte in bytes {
            if inner.bytes.len() == inner.limit {
                inner.bytes.pop_front();
            }
            inner.bytes.push_back(*byte);
        }
    }

    pub fn snapshot(&self) -> Vec<u8> {
        self.inner
            .lock()
            .expect("scrollback mutex poisoned")
            .bytes
            .iter()
            .copied()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrollback_keeps_last_n_bytes() {
        let scrollback = Scrollback::new(5);
        scrollback.push(b"hello");
        scrollback.push(b" world");
        assert_eq!(scrollback.snapshot(), b"world");
    }
}
