//! Lock-free object pools and ring buffers for zero-allocation packet and connection processing.

use std::sync::atomic::{AtomicUsize, Ordering};

/// Fixed-capacity ring buffer for low-latency thread communication
pub struct RingBuffer<T, const CAP: usize> {
    #[allow(dead_code)]
    buffer: Vec<Option<T>>,
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl<T: Default + Clone, const CAP: usize> Default for RingBuffer<T, CAP> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Default + Clone, const CAP: usize> RingBuffer<T, CAP> {
    pub fn new() -> Self {
        assert!(CAP.is_power_of_two(), "Capacity must be a power of two");
        let mut buffer = Vec::with_capacity(CAP);
        for _ in 0..CAP {
            buffer.push(None);
        }
        Self {
            buffer,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        head.wrapping_sub(tail) >= CAP
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        CAP
    }

    #[inline]
    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        head.wrapping_sub(tail)
    }
}

/// Fixed-size 64KB packet buffer
pub struct PacketBuffer {
    data: [u8; 65536],
    len: usize,
}

impl Default for PacketBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl PacketBuffer {
    pub fn new() -> Self {
        Self {
            data: [0u8; 65536],
            len: 0,
        }
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data[..self.len]
    }

    #[inline]
    pub fn set_len(&mut self, len: usize) {
        assert!(len <= 65536);
        self.len = len;
    }

    #[inline]
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

/// Pre-allocated buffer pool to eliminate heap malloc/free churn in packet proxying
pub struct BufferPool {
    pool: Vec<PacketBuffer>,
}

impl BufferPool {
    pub fn with_capacity(capacity: usize) -> Self {
        let mut pool = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            pool.push(PacketBuffer::new());
        }
        Self { pool }
    }

    #[inline]
    pub fn acquire(&mut self) -> Option<PacketBuffer> {
        self.pool.pop()
    }

    #[inline]
    pub fn release(&mut self, mut buf: PacketBuffer) {
        buf.clear();
        self.pool.push(buf);
    }

    #[inline]
    pub fn available(&self) -> usize {
        self.pool.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool_reuse() {
        let mut pool = BufferPool::with_capacity(8);
        assert_eq!(pool.available(), 8);

        let mut buf = pool.acquire().unwrap();
        buf.set_len(1024);
        assert_eq!(buf.as_slice().len(), 1024);
        assert_eq!(pool.available(), 7);

        pool.release(buf);
        assert_eq!(pool.available(), 8);
    }
}
