//! # InfluxDB Client
//! InfluxDB is an open source time series database with no external dependencies.
//! It's useful for recording metrics, events, and performing analytics.
//!
//! ## Usage
//!
//! ### http
//!
//! ```rust,no_run
//! # #[cfg(feature = "reqwest")] {
//! use influx_db_client::{Client, Point, Points, Precision, point, points};
//!
//! // default with "http://127.0.0.1:8086", db with "test"
//! let client = Client::default().set_authentication("root", "root");
//!
//! let point = point!("test1")
//!       .add_field("foo", "bar")
//!       .add_field("integer", 11)
//!       .add_field("float", 22.3)
//!       .add_field("'boolean'", false);
//!
//! let point1 = Point::new("test1")
//!       .add_tag("tags", "\\\"fda")
//!       .add_tag("number", 12)
//!       .add_tag("float", 12.6)
//!       .add_field("fd", "'3'")
//!       .add_field("quto", "\\\"fda")
//!       .add_field("quto1", "\"fda");
//!
//! let points = points!(point1, point);
//!
//! tokio::runtime::Runtime::new().unwrap().block_on(async move {
//!     // if Precision is None, the default is nanosecond
//!     // Multiple write
//!     client.write_points(points, Some(Precision::Seconds), None).await.unwrap();
//!
//!     // query, it's type is Option<Vec<Node>>
//!     let res = client.query("select * from test1", None).await.unwrap();
//!     println!("{:?}", res.unwrap()[0].series);
//! });
//! # }
//! ```
//!
//! `Client` defaults to `reqwest::Client` when the default `reqwest` feature is enabled,
//! but it is generic over the HTTP implementation.
//! Implement the transport traits for the APIs you want to support, then plug the transport into
//! [`Client::new_with_client`]. Borrowing APIs such as [`Client::ping_borrow`],
//! [`Client::get_version_borrow`], [`Client::query_borrow`], [`Client::query_chunked_borrow`],
//! [`Client::write_point_borrow`], and [`Client::write_points_borrow`] require [`BorrowHttpClient`]
//! and [`BorrowHttpResponse`]. Borrowed chunked queries also require
//! [`BorrowChunkedHttpResponse`]. Spawn-safe APIs such as [`Client::ping`],
//! [`Client::get_version`], [`Client::query`], [`Client::query_chunked`],
//! [`Client::write_point`], and [`Client::write_points`] plus the query-backed management
//! commands require [`HttpClient`] and [`HttpResponse`]. Spawn-safe chunked queries also require
//! [`ChunkedHttpResponse`]. You can implement borrowed-only, spawn-safe-only, or both modes on
//! the same transport type.
//! Borrowing APIs pass borrowed [`HttpRequest`] data through the borrow transport traits, while
//! the spawnable query/write APIs pass owned `HttpRequest<'static>` values.
//! Chunked responses now yield an async byte stream instead of a blocking reader.
//! Disable default features if you want to avoid compiling `reqwest` and provide your own
//! HTTP client.
//! The default `reqwest/default-tls` path currently resolves to rustls.
//! This is an incompatible feature-name update: the previous `rustls-tls*` feature names were
//! removed. Disable default features and enable `native-tls` to use reqwest's native-tls backend.
//! On Linux that typically means OpenSSL; on macOS and Windows it uses the platform TLS stack.
//!
//! `query_chunked` is also an incompatible change now: it returns an async [`futures::Stream`]
//! instead of a synchronous iterator. Import `futures::StreamExt` to consume it with
//! `.next().await`.
//!
//! ```rust,compile_fail
//! use influx_db_client::{
//!     QueryChunkedHttpResponse, QueryHttpClient, QueryHttpResponse, WriteHttpClient,
//! };
//! ```
//!
//! ### udp
//!
//! ```rust,no_run
//! use influx_db_client::{Point, UdpClient, point};
//!
//! let mut udp = UdpClient::new("127.0.0.1:8089".parse().unwrap());
//! udp.add_host("127.0.0.1:8090".parse().unwrap());
//!
//! let point = point!("test").add_field("foo", "bar");
//!
//! udp.write_point(point).unwrap();
//! ```

#![deny(warnings)]
#![deny(missing_docs)]

/// All API on influxdb client, Including udp, http
pub mod client;
/// Error module
pub mod error;
/// HTTP transport abstraction types and traits.
pub mod http;
/// Points and Query Data Deserialize
pub mod keys;
/// Serialization module
pub(crate) mod serialization;

pub use client::{Client, UdpClient};
pub use error::Error;
pub use http::{
    BorrowChunkedHttpResponse, BorrowHttpClient, BorrowHttpResponse, ChunkedHttpResponse,
    HttpClient, HttpMethod, HttpRequest, HttpResponse,
};
pub use keys::{ChunkedQuery, Node, Point, Points, Precision, Query, Series, Value};
pub use url::Url;

#[cfg(feature = "reqwest")]
pub use reqwest;
