extern crate hyper;

use hyper::client::Client;
use hyper::Url;
use std::collections::BTreeMap;
use std::io::Read;


pub enum Value {
    String(String),
    Float(f64),
    Integer(i64),
    Boolean(bool)
}

pub struct Point {
    pub measurement: String,
    pub tags: BTreeMap<String, Value>,
    pub fields: BTreeMap<String, Value>,
    pub timestamp: Option<i64>
}

pub struct Points {
    pub point: Vec<Point>
}

pub enum Precision {
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
    Minutes,
    Hours
}

trait Tostr {
    fn to_str(&self) -> &str;
}

impl Tostr for Precision {
    fn to_str(&self) -> &str {
        match *self {
            Precision::Nanoseconds => "n",
            Precision::Microseconds => "u",
            Precision::Milliseconds => "ms",
            Precision::Seconds => "s",
            Precision::Minutes => "m",
            Precision::Hours => "h"
        }
    }
}

pub struct InfluxdbClient<'a> {
    host: &'a str,
    db: &'a str,
    username: &'a str,
    passwd: &'a str,
}

pub trait InfluxClient {
    fn ping(&self) -> bool;
    fn get_version(&self) -> Option<String>;
    fn write_point(&self, Point, Option<Precision>, Option<&str>) -> Result<bool, String>;
    fn write_points(&self, Points, Option<Precision>, Option<&str>) -> Result<bool, String>;
}

impl<'a> InfluxdbClient<'a> {
    pub fn new(host: &'a str, db: &'a str, username: &'a str, passwd: &'a str) -> InfluxdbClient<'a> {
        InfluxdbClient {
            host: host,
            db: db,
            username: username,
            passwd: passwd,
        }
    }
}

impl<'a> InfluxClient for InfluxdbClient<'a> {
    fn ping(&self) -> bool {
        let ping = Client::new();
        let url = Url::parse(self.host).unwrap();
        let url = url.join("ping").unwrap();
        let res = ping.get(url).send().unwrap();
        match res.status_raw().0 {
            204 => true,
            _ => false,
        }
    }

    fn get_version(&self) -> Option<String> {
        let ping = Client::new();
        let url = Url::parse(self.host).unwrap();
        let url = url.join("ping").unwrap();
        let res = ping.get(url).send().unwrap();
        match res.status_raw().0 {
            204 => match res.headers.get_raw("X-Influxdb-Version") {
                Some(i) => Some(String::from_utf8(i[0].to_vec()).unwrap()),
                None => Some(String::from("Don't know"))
            },
            _ => None,
        }
    }

    fn write_point(&self, point: Point, precision: Option<Precision>, rp: Option<&str>) -> Result<bool, String> {
        let points = Points::new(point);
        self.write_points(points, precision, rp)
    }

    fn write_points(&self, points: Points, precision: Option<Precision>, rp: Option<&str>) -> Result<bool, String> {
        let mut line = Vec::new();
        for point in points.point {
            line.push(point.measurement);

            for (tag, value) in point.tags.iter() {
                line.push(",".to_string());
                line.push(tag.to_string());
                line.push("=".to_string());

                match value {
                    &Value::String(ref s) => line.push(s.to_string()),
                    &Value::Float(ref f) => line.push(f.to_string()),
                    &Value::Integer(ref i) => line.push(i.to_string()),
                    &Value::Boolean(b) => line.push({ if b { "true".to_string() } else { "false".to_string() } })
                }
            }

            let mut was_first = true;

            for (field, value) in point.fields.iter() {
                line.push({
                    if was_first {
                        was_first = false;
                        " "
                    } else { "," }
                }.to_string());
                line.push(field.to_string());
                line.push("=".to_string());

                match value {
                    &Value::String(ref s) => line.push(s.to_string()),
                    &Value::Float(ref f) => line.push(f.to_string()),
                    &Value::Integer(ref i) => line.push(i.to_string()),
                    &Value::Boolean(b) => line.push({ if b { "true".to_string() } else { "false".to_string() } })
                }
            }

            match point.timestamp {
                Some(t) => {
                    line.push(" ".to_string());
                    line.push(t.to_string());
                }
                _ => {}
            }

            line.push("\n".to_string())
        }

        let line = line.join("");

        let mut param = vec![("db", self.db), ("u", self.username), ("p", self.passwd)];

        match precision {
            Some(ref t) => param.push(("precision", t.to_str())),
            None => param.push(("precision", "s")),
        };

        match rp {
            Some(t) => param.push(("rp", t)),
            None => (),
        };

        let cleint = Client::new();
        let url = Url::parse(self.host).unwrap();
        let url = url.join("write").unwrap();
        let url = Url::parse_with_params(url.as_str(), &param).unwrap();

        let mut res = cleint.post(url).body(&line).send().unwrap();
        let mut err = String::new();
        let _ = res.read_to_string(&mut err);

        match res.status_raw().0 {
            204 => Ok(true),
            400 => Err(err),
            401 => Err("Invalid authentication credentials.".to_string()),
            404 => Err(err),
            500 => Err(err),
            _ => Err("There is something wrong".to_string())
        }
    }
}

impl Point {
    pub fn new(measurement: &str) -> Point {
        Point {
            measurement: String::from(measurement),
            tags: BTreeMap::new(),
            fields: BTreeMap::new(),
            timestamp: None,
        }
    }

    pub fn add_tag(&mut self, tag: &str, value: Value) {
        self.tags.insert(tag.to_string(), value);
    }

    pub fn add_field(&mut self, field: &str, value: Value) {
        self.fields.insert(field.to_string(), value);
    }

    pub fn add_timestamp(&mut self, timestamp: i64) {
        self.timestamp = Some(timestamp);
    }
}

impl Points {
    pub fn new(point: Point) -> Points {
        let mut points = Vec::new();
        points.push(point);
        Points {
            point: points,
        }
    }

    pub fn push(&mut self, point: Point) {
        self.point.push(point)
    }
}
