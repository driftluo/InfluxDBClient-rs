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
influx_db_client = "^0.5.0"
```

### http

```Rust
use influx_db_client::{
    Client, Point, Points, Value, Precision, point, points
};
use tokio;

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
        // if Precision is None, the default is second
        // Multiple write
        client.write_points(points, Some(Precision::Seconds), None).await.unwrap();

        // query, it's type is Option<Vec<Node>>
        let res = client.query("select * from test1", None).await.unwrap();
        println!("{:?}", res.unwrap()[0].series)
    });
}
```

### udp

```Rust
use influx_db_client::{UdpClient, Point, Value, point};

fn main() {
    let mut udp = UdpClient::new("127.0.0.1:8089");
    udp.add_host("127.0.0.1:8090");

    let point = point!("test").add_field("foo", Value::String(String::from("bar")));

    udp.write_point(point).unwrap();
}
```

## Compatibility

This is the [API Document](https://docs.influxdata.com/influxdb/v1.2/tools/api/), it may apply to version 1.0 or higher.

I have tested it in version 1.0.2/1.3.5/1.5.

## Thanks

Because [**influent**](https://github.com/gobwas/influent.rs) seems to have no longer updated, and only support to the 0.9 version. I read **influent.rs** and [**influxdb-python**](https://github.com/influxdata/influxdb-python) source, and then try to write a library for 1.0+ version for support for my own use.
