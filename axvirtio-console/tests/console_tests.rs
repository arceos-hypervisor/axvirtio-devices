//! Unit tests for axvirtio-console device functionality

use axvirtio_console::{ConsoleBackend, NullConsoleBackend};

#[cfg(test)]
mod backend_tests {
    use super::*;

    #[test]
    fn test_null_backend_get_size_default() {
        let backend = NullConsoleBackend;
        let (cols, rows) = backend.get_size();

        assert_eq!(cols, 80, "Default columns should be 80");
        assert_eq!(rows, 25, "Default rows should be 25");
    }

    #[test]
    fn test_null_backend_get_size_within_reasonable_range() {
        let backend = NullConsoleBackend;
        let (cols, rows) = backend.get_size();

        // Terminal size should be within reasonable bounds
        assert!(
            cols >= 40 && cols <= 512,
            "Columns {} should be between 40 and 512",
            cols
        );
        assert!(
            rows >= 10 && rows <= 200,
            "Rows {} should be between 10 and 200",
            rows
        );
    }

    #[test]
    fn test_null_backend_read_returns_zero() {
        let backend = NullConsoleBackend;
        let mut buffer = [0u8; 128];

        let result = backend.read(&mut buffer);
        assert!(result.is_ok(), "Read should succeed");
        assert_eq!(
            result.unwrap(),
            0,
            "Null backend should return 0 bytes read"
        );
    }

    #[test]
    fn test_null_backend_write_returns_buffer_length() {
        let backend = NullConsoleBackend;
        let data = b"Hello, World!";

        let result = backend.write(data);
        assert!(result.is_ok(), "Write should succeed");
        assert_eq!(result.unwrap(), data.len(), "Should write all bytes");
    }

    #[test]
    fn test_null_backend_has_pending_input_false() {
        let backend = NullConsoleBackend;

        assert!(
            !backend.has_pending_input(),
            "Null backend should never have pending input"
        );
    }

    #[test]
    fn test_null_backend_write_empty_buffer() {
        let backend = NullConsoleBackend;
        let data = b"";

        let result = backend.write(data);
        assert!(result.is_ok(), "Write empty buffer should succeed");
        assert_eq!(result.unwrap(), 0, "Should write 0 bytes for empty buffer");
    }

    #[test]
    fn test_null_backend_write_large_buffer() {
        let backend = NullConsoleBackend;
        let large_data = vec![0u8; 4096];

        let result = backend.write(&large_data);
        assert!(result.is_ok(), "Write large buffer should succeed");
        assert_eq!(result.unwrap(), 4096, "Should write all 4096 bytes");
    }

    #[test]
    fn test_null_backend_read_into_empty_buffer() {
        let backend = NullConsoleBackend;
        let mut buffer = [];

        let result = backend.read(&mut buffer);
        assert!(result.is_ok(), "Read into empty buffer should succeed");
        assert_eq!(result.unwrap(), 0, "Should return 0 for empty buffer");
    }

    #[test]
    fn test_null_backend_read_into_small_buffer() {
        let backend = NullConsoleBackend;
        let mut buffer = [0u8; 8];

        let result = backend.read(&mut buffer);
        assert!(result.is_ok(), "Read into small buffer should succeed");
        assert_eq!(result.unwrap(), 0, "Should return 0 bytes");
        // Buffer should remain unchanged
        assert_eq!(buffer, [0u8; 8], "Buffer should remain all zeros");
    }

    #[test]
    fn test_null_backend_write_various_sizes() {
        let backend = NullConsoleBackend;

        // Test different buffer sizes
        for size in [1, 7, 64, 128, 256, 512, 1024, 2048] {
            let data = vec![0xABu8; size];
            let result = backend.write(&data);
            assert!(result.is_ok(), "Write of {} bytes should succeed", size);
            assert_eq!(result.unwrap(), size, "Should write all {} bytes", size);
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_console_backend_send_sync() {
        // Verify that NullConsoleBackend implements Send and Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NullConsoleBackend>();
    }

    #[test]
    fn test_backend_trait_object_compatibility() {
        // Test that backend can be used as a trait object
        let backend: Box<dyn ConsoleBackend> = Box::new(NullConsoleBackend);

        let (cols, rows) = backend.get_size();
        assert_eq!(cols, 80);
        assert_eq!(rows, 25);

        let data = b"test";
        let result = backend.write(data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 4);
    }
}
