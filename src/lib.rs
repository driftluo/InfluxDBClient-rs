//! # InfluxDB Client
//! InfluxDB is an open source time series database with no external dependencies.
//! It's useful for recording metrics, events, and performing analytics.
//!
//! ## Usage
//!
//! ### http
//!
//! ```Rust
//! #[macro_use]
//! extern crate influx_db_client;
//!
//! use influx_db_client::{Client, Point, Points, Value, Precision};
//!
//! fn main() {
//!         // default with "http://127.0.0.1:8086", db with "test"
//!         let client = Client::default().set_authentication("root", "root");
//!
//!         let mut point = point!("test1");
//!         point.add_field("foo", Value::String("bar".to_string()));
//!         point.add_field("integer", Value::Integer(11));
//!         point.add_field("float", Value::Float(22.3));
//!         point.add_field("'boolean'", Value::Boolean(false));
//!
//!         let mut point1 = point!("test2");
//!         point1.add_tag("tags", Value::String(String::from("'=213w")));
//!         point1.add_tag("number", Value::Integer(12));
//!         point1.add_tag("float", Value::Float(12.6));
//!         point1.add_field("fd", Value::String("'3'".to_string()));
//!
//!         let points = points!(point1, point);
//!
//!         // if Precision is None, the default is second
//!         // Multiple write
//!         let _ = client.write_points(points, Some(Precision::Seconds), None).unwrap();
//!
//!         // query, it's type is Option<Vec<Node>>
//!         let res = client.query("select * from test1", None).unwrap();
//!         println!("{:?}", res.unwrap()[0].series)
//! }
//! ```
//!
//! ### udp
//!
//! ```Rust
//! #[macro_use]
//! extern crate influx_db_client;
//!
//! use influx_db_client::{UdpClient, Point, Value};
//!
//! fn main() {
//!     let mut udp = UdpClient::new("127.0.0.1:8089");
//!     udp.add_host("127.0.0.1:8090");
//!
//!     let mut point = point!("test");
//!     point.add_field("foo", Value::String(String::from("bar")));
//!
//!     let _ = udp.write_point(point).unwrap();
//! }
//! ```

#![deny(warnings)]
#![deny(missing_docs)]

extern crate hyper;
extern crate hyper_native_tls;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

/// Serialization module
pub mod serialization;
/// Error module
pub mod error;
/// All API on influxdb client, Including udp, http
pub mod client;
/// Points and Query Data Deserialize
pub mod keys;

pub use client::{Client, TLSOption, UdpClient};
pub use keys::{Node, Point, Points, Precision, Query, Series, Value};
pub use error::Error;
