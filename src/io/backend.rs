//! Trait-based interface for custom stream backends
//!
//! This module provides an idiomatic Rust way to implement custom stream backends
//! that can be used with SVN streams.

use crate::Error;
use std::io::{Read, Write};

/// Trait for implementing custom stream backends
///
/// This trait allows you to create custom stream implementations that can be used
/// with SVN's stream API. All methods have default implementations that return errors,
/// so you only need to implement the operations your backend supports.
pub trait StreamBackend: Send + 'static {
    /// Read data from the stream
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let _ = buf;
        Err(Error::from_str("Read not supported"))
    }

    /// Write data to the stream
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        let _ = buf;
        Err(Error::from_str("Write not supported"))
    }

    /// Close the stream and flush any pending data
    fn close(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// Check if data is available without blocking
    fn data_available(&mut self) -> Result<bool, Error> {
        Ok(true)
    }

    /// Skip forward in the stream
    fn skip(&mut self, count: usize) -> Result<usize, Error> {
        // Default implementation using read
        let mut buf = vec![0u8; count.min(8192)];
        let mut total_skipped = 0;

        while total_skipped < count {
            let to_skip = (count - total_skipped).min(buf.len());
            let n = self.read(&mut buf[..to_skip])?;
            if n == 0 {
                break; // EOF
            }
            total_skipped += n;
        }

        Ok(total_skipped)
    }

    /// Mark support - returns whether this backend supports marking
    fn supports_mark(&self) -> bool {
        false
    }

    /// Create a mark at the current position (if supported)
    fn mark(&mut self) -> Result<StreamMark, Error> {
        Err(Error::from_str("Mark not supported"))
    }

    /// Seek to a previously created mark (if supported)
    fn seek(&mut self, _mark: &StreamMark) -> Result<(), Error> {
        Err(Error::from_str("Seek not supported"))
    }

    /// Reset to the beginning of the stream (if supported)
    fn reset(&mut self) -> Result<(), Error> {
        Err(Error::from_str("Reset not supported"))
    }

    /// Check if this backend supports reset
    fn supports_reset(&self) -> bool {
        false
    }
}

/// Opaque mark type for stream positioning
pub struct StreamMark {
    // This will be converted to/from svn_stream_mark_t
    pub(crate) position: u64,
}

// Adapter implementation for types that implement both Read and Write
impl<T> StreamBackend for T
where
    T: Read + Write + Send + 'static,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        Read::read(self, buf).map_err(|e| Error::from_str(&e.to_string()))
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        Write::write(self, buf).map_err(|e| Error::from_str(&e.to_string()))
    }

    fn close(&mut self) -> Result<(), Error> {
        Write::flush(self).map_err(|e| Error::from_str(&e.to_string()))
    }
}

/// Buffer-backed stream implementation
pub struct BufferBackend {
    buffer: Vec<u8>,
    read_pos: usize,
    write_pos: usize,
}

impl Default for BufferBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl BufferBackend {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            read_pos: 0,
            write_pos: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            read_pos: 0,
            write_pos: 0,
        }
    }

    pub fn from_vec(buffer: Vec<u8>) -> Self {
        let len = buffer.len();
        Self {
            buffer,
            read_pos: 0,
            write_pos: len,
        }
    }
}

impl StreamBackend for BufferBackend {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let available = self.write_pos.saturating_sub(self.read_pos);
        let to_read = buf.len().min(available);

        if to_read > 0 {
            buf[..to_read].copy_from_slice(&self.buffer[self.read_pos..self.read_pos + to_read]);
            self.read_pos += to_read;
        }

        Ok(to_read)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        // Ensure buffer has enough capacity
        if self.write_pos + buf.len() > self.buffer.len() {
            self.buffer.resize(self.write_pos + buf.len(), 0);
        }

        self.buffer[self.write_pos..self.write_pos + buf.len()].copy_from_slice(buf);
        self.write_pos += buf.len();

        Ok(buf.len())
    }

    fn reset(&mut self) -> Result<(), Error> {
        self.read_pos = 0;
        Ok(())
    }

    fn supports_reset(&self) -> bool {
        true
    }

    fn supports_mark(&self) -> bool {
        true
    }

    fn mark(&mut self) -> Result<StreamMark, Error> {
        Ok(StreamMark {
            position: self.read_pos as u64,
        })
    }

    fn seek(&mut self, mark: &StreamMark) -> Result<(), Error> {
        let pos = mark.position as usize;
        if pos <= self.write_pos {
            self.read_pos = pos;
            Ok(())
        } else {
            Err(Error::from_str("Seek position out of bounds"))
        }
    }
}

/// Read-only adapter for types implementing std::io::Read
pub struct ReadOnlyBackend<R: Read + Send + 'static> {
    reader: R,
}

impl<R: Read + Send + 'static> ReadOnlyBackend<R> {
    pub fn new(reader: R) -> Self {
        Self { reader }
    }
}

impl<R: Read + Send + 'static> StreamBackend for ReadOnlyBackend<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        self.reader
            .read(buf)
            .map_err(|e| Error::from_str(&e.to_string()))
    }
}

/// Write-only adapter for types implementing std::io::Write
pub struct WriteOnlyBackend<W: Write + Send + 'static> {
    writer: W,
}

impl<W: Write + Send + 'static> WriteOnlyBackend<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: Write + Send + 'static> StreamBackend for WriteOnlyBackend<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.writer
            .write(buf)
            .map_err(|e| Error::from_str(&e.to_string()))
    }

    fn close(&mut self) -> Result<(), Error> {
        self.writer
            .flush()
            .map_err(|e| Error::from_str(&e.to_string()))
    }
}
