//! Example demonstrating custom stream backends in subversion-rs
//!
//! This example shows how to create custom stream implementations
//! that can be used with SVN's stream API.

use std::io::{self, Read, Write};
use subversion::io::{backend::*, Stream};

/// Custom backend that logs all operations
struct LoggingBackend<T> {
    inner: T,
    log_prefix: String,
}

impl<T> LoggingBackend<T> {
    fn new(inner: T, prefix: &str) -> Self {
        Self {
            inner,
            log_prefix: prefix.to_string(),
        }
    }
}

impl<T: Read + Write + Send + 'static> StreamBackend for LoggingBackend<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, subversion::Error> {
        println!("{}: Reading up to {} bytes", self.log_prefix, buf.len());
        let n = self.inner
            .read(buf)
            .map_err(|e| subversion::Error::from_str(&e.to_string()))?;
        println!("{}: Read {} bytes", self.log_prefix, n);
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, subversion::Error> {
        println!("{}: Writing {} bytes", self.log_prefix, buf.len());
        let n = self.inner
            .write(buf)
            .map_err(|e| subversion::Error::from_str(&e.to_string()))?;
        println!("{}: Wrote {} bytes", self.log_prefix, n);
        Ok(n)
    }

    fn close(&mut self) -> Result<(), subversion::Error> {
        println!("{}: Closing stream", self.log_prefix);
        self.inner
            .flush()
            .map_err(|e| subversion::Error::from_str(&e.to_string()))?;
        Ok(())
    }
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Custom Stream Backend Examples ===\n");

    // Example 1: BufferBackend (built-in) using IntoStream trait
    println!("1. Using BufferBackend with IntoStream:");
    use subversion::io::IntoStream;
    let backend = BufferBackend::from_vec(b"Hello, Subversion!".to_vec());
    let mut stream = backend.into_stream()?;
    
    let mut buf = vec![0u8; 18];
    let n = stream.read(&mut buf)?;
    println!("   Read {} bytes: {:?}", n, std::str::from_utf8(&buf[..n])?);

    // Example 2: Logging backend wrapping a buffer
    println!("\n2. Using LoggingBackend:");
    let buffer = std::io::Cursor::new(b"Logged data".to_vec());
    let logging_backend = LoggingBackend::new(buffer, "LOG");
    let mut stream = Stream::from_backend(logging_backend)?;
    
    let mut buf = vec![0u8; 11];
    stream.read(&mut buf)?;
    println!("   Result: {:?}", std::str::from_utf8(&buf)?);

    // Example 3: BufferBackend with write and reset
    println!("\n3. Using BufferBackend with write:");
    let backend = BufferBackend::new();
    // Both approaches are equivalent:
    // 1. Direct method call:
    let mut stream = Stream::from_backend(backend)?;
    // 2. Using IntoStream trait (would be: backend.into_stream()?)
    
    // Write data - using std::io::Write trait
    use std::io::Write;
    stream.write_all(b"Written data")?;
    stream.flush()?;
    
    // Note: BufferBackend supports both read and write
    println!("   Data successfully written to buffer backend");

    // Example 4: Using StreamBuilder
    println!("\n4. Using StreamBuilder:");
    use subversion::io::builder::StreamBuilder;
    
    let backend = BufferBackend::from_vec(b"Built with builder".to_vec());
    let stream = StreamBuilder::new(backend)
        .buffer_size(1024)
        .build()?;
    
    println!("   Stream created with builder pattern");

    // Example 5: Read-only and Write-only backends
    println!("\n5. Using specialized backends:");
    
    // Read-only backend from stdin (in practice)
    let data = b"Read-only data";
    let reader = std::io::Cursor::new(data);
    let readonly = ReadOnlyBackend::new(reader);
    let mut stream = Stream::from_backend(readonly)?;
    
    let mut buf = vec![0u8; 14];
    let n = stream.read(&mut buf)?;
    println!("   Read-only: {:?}", std::str::from_utf8(&buf[..n])?);
    
    // Write-only backend to a buffer
    let writer = Vec::new();
    let writeonly = WriteOnlyBackend::new(writer);
    let mut stream = Stream::from_backend(writeonly)?;
    stream.write_all(b"Write-only data")?;
    println!("   Write-only: Data written successfully");

    println!("\n=== All examples completed successfully ===");
    Ok(())
}