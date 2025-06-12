//! Serial Port Mock for Testing
//! 
//! This module provides a mock serial port implementation for testing
//! Modbus RTU communication without requiring actual hardware.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Mock serial port for testing
#[derive(Debug, Clone)]
pub struct MockSerialPort {
    /// Buffer for data to be read
    read_buffer: Arc<Mutex<VecDeque<u8>>>,
    /// Buffer for data that was written
    write_buffer: Arc<Mutex<VecDeque<u8>>>,
    /// Flag to simulate connection status
    connected: Arc<Mutex<bool>>,
}

impl MockSerialPort {
    /// Create a new mock serial port
    pub fn new() -> Self {
        Self {
            read_buffer: Arc::new(Mutex::new(VecDeque::new())),
            write_buffer: Arc::new(Mutex::new(VecDeque::new())),
            connected: Arc::new(Mutex::new(true)),
        }
    }
    
    /// Add data to the read buffer (simulates data coming from device)
    pub fn add_read_data(&self, data: &[u8]) {
        let mut buffer = self.read_buffer.lock().unwrap();
        for &byte in data {
            buffer.push_back(byte);
        }
    }
    
    /// Get data from the write buffer (data that was written to the port)
    pub fn get_written_data(&self) -> Vec<u8> {
        let mut buffer = self.write_buffer.lock().unwrap();
        let data: Vec<u8> = buffer.drain(..).collect();
        data
    }
    
    /// Clear all buffers
    pub fn clear_buffers(&self) {
        self.read_buffer.lock().unwrap().clear();
        self.write_buffer.lock().unwrap().clear();
    }
    
    /// Set connection status
    pub fn set_connected(&self, connected: bool) {
        *self.connected.lock().unwrap() = connected;
    }
    
    /// Check if connected
    pub fn is_connected(&self) -> bool {
        *self.connected.lock().unwrap()
    }
}

impl AsyncRead for MockSerialPort {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if !self.is_connected() {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "Mock serial port not connected"
            )));
        }
        
        let mut read_buffer = self.read_buffer.lock().unwrap();
        let bytes_to_read = std::cmp::min(buf.remaining(), read_buffer.len());
        
        if bytes_to_read == 0 {
            // No data available - would block
            return Poll::Pending;
        }
        
        let mut data = vec![0u8; bytes_to_read];
        for i in 0..bytes_to_read {
            data[i] = read_buffer.pop_front().unwrap();
        }
        
        buf.put_slice(&data);
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for MockSerialPort {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if !self.is_connected() {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "Mock serial port not connected"
            )));
        }
        
        let mut write_buffer = self.write_buffer.lock().unwrap();
        for &byte in buf {
            write_buffer.push_back(byte);
        }
        
        Poll::Ready(Ok(buf.len()))
    }
    
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    
    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.set_connected(false);
        Poll::Ready(Ok(()))
    }
}

/// Mock serial port pair for bidirectional communication testing
#[derive(Debug)]
pub struct MockSerialPortPair {
    pub client_port: MockSerialPort,
    pub server_port: MockSerialPort,
}

impl MockSerialPortPair {
    /// Create a connected pair of mock serial ports
    pub fn new() -> Self {
        let client_port = MockSerialPort::new();
        let server_port = MockSerialPort::new();
        
        Self {
            client_port,
            server_port,
        }
    }
    
    /// Connect the ports so data written to one can be read from the other
    pub fn connect(&self) {
        // This would be implemented to forward data between ports
        // For now, we'll handle this manually in tests
    }
    
    /// Simulate client writing data that server can read
    pub fn client_to_server_data(&self, data: &[u8]) {
        self.server_port.add_read_data(data);
    }
    
    /// Simulate server writing data that client can read
    pub fn server_to_client_data(&self, data: &[u8]) {
        self.client_port.add_read_data(data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    
    #[tokio::test]
    async fn test_mock_serial_port_write_read() {
        let port = MockSerialPort::new();
        
        // Test writing data
        let mut port_write = port.clone();
        let written = port_write.write(b"Hello").await.unwrap();
        assert_eq!(written, 5);
        
        // Check that data was captured
        let written_data = port.get_written_data();
        assert_eq!(written_data, b"Hello");
    }
    
    #[tokio::test]
    async fn test_mock_serial_port_read() {
        let port = MockSerialPort::new();
        
        // Add data to read buffer
        port.add_read_data(b"World");
        
        // Test reading data
        let mut port_read = port.clone();
        let mut buffer = [0u8; 10];
        let read_count = port_read.read(&mut buffer).await.unwrap();
        
        assert_eq!(read_count, 5);
        assert_eq!(&buffer[..read_count], b"World");
    }
    
    #[test]
    fn test_connection_status() {
        let port = MockSerialPort::new();
        
        assert!(port.is_connected());
        
        port.set_connected(false);
        assert!(!port.is_connected());
    }
    
    #[test]
    fn test_buffer_operations() {
        let port = MockSerialPort::new();
        
        // Add some data
        port.add_read_data(b"test");
        
        // Clear buffers
        port.clear_buffers();
        
        // Check that buffers are empty
        let written_data = port.get_written_data();
        assert!(written_data.is_empty());
    }
} 