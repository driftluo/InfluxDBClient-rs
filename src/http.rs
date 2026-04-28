use bytes::Bytes;
use futures::Stream;
use serde::de::DeserializeOwned;
use std::{borrow::Cow, future::Future};
use url::Url;

use crate::error;

/// An abstract HTTP request used by HTTP transport implementations.
#[derive(Debug, Clone)]
pub struct HttpRequest<'a> {
    url: Url,
    body: Option<Cow<'a, str>>,
    bearer_token: Option<Cow<'a, str>>,
}

impl<'a> HttpRequest<'a> {
    /// Create a new request for the provided URL.
    pub fn new(url: Url) -> Self {
        Self {
            url,
            body: None,
            bearer_token: None,
        }
    }

    /// Attach a request body.
    pub fn with_body<T>(mut self, body: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        self.body = Some(body.into());
        self
    }

    /// Attach a bearer token.
    pub fn with_bearer_token<T>(mut self, token: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        self.bearer_token = Some(token.into());
        self
    }

    /// View the request URL.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// View the request body.
    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    /// View the request bearer token.
    pub fn bearer_token(&self) -> Option<&str> {
        self.bearer_token.as_deref()
    }

    /// Decompose the request into its parts.
    pub fn into_parts(self) -> (Url, Option<Cow<'a, str>>, Option<Cow<'a, str>>) {
        (self.url, self.body, self.bearer_token)
    }

    /// Promote any borrowed request parts into owned storage.
    pub fn into_owned(self) -> HttpRequest<'static> {
        let (url, body, bearer_token) = self.into_parts();

        HttpRequest {
            url,
            body: body.map(Cow::into_owned).map(Cow::Owned),
            bearer_token: bearer_token.map(Cow::into_owned).map(Cow::Owned),
        }
    }
}

/// The HTTP method used by spawn-safe request APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    /// A GET request.
    Get,
    /// A POST request.
    Post,
}

/// An abstract HTTP response returned by [`BorrowHttpClient`].
pub trait BorrowHttpResponse: Sized {
    /// Return the HTTP status code.
    fn status(&self) -> u16;

    /// Return a response header by name.
    fn header(&self, name: &str) -> Result<Option<&str>, error::Error>;

    /// Read the response body as text.
    fn text(self) -> impl Future<Output = Result<String, error::Error>>;

    /// Read the response body as bytes.
    fn bytes(self) -> impl Future<Output = Result<Bytes, error::Error>>;

    /// Deserialize the response body as JSON.
    fn json<T>(self) -> impl Future<Output = Result<T, error::Error>>
    where
        T: DeserializeOwned;
}

/// An HTTP response that can expose its body as an async byte stream for chunked queries.
pub trait BorrowChunkedHttpResponse: BorrowHttpResponse {
    /// The async byte stream used to decode chunked query results.
    type Stream: Stream<Item = Result<Bytes, error::Error>>;

    /// Convert the response body into an async byte stream.
    fn into_chunk_stream(
        self,
    ) -> impl Future<Output = Result<Self::Stream, error::Error>> + use<Self>;
}

/// An HTTP response that can expose its body through `Send + 'static` futures.
///
/// This is required for spawn-safe query/write APIs, whose returned futures are spawnable.
pub trait HttpResponse: Send + 'static {
    /// Return the HTTP status code.
    fn status(&self) -> u16;

    /// Return a response header by name.
    fn header(&self, name: &str) -> Result<Option<&str>, error::Error>;

    /// Read the response body as text through a `Send + 'static` future.
    fn text(
        self,
    ) -> impl Future<Output = Result<String, error::Error>> + Send + 'static + use<Self>;

    /// Read the response body as bytes through a `Send + 'static` future.
    fn bytes(
        self,
    ) -> impl Future<Output = Result<Bytes, error::Error>> + Send + 'static + use<Self>;

    /// Deserialize the response body as JSON through a `Send + 'static` future.
    fn json<T>(
        self,
    ) -> impl Future<Output = Result<T, error::Error>> + Send + 'static + use<Self, T>
    where
        T: DeserializeOwned + 'static;
}

/// A chunked HTTP response that can expose its async byte stream through a `Send + 'static`
/// future.
pub trait ChunkedHttpResponse: HttpResponse {
    /// The async byte stream used to decode spawn-safe chunked query results.
    type Stream: Stream<Item = Result<Bytes, error::Error>> + Send + 'static;

    /// Convert the response body into an async byte stream through a `Send + 'static` future.
    fn into_chunk_stream(
        self,
    ) -> impl Future<Output = Result<Self::Stream, error::Error>> + Send + 'static + use<Self>;
}

/// An HTTP client that can service spawn-safe query/write APIs through spawnable futures.
pub trait HttpClient {
    /// The response type produced by this client for spawn-safe APIs.
    type Response: HttpResponse;

    /// Send a spawn-safe owned request.
    fn send(
        &self,
        method: HttpMethod,
        request: HttpRequest<'static>,
    ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static + use<Self>;
}

/// An HTTP client that can service borrowing InfluxDB request APIs.
pub trait BorrowHttpClient {
    /// The response type produced by this client.
    type Response: BorrowHttpResponse;

    /// Send a borrowing request.
    fn send<'a>(
        &'a self,
        method: HttpMethod,
        request: HttpRequest<'a>,
    ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'a, Self>;
}

#[cfg(test)]
mod tests {
    use super::HttpRequest;
    use std::borrow::Cow;
    use url::Url;

    #[test]
    fn http_request_preserves_borrowed_body_and_token() {
        let request = HttpRequest::new(Url::parse("http://localhost:8086").unwrap())
            .with_body("cpu value=1i")
            .with_bearer_token("jwt-token");
        let (_url, body, token) = request.into_parts();

        assert!(matches!(body, Some(Cow::Borrowed("cpu value=1i"))));
        assert!(matches!(token, Some(Cow::Borrowed("jwt-token"))));
    }

    #[test]
    fn http_request_into_owned_promotes_borrowed_parts() {
        let request = HttpRequest::new(Url::parse("http://localhost:8086").unwrap())
            .with_body("cpu value=1i")
            .with_bearer_token("jwt-token")
            .into_owned();
        let (_url, body, token) = request.into_parts();

        assert!(matches!(body, Some(Cow::Owned(body)) if body == "cpu value=1i"));
        assert!(matches!(token, Some(Cow::Owned(token)) if token == "jwt-token"));
    }
}
