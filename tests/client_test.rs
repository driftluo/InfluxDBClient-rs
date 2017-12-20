#[macro_use]
extern crate influx_db_client;
extern crate native_tls;

use influx_db_client::{ Client, UdpClient, Point, Points, Precision, Value, TLSOption };
use native_tls::{ TlsConnector, Certificate };
use std::thread::sleep;
use std::time::Duration;
use std::io::Read;
use std::fs::File;

#[test]
fn create_and_delete_database() {
    let client = Client::default().set_authentication("root", "root");

    let _ = client.create_database("temporary").unwrap();

    let _ = client.drop_database("temporary").unwrap();
}

#[test]
fn create_and_delete_measurement() {
    let client = Client::default().set_authentication("root", "root");
    let mut point = Point::new("temporary");
    point.add_field("foo", Value::String("bar".to_string()));
    point.add_field("integer", Value::Integer(11));
    point.add_field("float", Value::Float(22.3));
    point.add_field("'boolean'", Value::Boolean(false));

    let _ = client.write_point(point,Some(Precision::Seconds), None).unwrap();

    let _ = client.drop_measurement("temporary").unwrap();
}

#[test]
fn use_points() {
    let client = Client::default().set_authentication("root", "root");
    let mut point = Point::new("test1");
    point.add_field("foo", Value::String("bar".to_string()));
    point.add_field("integer", Value::Integer(11));
    point.add_field("float", Value::Float(22.3));
    point.add_field("'boolean'", Value::Boolean(false));

    let mut point1 = Point::new("test2");
    point1.add_tag("tags", Value::String(String::from("'=213w")));
    point1.add_tag("number", Value::Integer(12));
    point1.add_tag("float", Value::Float(12.6));
    point1.add_field("fd", Value::String("'3'".to_string()));

    let points = Points::create_new(vec![point1, point]);

    let _ = client.write_points(points, Some(Precision::Seconds), None).unwrap();

    let _ = sleep(Duration::from_secs(3));

    let _ = client.drop_measurement("test1").unwrap();
    let _ = client.drop_measurement("test2").unwrap();
}

#[test]
fn query() {
    let client = Client::default().set_authentication("root", "root");
    let mut point = Point::new("test3");
    point.add_field("foo", Value::String("bar".to_string()));
    let mut point1 = point.clone();
    point.add_timestamp(1508981970);
    point1.add_timestamp(1508982026);

    let _ = client.write_point(point, None, None);

    let _ = client.query("select * from test3", None).unwrap();

    let _ = client.write_point(point1, None, None).unwrap();

    let _ = client.drop_measurement("test3").unwrap();
}

#[test]
fn use_macro() {
    let client = Client::default().set_authentication("root", "root");
    let mut point = point!("test4");
    point.add_field("foo", Value::String("bar".to_string()));
    let mut point1 = point.clone();
    point.add_timestamp(1508981970);
    point1.add_timestamp(1508982026);

    let points = points![point, point1];
    let _ = client.write_points(points, None, None);

    let _ = client.query("select * from test4", None).unwrap();

    let _ = client.drop_measurement("test4").unwrap();
}

#[test]
fn use_udp() {
    let mut udp = UdpClient::new("127.0.0.1:8089");
    udp.add_host("127.0.0.1:8090");
    let mut client = Client::default().set_authentication("root", "root");

    let mut point = point!("test");
    point.add_field("foo", Value::String(String::from("bar")));

    let _ = udp.write_point(point).unwrap();

    let _ = sleep(Duration::from_secs(1));
    client.swith_database("udp");
    let _ = client.drop_measurement("test").unwrap();
    client.swith_database("telegraf");
    let _ = client.drop_measurement("test").unwrap();
}

#[test]
fn use_https() {
    let mut ca_cert_file = File::open("/etc/ssl/influxdb-selfsigned.crt").unwrap();
    let mut ca_cert_buffer = Vec::new();
    ca_cert_file.read_to_end(&mut ca_cert_buffer).unwrap();

    let mut builder = TlsConnector::builder().unwrap();
    builder.add_root_certificate(Certificate::from_der(&ca_cert_buffer).unwrap()).unwrap();

    let tls_connector = TLSOption::new(builder.build().unwrap());

    let client = Client::new_with_option("https://127.0.0.1:8086", "test", Some(tls_connector));

    let mut point = point!("foo");
    point.add_field("foo", Value::String(String::from("bar")));

    let _ = client.write_point(point, None, None).unwrap();

    let _ = client.query("select * from foo", None).unwrap();

    let _ = client.drop_measurement("foo").unwrap();
}
