# InfluxDBClient-rs

![image](https://img.shields.io/crates/v/influx_db_client.svg)
[![Build Status](https://travis-ci.org/driftluo/InfluxDBClient-rs.svg?branch=master)](https://travis-ci.org/driftluo/InfluxDBClient-rs)

A easy-use client to influxdb

## Overview

This is an InfluxDB driver for Rust.

## Status

This project has been able to run properly, PR is welcome.

## Usage

### Recommend

```
[dependencies]
influx_db_client = "^0.3.2"

[patch.crates-io]
influx_db_client = { git = 'https://github.com/driftluo/InfluxDBClient-rs' }
```

### http

```Rust
#[macro_use]
extern crate influx_db_client;

use influx_db_client::{Client, Point, Points, Value, Precision};

fn main() {
    // default with "http://127.0.0.1:8086", db with "test"
    let client = Client::default().set_authentication("root", "root");

    let mut point = point!("test1");
    point.add_field("foo", Value::String("bar".to_string()));
    point.add_field("integer", Value::Integer(11));
    point.add_field("float", Value::Float(22.3));
    point.add_field("'boolean'", Value::Boolean(false));

    let mut point1 = Point::new("test1");
    point1.add_tag("tags", Value::String(String::from("'=213w")));
    point1.add_tag("number", Value::Integer(12));
    point1.add_tag("float", Value::Float(12.6));
    point1.add_field("fd", Value::String("'3'".to_string()));

    let points = points!(point1, point);

    // if Precision is None, the default is second
    // Multiple write
    let _ = client.write_points(points, Some(Precision::Seconds), None).unwrap();

    // query, it's type is Option<Vec<Node>>
    let res = client.query("select * from test1", None).unwrap();
    println!("{:?}", res.unwrap()[0].series)
}
```

### udp

```Rust
#[macro_use]
extern crate influx_db_client;

use influx_db_client::{UdpClient, Point, Value};

fn main() {
    let mut udp = UdpClient::new("127.0.0.1:8089");
    udp.add_host("127.0.0.1:8090");

    let mut point = point!("test");
    point.add_field("foo", Value::String(String::from("bar")));

    let _ = udp.write_point(point).unwrap();
}
```

## Compatibility

This is the [API Document](https://docs.influxdata.com/influxdb/v1.2/tools/api/), it may apply to version 1.0 or higher.

I have tested it in version 1.0.2 and 1.3.5.

## Thanks

Because [**influent**](https://github.com/gobwas/influent.rs) seems to have no longer updated, and only support to the 0.9 version. I read **influent.rs** and [**influxdb-python**](https://github.com/influxdata/influxdb-python) source, and then try to write a library for 1.0+ version for support for my own use.
