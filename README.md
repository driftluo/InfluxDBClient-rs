# InfluxDBClient-rs

![image](https://img.shields.io/crates/v/influx_db_client.svg)
[![Build Status](https://api.travis-ci.org/driftluo/InfluxDBClient-rs.svg?branch=master)](https://travis-ci.org/driftluo/InfluxDBClient-rs)

A easy-use client to influxdb

## Overview

This is an InfluxDB driver for Rust.

## Status

This project has been able to run properly, PR is welcome.

## Usage

### Use

```
[dependencies]
influx_db_client = "^0.7.0"
tokio = { version = "1", features = ["rt-multi-thread"] }
```

### http

```rust,no_run
use influx_db_client::{Client, Point, Points, Precision, point, points};

fn main() {
    // default with "http://127.0.0.1:8086", db with "test"
    let client = Client::default().set_authentication("root", "root");

    let point = point!("test1")
        .add_field("foo", "bar")
        .add_field("integer", 11)
        .add_field("float", 22.3)
        .add_field("'boolean'", false);

    let point1 = Point::new("test1")
        .add_tag("tags", "\\\"fda")
        .add_tag("number", 12)
        .add_tag("float", 12.6)
        .add_field("fd", "'3'")
        .add_field("quto", "\\\"fda")
        .add_field("quto1", "\"fda");

    let points = points!(point1, point);

    tokio::runtime::Runtime::new().unwrap().block_on(async move {
        // if Precision is None, the default is nanosecond
        // Multiple write
        client.write_points(points, Some(Precision::Seconds), None).await.unwrap();

        // query, it's type is Option<Vec<Node>>
        let res = client.query("select * from test1", None).await.unwrap();
        println!("{:?}", res.unwrap()[0].series)
    });
}
```

`Client` defaults to `reqwest::Client` when the default `reqwest` feature is enabled,
but it is generic over the HTTP implementation.
If you need a custom transport, implement the transport traits for the APIs you want to support,
then create it with `Client::new_with_client(...)`.
Borrowing APIs such as `ping_borrow`, `get_version_borrow`, `query_borrow`,
`query_chunked_borrow`, `write_point_borrow`, `write_points_borrow`, and the corresponding
query-backed `*_borrow` management APIs require `BorrowHttpClient` and `BorrowHttpResponse`.
Borrowed chunked queries also require `BorrowChunkedHttpResponse`. Spawn-safe APIs such as `ping`,
`get_version`, `query`, `query_chunked`, `write_point`, `write_points`, and the query-backed
management commands require `HttpClient` and `HttpResponse`. Spawn-safe chunked queries also
require `ChunkedHttpResponse`. You can implement borrowed-only, spawn-safe-only, or both modes on the same
transport type.
Borrowing APIs use borrowed `HttpRequest` data, while spawnable query/write APIs receive owned
`HttpRequest<'static>` values. Chunked responses now expose an async byte stream rather than a
blocking reader.

`query_chunked` is an incompatible API change in this release: it now returns an async stream
instead of a synchronous iterator. Add `futures = "0.3"` if you want to consume it with
`StreamExt::next`:

```rust,no_run
use futures::StreamExt;
use influx_db_client::{Client, Query};

# #[cfg(feature = "reqwest")] {
# tokio::runtime::Runtime::new().unwrap().block_on(async move {
let client = Client::default();
let mut stream = client.query_chunked("select * from test1", None).await.unwrap();

while let Some(result) = stream.next().await {
    let query: Query = result.unwrap();
    println!("{:?}", query.results);
}
# });
# }
```

To avoid compiling `reqwest`, disable default features and provide your own HTTP client:

```toml
[dependencies]
influx_db_client = { version = "^0.7.0", default-features = false }
```

The crate's default `reqwest/default-tls` path currently resolves to a rustls-based backend.
This is an incompatible feature-name update: the previous `rustls-tls*` feature names were removed.

To build the default `reqwest` transport with the `native-tls` backend, disable default features
and enable one of the `native-tls*` features explicitly. On Linux that typically means OpenSSL;
on macOS and Windows it uses the platform TLS stack:

```toml
[dependencies]
influx_db_client = { version = "^0.7.0", default-features = false, features = ["native-tls"] }
```

### udp

```rust,no_run
use influx_db_client::{Point, UdpClient, point};

fn main() {
    let mut udp = UdpClient::new("127.0.0.1:8089".parse().unwrap());
    udp.add_host("127.0.0.1:8090".parse().unwrap());

    let point = point!("test").add_field("foo", "bar");

    udp.write_point(point).unwrap();
}
```

## Compatibility

This is the [API Document](https://docs.influxdata.com/influxdb/v1.2/tools/api/), it may apply to version 1.0 or higher.

I have tested it in version 1.0.2/1.3.5/1.5.

## Thanks

Because [**influent**](https://github.com/gobwas/influent.rs) seems to have no longer updated, and only support to the 0.9 version. I read **influent.rs** and [**influxdb-python**](https://github.com/influxdata/influxdb-python) source, and then try to write a library for 1.0+ version for support for my own use.
