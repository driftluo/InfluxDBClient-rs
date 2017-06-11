extern crate hyper;

use hyper::client::Client;
use hyper::Url;
use std::collections::BTreeMap;

pub trait InfluxClient {
    fn ping(&self) -> bool;
    fn get_version(&self) -> Option<String>;
    fn write_points(&self, Points) -> Result<bool, String>;
}

pub struct InfluxdbClient<'a> {
    host: &'a str,
    db: &'a str,
    username: &'a str,
    passwd: &'a str,
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

    fn write_points(&self, points: Points) -> Result<bool, String> {
        let mut line = Vec::new();
        for point in points.point {
            line.push(point.measurement);

            for (tag, value) in point.tags.iter() {
                line.push(",".to_string());
                line.push(tag.to_string());
                line.push("=".to_string());
                line.push(value.to_string());
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

        let cleint = Client::new();
        let url = Url::parse(self.host).unwrap();
        let url = url.join("write").unwrap();
        let url = Url::parse_with_params(url.as_str(), &[("db", self.db), ("u", self.username), ("p", self.passwd)]).unwrap();

        let res = cleint.post(url).body(&line).send().unwrap();
        match res.status_raw().0 {
            204 => Ok(true),
            _ => Err("there is something wrong".to_string())
        }
    }
}

#[allow(dead_code)]
pub enum Value {
    String(String),
    Float(f64),
    Integer(i64),
    Boolean(bool)
}

#[allow(dead_code)]
pub struct Point {
    pub measurement: String,
    pub tags: BTreeMap<String, String>,
    pub fields: BTreeMap<String, Value>,
    pub timestamp: Option<i64>
}

impl Point {
    pub fn new(measurement: &str) -> Point {
        Point{
            measurement: String::from(measurement),
            tags: BTreeMap::new(),
            fields: BTreeMap::new(),
            timestamp: None,
        }
    }

    pub fn add_tag(&mut self, tag: &str, value: &str) {
        self.tags.insert(tag.to_string(), value.to_string());
    }

    pub fn add_field(&mut self, field: &str, value: Value) {
        self.fields.insert(field.to_string(), value);
    }

    pub fn add_timestamp(&mut self, timestamp: i64) {
        self.timestamp = Some(timestamp);
    }
}

pub struct Points {
    pub point: Vec<Point>
}

impl Points {
    pub fn new(point: Point) -> Points {
        let mut points = Vec::new();
        points.push(point);
        Points{
            point: points,
        }
    }
}
