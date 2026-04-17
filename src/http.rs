use bytes::Bytes;
use futures::Stream;
use serde::de::DeserializeOwned;
use std::future::Future;
use url::Url;

use crate::error;

/// An abstract HTTP request used by [`HttpClient`] implementations.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    url: Url,
    body: Option<String>,
    bearer_token: Option<String>,
}

impl HttpRequest {
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
        T: Into<String>,
    {
        self.body = Some(body.into());
        self
    }

    /// Attach a bearer token.
    pub fn with_bearer_token<T>(mut self, token: T) -> Self
    where
        T: Into<String>,
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
    pub fn into_parts(self) -> (Url, Option<String>, Option<String>) {
        (self.url, self.body, self.bearer_token)
    }
}

/// An abstract HTTP response returned by [`HttpClient`].
pub trait HttpResponse: Sized {
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
pub trait ChunkedHttpResponse: HttpResponse {
    /// The async byte stream used to decode chunked query results.
    type Stream: Stream<Item = Result<Bytes, error::Error>>;

    /// Convert the response body into an async byte stream.
    fn into_chunk_stream(
        self,
    ) -> impl Future<Output = Result<Self::Stream, error::Error>> + use<Self>;
}

/// An HTTP response that can expose its JSON body through a `Send + 'static` future.
///
/// This is required for owned query APIs, whose returned futures are spawnable.
pub trait QueryHttpResponse: HttpResponse + Send + 'static {
    /// Deserialize the response body as JSON through a `Send + 'static` future.
    fn json_send<T>(self) -> impl Future<Output = Result<T, error::Error>> + Send + 'static
    where
        T: DeserializeOwned + 'static;
}

/// A chunked HTTP response that can expose its async byte stream through a `Send + 'static`
/// future.
pub trait QueryChunkedHttpResponse:
    QueryHttpResponse + ChunkedHttpResponse<Stream: Send + 'static>
{
    /// Convert the response body into an async byte stream through a `Send + 'static` future.
    fn into_chunk_stream_send(
        self,
    ) -> impl Future<Output = Result<Self::Stream, error::Error>> + Send + 'static;
}

/// An HTTP client that can service owned query APIs through spawnable futures.
pub trait QueryHttpClient:
    HttpClient<Response: QueryHttpResponse> + Clone + Send + 'static
{
    /// Send a GET request for owned query APIs.
    fn send_get(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static;

    /// Send a POST request for owned query APIs.
    fn send_post(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static;
}

/// An HTTP client that can service write requests through spawnable futures.
pub trait WriteHttpClient: HttpClient {
    /// Send a POST request for write APIs, returning the status code and response body text.
    fn post_send(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<(u16, String), error::Error>> + Send + 'static + use<Self>;
}

/// An abstract HTTP client that can service InfluxDB requests.
pub trait HttpClient {
    /// The response type produced by this client.
    type Response: HttpResponse;

    /// Send a GET request.
    fn get(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'_, Self>;

    /// Send a POST request.
    fn post(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'_, Self>;
}
