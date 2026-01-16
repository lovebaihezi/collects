//! Cross-platform stdin reader with trait-based abstraction for testability.
//!
//! This module provides a `StdinReader` trait that abstracts stdin reading,
//! allowing for easy testing with mock implementations.
//!
//! # Cross-platform EOF handling
//!
//! - **Unix**: Ctrl+D sends EOF (works when buffer is empty at line start)
//! - **Windows**: Ctrl+Z followed by Enter sends EOF
//!
//! The implementation uses `BufRead` for line-by-line reading which properly
//! handles EOF detection across platforms.
//!
//! # Example
//!
//! ```ignore
//! use collects_input::stdin::{StdinReader, RealStdinReader};
//!
//! fn read_body<R: StdinReader>(reader: &mut R) -> std::io::Result<Option<String>> {
//!     reader.read_body()
//! }
//!
//! // In production:
//! let mut reader = RealStdinReader::new();
//! let body = read_body(&mut reader)?;
//!
//! // In tests:
//! let mut reader = MockStdinReader::new("test content");
//! let body = read_body(&mut reader)?;
//! ```

use std::io::{self, BufRead as _, Read};

/// Trait for reading body content from stdin or other sources.
///
/// This abstraction allows for easy testing by providing mock implementations
/// that don't depend on actual stdin.
pub trait StdinReader {
    /// Read body content until EOF.
    ///
    /// Returns `Ok(Some(content))` if content was read,
    /// `Ok(None)` if no content was provided (empty input),
    /// or `Err` on I/O errors.
    fn read_body(&mut self) -> io::Result<Option<String>>;
}

/// Real stdin reader that reads from `std::io::stdin()`.
///
/// Uses `BufRead` for proper cross-platform EOF detection:
/// - On Unix, Ctrl+D at the start of a line sends EOF
/// - On Windows, Ctrl+Z followed by Enter sends EOF
pub struct RealStdinReader<R: Read> {
    reader: io::BufReader<R>,
}

impl RealStdinReader<io::Stdin> {
    /// Create a new stdin reader that reads from `std::io::stdin()`.
    pub fn new() -> Self {
        Self {
            reader: io::BufReader::new(io::stdin()),
        }
    }
}

impl Default for RealStdinReader<io::Stdin> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: Read> RealStdinReader<R> {
    /// Create a stdin reader with a custom reader (useful for testing with files, etc.)
    pub fn with_reader(reader: R) -> Self {
        Self {
            reader: io::BufReader::new(reader),
        }
    }
}

impl<R: Read> StdinReader for RealStdinReader<R> {
    fn read_body(&mut self) -> io::Result<Option<String>> {
        let mut content = String::new();

        // Read line by line until EOF.
        // This approach properly detects EOF across platforms:
        // - read_line returns Ok(0) on EOF
        // - Each successful read appends to content (including newlines)
        loop {
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) => break, // EOF reached
                Ok(_) => content.push_str(&line),
                Err(e) => return Err(e),
            }
        }

        if content.is_empty() {
            Ok(None)
        } else {
            Ok(Some(content))
        }
    }
}

/// Mock stdin reader for testing purposes.
///
/// Provides predetermined content without requiring actual stdin interaction.
#[derive(Debug, Clone)]
pub struct MockStdinReader {
    content: Option<String>,
    consumed: bool,
}

impl MockStdinReader {
    /// Create a new mock reader with the given content.
    pub fn new<S: Into<String>>(content: S) -> Self {
        Self {
            content: Some(content.into()),
            consumed: false,
        }
    }

    /// Create a mock reader that simulates empty stdin (immediate EOF).
    pub fn empty() -> Self {
        Self {
            content: None,
            consumed: false,
        }
    }
}

impl StdinReader for MockStdinReader {
    fn read_body(&mut self) -> io::Result<Option<String>> {
        if self.consumed {
            return Ok(None);
        }
        self.consumed = true;
        Ok(self.content.take())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_reader_with_content() {
        let mut reader = MockStdinReader::new("Hello, world!");
        let result = reader.read_body().expect("should read body");
        assert_eq!(result, Some("Hello, world!".to_owned()));

        // Second read should return None (already consumed)
        let result = reader.read_body().expect("should read body again");
        assert_eq!(result, None);
    }

    #[test]
    fn test_mock_reader_empty() {
        let mut reader = MockStdinReader::empty();
        let result = reader.read_body().expect("should read body");
        assert_eq!(result, None);
    }

    #[test]
    fn test_mock_reader_multiline() {
        let content = "Line 1\nLine 2\nLine 3";
        let mut reader = MockStdinReader::new(content);
        let result = reader.read_body().expect("should read body");
        assert_eq!(result, Some(content.to_owned()));
    }

    #[test]
    fn test_real_reader_with_cursor() {
        use std::io::Cursor;

        let input = "Test input\nwith multiple lines\n";
        let cursor = Cursor::new(input.as_bytes().to_vec());
        let mut reader = RealStdinReader::with_reader(cursor);

        let result = reader.read_body().expect("should read body");
        assert_eq!(result, Some(input.to_owned()));
    }

    #[test]
    fn test_real_reader_empty_cursor() {
        use std::io::Cursor;

        let cursor = Cursor::new(Vec::<u8>::new());
        let mut reader = RealStdinReader::with_reader(cursor);

        let result = reader.read_body().expect("should read body");
        assert_eq!(result, None);
    }

    #[test]
    fn test_generic_function_with_mock() {
        fn process_stdin<R: StdinReader>(reader: &mut R) -> String {
            reader
                .read_body()
                .expect("should read body")
                .unwrap_or_else(|| "no input".to_owned())
        }

        let mut mock = MockStdinReader::new("test content");
        assert_eq!(process_stdin(&mut mock), "test content");

        let mut empty_mock = MockStdinReader::empty();
        assert_eq!(process_stdin(&mut empty_mock), "no input");
    }
}
