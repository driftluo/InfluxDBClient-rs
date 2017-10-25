//! # InfluxDB Client
//! InfluxDB is an open source time series database with no external dependencies.
//! It's useful for recording metrics, events, and performing analytics.
//!
//! ## Usage
//!
//! ```Rust
//! extern crate influx_db_client;
//!
//! use influx_db_client::{InfluxdbClient, Point, Points, Value, InfluxClient, Precision};
//!
//! fn main() {
//!     let mut client = InfluxdbClient::new("http://localhost:8086", "test", "root", "root");
//!     client.set_write_timeout(10);
//!     client.set_read_timeout(10);
//!
//!     let mut point = Point::new("test");
//!     point.add_field("somefield", Value::Integer(65));
//!
//!     let mut point1 = Point::new("test2");
//!     point1.add_field("somefield", Value::Float(12.2));
//!     point1.add_tag("sometag", Value::Boolean(false));
//!
//!     let mut points = Points::new(point);
//!     points.push(point1);
//!
//!     // if Precision is None, the default is second
//!     // Multiple write
//!     let res = client.write_points(points, Some(Precision::Microseconds), None).unwrap();
//!     let version = client.get_version().unwrap();
//!     println!("{}\nversion:{}", res, version)
//!
//!     // query
//!     let res = client.query("select * from test", None).unwrap();
//!     println!("{:?}", res[0].get("series").unwrap()[0].get("values"))
//! }
//! ```

#![deny(warnings)]

extern crate hyper;
extern crate serde_json;
extern crate hyper_native_tls;
extern crate serde;
#[macro_use]
extern crate serde_derive;

pub mod serialization;
pub mod error;
pub mod client;
pub mod keys;

pub use client::{ Client, UdpClient };
pub use keys::{ Point, Precision, Points, Value, Node, Query, Series };
pub use error::Error;
