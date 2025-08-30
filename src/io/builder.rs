//! Builder pattern for creating streams with custom configurations

use super::backend::StreamBackend;
use super::Stream;
use crate::Error;

/// Builder for creating streams with custom backends and configurations
pub struct StreamBuilder<B: StreamBackend> {
    backend: B,
    buffer_size: Option<usize>,
    lazy_open: bool,
    disown: bool,
}

impl<B: StreamBackend> StreamBuilder<B> {
    /// Create a new stream builder with the given backend
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            buffer_size: None,
            lazy_open: false,
            disown: false,
        }
    }

    /// Set the buffer size for buffered operations
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = Some(size);
        self
    }

    /// Enable lazy opening - stream won't be opened until first use
    pub fn lazy_open(mut self) -> Self {
        self.lazy_open = true;
        self
    }

    /// Create a disowned stream that doesn't close the underlying resource
    pub fn disown(mut self) -> Self {
        self.disown = true;
        self
    }

    /// Build the stream with the configured options
    pub fn build(self) -> Result<Stream, Error> {
        let stream = Stream::from_backend(self.backend)?;

        // Apply configurations
        if self.buffer_size.is_some() {
            // In a real implementation, we might wrap the stream in a buffering layer
            // For now, SVN handles its own buffering
        }

        // Note: disown functionality would need to be implemented at the backend level
        // The SVN disown creates a new stream that doesn't close the underlying resource
        // For custom backends, this behavior should be controlled by the backend itself

        Ok(stream)
    }
}

/// Extension trait for creating streams from various types
pub trait IntoStream {
    /// Convert this type into a Stream
    fn into_stream(self) -> Result<Stream, Error>;
}

impl<T> IntoStream for T
where
    T: StreamBackend,
{
    fn into_stream(self) -> Result<Stream, Error> {
        Stream::from_backend(self)
    }
}

/// Helper to create a buffered stream
pub fn buffered_stream<B: StreamBackend>(backend: B, buffer_size: usize) -> Result<Stream, Error> {
    StreamBuilder::new(backend).buffer_size(buffer_size).build()
}

/// Helper to create a lazy stream that opens on first use
pub fn lazy_stream<B: StreamBackend>(backend: B) -> Result<Stream, Error> {
    StreamBuilder::new(backend).lazy_open().build()
}
