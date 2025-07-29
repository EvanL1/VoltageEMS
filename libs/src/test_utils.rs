//! Test utilities for `VoltageEMS` services

use std::net::TcpListener;
use tempfile::{TempDir, TempPath};
use tokio::sync::oneshot;

/// Find an available port for testing
pub fn find_available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind to port 0")
        .local_addr()
        .expect("Failed to get local address")
        .port()
}

/// Create a temporary directory that is automatically cleaned up
pub fn temp_dir() -> TempDir {
    TempDir::new().expect("Failed to create temp directory")
}

/// Create a temporary file path (file is not created)
pub fn temp_file_path(extension: &str) -> TempPath {
    tempfile::Builder::new()
        .suffix(extension)
        .tempfile()
        .expect("Failed to create temp file")
        .into_temp_path()
}

// Point data generation functions removed - each service should define its own test data

/// Test fixture for async tests with timeout
pub async fn with_timeout<F, Fut, T>(duration: std::time::Duration, f: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    tokio::time::timeout(duration, f())
        .await
        .expect("Test timed out")
}

/// Mock service for testing inter-service communication
pub struct MockService {
    pub name: String,
    pub port: u16,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl MockService {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            port: find_available_port(),
            shutdown_tx: None,
        }
    }

    pub async fn start(&mut self) {
        let (tx, rx) = oneshot::channel();
        self.shutdown_tx = Some(tx);

        // Mock service implementation
        let port = self.port;
        tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
                .await
                .expect("Failed to bind");

            tokio::select! {
                _ = rx => {},
                () = async {
                    loop {
                        if let Ok((stream, _)) = listener.accept().await {
                            // Handle connection
                            drop(stream);
                        }
                    }
                } => {}
            }
        });

        // Give the service time to start
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{}", self.port, path)
    }
}

impl Drop for MockService {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Assert that two floats are approximately equal
pub fn assert_approx_eq(a: f64, b: f64, epsilon: f64) {
    assert!(
        (a - b).abs() < epsilon,
        "Values are not approximately equal: {a} != {b} (epsilon: {epsilon})"
    );
}

/// Create a test configuration file
pub fn create_test_config<T: serde::Serialize>(
    config: &T,
    format: &str,
) -> Result<TempPath, Box<dyn std::error::Error>> {
    let path = temp_file_path(&format!(".{format}"));
    let content = match format {
        "yaml" | "yml" => serde_yaml::to_string(config)?,
        "json" => serde_json::to_string_pretty(config)?,
        "toml" => toml::to_string_pretty(config)?,
        _ => return Err("Unsupported format".into()),
    };
    std::fs::write(&path, content)?;
    Ok(path)
}

/// Test data generator for various patterns
pub struct TestDataGenerator {
    counter: u64,
}

impl TestDataGenerator {
    pub fn new() -> Self {
        Self { counter: 0 }
    }

    /// Generate sine wave data
    pub fn sine_wave(&mut self, amplitude: f64, frequency: f64) -> f64 {
        let value = amplitude * (self.counter as f64 * frequency).sin();
        self.counter += 1;
        value
    }

    /// Generate square wave data
    pub fn square_wave(&mut self, amplitude: f64, period: u64) -> f64 {
        let value = if (self.counter / period) % 2 == 0 {
            amplitude
        } else {
            -amplitude
        };
        self.counter += 1;
        value
    }

    /// Generate random data
    pub fn random(&mut self, min: f64, max: f64) -> f64 {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        self.counter += 1;
        rng.gen_range(min..max)
    }

    /// Generate constant data
    pub fn constant(&mut self, value: f64) -> f64 {
        self.counter += 1;
        value
    }

    pub fn reset(&mut self) {
        self.counter = 0;
    }
}

impl Default for TestDataGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_available_port() {
        let port = find_available_port();
        assert!(port > 0);
        // Port is u16, so it's always < 65536
    }

    #[test]
    fn test_temp_dir() {
        let dir = temp_dir();
        assert!(dir.path().exists());
        let path = dir.path().to_path_buf();
        drop(dir);
        assert!(!path.exists());
    }

    // test_generate_point_data removed - point data is now service-specific

    #[test]
    fn test_data_generator() {
        let mut gen = TestDataGenerator::new();

        // Test sine wave
        let val1 = gen.sine_wave(1.0, 0.1);
        let val2 = gen.sine_wave(1.0, 0.1);
        assert_ne!(val1, val2);

        // Test square wave
        gen.reset();
        let vals: Vec<_> = (0..10).map(|_| gen.square_wave(1.0, 2)).collect();
        assert_eq!(vals[0], 1.0);
        assert_eq!(vals[1], 1.0);
        assert_eq!(vals[2], -1.0);
        assert_eq!(vals[3], -1.0);
    }

    #[tokio::test]
    async fn test_with_timeout() {
        let result = with_timeout(std::time::Duration::from_secs(1), || async {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            42
        })
        .await;
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_mock_service() {
        let mut service = MockService::new("test_service");
        service.start().await;

        let url = service.url("/test");
        assert!(url.contains(&service.port.to_string()));

        // Service should be listening
        let result = tokio::net::TcpStream::connect(("127.0.0.1", service.port)).await;
        assert!(result.is_ok());
    }
}
