//! # InfluxDB Client
//! InfluxDB is an open source time series database with no external dependencies.
//! It's useful for recording metrics, events, and performing analytics.
//!
//! ## Usage
//!
//! ### http
//!
//! ```Rust
//! use influx_db_client::{Client, Point, Points, Value, Precision};
//!
//! // default with "http://127.0.0.1:8086", db with "test"
//! let client = Client::default().set_authentication("root", "root");
//!
//! let mut point = point!("test1");
//! let point = point
//!       .add_field("foo", Value::String("bar".to_string()))
//!       .add_field("integer", Value::Integer(11))
//!       .add_field("float", Value::Float(22.3))
//!       .add_field("'boolean'", Value::Boolean(false));
//!
//! let point1 = Point::new("test1")
//!       .add_tag("tags", Value::String(String::from("\\\"fda")))
//!       .add_tag("number", Value::Integer(12))
//!       .add_tag("float", Value::Float(12.6))
//!       .add_field("fd", Value::String("'3'".to_string()))
//!       .add_field("quto", Value::String("\\\"fda".to_string()))
//!       .add_field("quto1", Value::String("\"fda".to_string()));
//!
//! let points = points!(point1, point);
//!
//! // if Precision is None, the default is second
//! // Multiple write
//! client.write_points(points, Some(Precision::Seconds), None).unwrap();
//!
//! // query, it's type is Option<Vec<Node>>
//! let res = client.query("select * from test1", None).unwrap();
//! println!("{:?}", res.unwrap()[0].series)
//! ```
//!
//! ### udp
//!
//! ```Rust
//! use influx_db_client::{UdpClient, Point, Value};
//!
//! let mut udp = UdpClient::new("127.0.0.1:8089".parse().unwrap());
//! udp.add_host("127.0.0.1:8090".parse().unwrap());
//!
//! let mut point = point!("test");
//! point.add_field("foo", Value::String(String::from("bar")));
//!
//! udp.write_point(point).unwrap();
//! ```

#![deny(warnings)]
#![deny(missing_docs)]

#[macro_use]
extern crate serde_derive;

/// All API on influxdb client, Including udp, http
pub mod client;
/// Error module
pub mod error;
/// Points and Query Data Deserialize
pub mod keys;
/// Serialization module
pub(crate) mod serialization;

pub use client::{Client, UdpClient};
pub use error::Error;
pub use keys::{ChunkedQuery, Node, Point, Points, Precision, Query, Series, Value};

pub use reqwest;
