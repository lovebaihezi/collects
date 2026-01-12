//! Platform-abstracted HTTP client with Send-safe futures.
//!
//! This module provides an HTTP client abstraction that works on both native and WASM targets.
//! The key insight is that on WASM, `reqwest::Response` is not `Send` because it contains
//! JS types (`JsValue`, `JsFuture`, etc.) that are inherently single-threaded.
//!
//! To solve this, we:
//! - On **native**: use reqwest directly (futures are Send)
//! - On **WASM**: spawn the HTTP request on the JS thread using `wasm_bindgen_futures::spawn_local`,
//!   then send the results back through a `flume` channel (which is Send-safe)
//!
//! This allows Commands to return `Pin<Box<dyn Future<Output = ()> + Send>>` on all platforms.

use std::collections::HashMap;

/// HTTP method for requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
}

/// A simplified HTTP response that contains only Send-safe data.
#[derive(Debug, Clone)]
pub struct Response {
    /// HTTP status code
    pub status: u16,
    /// Response headers (lowercased keys)
    pub headers: HashMap<String, String>,
    /// Response body as bytes
    pub body: Vec<u8>,
}

impl Response {
    /// Returns true if the status code is in the 2xx range.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Get a header value by name (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_lowercase()).map(|s| s.as_str())
    }

    /// Attempt to parse the body as UTF-8 text.
    pub fn text(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.body.clone())
    }

    /// Attempt to deserialize the body as JSON.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.body)
    }
}

/// HTTP client error.
#[derive(Debug, Clone)]
pub struct HttpError {
    pub message: String,
}

impl HttpError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP error: {}", self.message)
    }
}

impl std::error::Error for HttpError {}

/// Result type for HTTP operations.
pub type HttpResult<T> = Result<T, HttpError>;

/// A builder for constructing HTTP requests.
#[derive(Debug, Clone)]
pub struct RequestBuilder {
    method: Method,
    url: String,
    headers: HashMap<String, String>,
    body: Option<Vec<u8>>,
}

impl RequestBuilder {
    /// Create a new request builder.
    fn new(method: Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: HashMap::new(),
            body: None,
        }
    }

    /// Add a header to the request.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    /// Add multiple headers to the request.
    pub fn headers(mut self, headers: impl IntoIterator<Item = (String, String)>) -> Self {
        self.headers.extend(headers);
        self
    }

    /// Set the request body as raw bytes.
    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Set the request body as JSON.
    pub fn json<T: serde::Serialize>(mut self, value: &T) -> Result<Self, serde_json::Error> {
        let json_bytes = serde_json::to_vec(value)?;
        self.body = Some(json_bytes);
        self.headers
            .insert("content-type".to_string(), "application/json".to_string());
        Ok(self)
    }

    /// Send the request and return a Send-safe future.
    ///
    /// On native, this uses reqwest directly.
    /// On WASM, this spawns the request on the JS thread and returns results via a channel.
    pub async fn send(self) -> HttpResult<Response> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.send_native().await
        }

        #[cfg(target_arch = "wasm32")]
        {
            self.send_wasm().await
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn send_native(self) -> HttpResult<Response> {
        let client = reqwest::Client::new();

        let mut request = match self.method {
            Method::Get => client.get(&self.url),
            Method::Post => client.post(&self.url),
            Method::Put => client.put(&self.url),
            Method::Delete => client.delete(&self.url),
        };

        // Add headers
        for (name, value) in &self.headers {
            request = request.header(name, value);
        }

        // Add body if present
        if let Some(body) = self.body {
            request = request.body(body);
        }

        // Send request
        let response = request
            .send()
            .await
            .map_err(|e| HttpError::new(e.to_string()))?;

        // Extract status and headers before consuming the response
        let status = response.status().as_u16();
        let mut headers = HashMap::new();
        for (name, value) in response.headers() {
            if let Ok(v) = value.to_str() {
                headers.insert(name.as_str().to_lowercase(), v.to_string());
            }
        }

        // Get body
        let body = response
            .bytes()
            .await
            .map_err(|e| HttpError::new(e.to_string()))?
            .to_vec();

        Ok(Response {
            status,
            headers,
            body,
        })
    }

    #[cfg(target_arch = "wasm32")]
    async fn send_wasm(self) -> HttpResult<Response> {
        // Create a oneshot channel to receive the result
        // flume channels are Send-safe, so this future is Send
        let (tx, rx) = flume::bounded::<HttpResult<Response>>(1);

        // Clone data needed for the spawned task
        let method = self.method;
        let url = self.url;
        let headers = self.headers;
        let body = self.body;

        // Spawn the actual HTTP request on the JS thread
        // This closure is NOT Send, but spawn_local doesn't require Send
        wasm_bindgen_futures::spawn_local(async move {
            let result = Self::execute_wasm_request(method, url, headers, body).await;
            // Send result back through the channel (ignore send errors if receiver dropped)
            let _ = tx.send_async(result).await;
        });

        // Wait for the result through the channel
        // This future IS Send because flume::Receiver is Send
        rx.recv_async()
            .await
            .map_err(|_| HttpError::new("Request cancelled"))?
    }

    #[cfg(target_arch = "wasm32")]
    async fn execute_wasm_request(
        method: Method,
        url: String,
        headers: HashMap<String, String>,
        body: Option<Vec<u8>>,
    ) -> HttpResult<Response> {
        let client = reqwest::Client::new();

        let mut request = match method {
            Method::Get => client.get(&url),
            Method::Post => client.post(&url),
            Method::Put => client.put(&url),
            Method::Delete => client.delete(&url),
        };

        // Add headers
        for (name, value) in &headers {
            request = request.header(name, value);
        }

        // Add body if present
        if let Some(body) = body {
            request = request.body(body);
        }

        // Send request
        let response = request
            .send()
            .await
            .map_err(|e| HttpError::new(e.to_string()))?;

        // Extract status and headers before consuming the response
        let status = response.status().as_u16();
        let mut response_headers = HashMap::new();
        for (name, value) in response.headers() {
            if let Ok(v) = value.to_str() {
                response_headers.insert(name.as_str().to_lowercase(), v.to_string());
            }
        }

        // Get body
        let body = response
            .bytes()
            .await
            .map_err(|e| HttpError::new(e.to_string()))?
            .to_vec();

        Ok(Response {
            status,
            headers: response_headers,
            body,
        })
    }
}

/// HTTP client with Send-safe futures on all platforms.
///
/// # Example
///
/// ```ignore
/// use collects_business::http::Client;
///
/// async fn fetch_data() {
///     let response = Client::get("https://api.example.com/data")
///         .header("Authorization", "Bearer token")
///         .send()
///         .await
///         .unwrap();
///
///     if response.is_success() {
///         let data: MyData = response.json().unwrap();
///     }
/// }
/// ```
pub struct Client;

impl Client {
    /// Create a GET request.
    pub fn get(url: impl Into<String>) -> RequestBuilder {
        RequestBuilder::new(Method::Get, url)
    }

    /// Create a POST request.
    pub fn post(url: impl Into<String>) -> RequestBuilder {
        RequestBuilder::new(Method::Post, url)
    }

    /// Create a PUT request.
    pub fn put(url: impl Into<String>) -> RequestBuilder {
        RequestBuilder::new(Method::Put, url)
    }

    /// Create a DELETE request.
    pub fn delete(url: impl Into<String>) -> RequestBuilder {
        RequestBuilder::new(Method::Delete, url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_is_success() {
        let response = Response {
            status: 200,
            headers: HashMap::new(),
            body: Vec::new(),
        };
        assert!(response.is_success());

        let response = Response {
            status: 404,
            headers: HashMap::new(),
            body: Vec::new(),
        };
        assert!(!response.is_success());
    }

    #[test]
    fn test_response_header_case_insensitive() {
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());

        let response = Response {
            status: 200,
            headers,
            body: Vec::new(),
        };

        // Lookup is case-insensitive: both should find the header
        assert_eq!(response.header("content-type"), Some("application/json"));
        assert_eq!(response.header("Content-Type"), Some("application/json"));
        assert_eq!(response.header("CONTENT-TYPE"), Some("application/json"));
    }

    #[test]
    fn test_response_text() {
        let response = Response {
            status: 200,
            headers: HashMap::new(),
            body: b"Hello, World!".to_vec(),
        };
        assert_eq!(response.text().unwrap(), "Hello, World!");
    }

    #[test]
    fn test_response_json() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct TestData {
            message: String,
        }

        let response = Response {
            status: 200,
            headers: HashMap::new(),
            body: br#"{"message": "hello"}"#.to_vec(),
        };

        let data: TestData = response.json().unwrap();
        assert_eq!(
            data,
            TestData {
                message: "hello".to_string()
            }
        );
    }

    #[test]
    fn test_request_builder_headers() {
        let builder = Client::get("https://example.com")
            .header("Authorization", "Bearer token")
            .header("Accept", "application/json");

        assert_eq!(
            builder.headers.get("Authorization"),
            Some(&"Bearer token".to_string())
        );
        assert_eq!(
            builder.headers.get("Accept"),
            Some(&"application/json".to_string())
        );
    }

    #[test]
    fn test_request_builder_json() {
        #[derive(serde::Serialize)]
        struct TestBody {
            name: String,
        }

        let builder = Client::post("https://example.com")
            .json(&TestBody {
                name: "test".to_string(),
            })
            .unwrap();

        assert_eq!(
            builder.headers.get("content-type"),
            Some(&"application/json".to_string())
        );
        assert!(builder.body.is_some());
    }
}
