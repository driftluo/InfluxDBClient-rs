# InfluxDBClient-rs
A easy-use client to influxdb

## Overview

This is an InfluxDB driver for Rust.

## Usage

```
extern crate influx_db_client;

use influx_db_client::{InfluxdbClient, Point, Points, Value, InfluxClient, Precision};

fn main() {
    let client = InfluxdbClient::new("http://localhost:8086", "test", "root", "root");
    let mut point = Point::new("test");
    point.add_field("somefield", Value::Integer(65));

    let mut point1 = Point::new("test2");
    point1.add_field("somefield", Value::Float(12.2));
    point1.add_tag("sometag", Value::Boolean(false));

    let mut points = Points::new(point);
    points.push(point1);

    // if Precision is None, the default is second
    let res = client.write_points(points, Some(Precision::Microseconds)).unwrap();
    let version = client.get_version().unwrap();
    println!("{}\nversion:{}", res, version)
}
```

## Compatibility

This is the [API Document](https://docs.influxdata.com/influxdb/v1.2/tools/api/), it may apply to version 1.0 or higher.

## Thanks

Because [**influent**](https://github.com/gobwas/influent.rs) seems to have no longer updated, and only support to the 0.9 version. I read **influent.rs** and [**influxdb-python**](https://github.com/influxdata/influxdb-python) source, and then try to write a library for 1.0+ version for support for my own use

I have tested it in version 1.0.2.