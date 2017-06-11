# InfluxDBClient-rs
A easy-use client to influxdb

## Overview

This is an InfluxDB driver for Rust.

## Usage

```
extern crate influx_db_client;

use influx_db_client::{InfluxdbClient, Point, Points, Value, InfluxClient};

fn main() {
    let client = InfluxdbClient::new("http://localhost:8086", "test", "root", "root");
    let mut point = Point::new("test_measurement");
    point.add_field("field", Value::String("Hello".to_string());
    let points = Points::new(point);
    let res = client.write_points(points).unwrap();
    let version = client.get_version().unwrap();
}
```

## Compatibility

This is the [API Document](https://docs.influxdata.com/influxdb/v1.2/tools/api/), it may apply to version 1.0 or higher.

## Thanks

Because influent seems to have no longer updated, influxdb only support to the 0.9 version, I read influent.rs and influxdb-python source, and then try to write a library for 1.0+ version for support for my own use

I have tested it in version 1.0.2.