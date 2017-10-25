extern crate influx_db_client;

use influx_db_client::{ Client, Point, Points, Precision, Value };
use std::thread::sleep;
use std::time::Duration;

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

    let _ = client.write_points(points,Some(Precision::Seconds), None).unwrap();

    let _ = sleep(Duration::from_secs(3));

    let _ = client.drop_measurement("test1").unwrap();
    let _ = client.drop_measurement("test2").unwrap();
}
