use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::iter::FromIterator;
use std::iter::Iterator;

/// Influxdb value, Please look at [this address](https://docs.influxdata.com/influxdb/v1.3/write_protocols/line_protocol_reference/)
#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Value {
    /// string
    String(String),
    /// Integer
    Integer(i64),
    /// float
    Float(f64),
    /// Bool
    Boolean(bool),
}

/// influxdb point
#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct Point {
    /// measurement
    pub measurement: String,
    /// tags
    pub tags: HashMap<String, Value>,
    /// fields
    pub fields: HashMap<String, Value>,
    /// timestamp
    pub timestamp: Option<i64>,
}

impl Point {
    /// Create a new point
    pub fn new(measurement: &str) -> Point {
        Point {
            measurement: String::from(measurement),
            tags: HashMap::new(),
            fields: HashMap::new(),
            timestamp: None,
        }
    }

    /// Add a tag and its value
    pub fn add_tag<T: Into<String>, F: Into<Value>>(mut self, tag: T, value: F) -> Self {
        self.tags.insert(tag.into(), value.into());
        self
    }

    /// Add a field and its value
    pub fn add_field<T: Into<String>, F: Into<Value>>(mut self, field: T, value: F) -> Self {
        self.fields.insert(field.into(), value.into());
        self
    }

    /// Set the specified timestamp
    pub fn add_timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
}

/// Points
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Points {
    /// points
    pub point: Vec<Point>,
}

impl Points {
    /// Create a new points
    pub fn new(point: Point) -> Points {
        let mut points = Vec::new();
        points.push(point);
        Points { point: points }
    }

    /// Insert point into already existing points
    pub fn push(mut self, point: Point) -> Self {
        self.point.push(point);
        self
    }

    /// Create a multi Points more directly
    pub fn create_new(points: Vec<Point>) -> Points {
        Points { point: points }
    }
}

impl FromIterator<Point> for Points {
    fn from_iter<T: IntoIterator<Item = Point>>(iter: T) -> Self {
        let mut points = Vec::new();

        for point in iter {
            points.push(point);
        }

        Points { point: points }
    }
}

impl Iterator for Points {
    type Item = Point;

    fn next(&mut self) -> Option<Point> {
        self.point.pop()
    }
}

/// Query data
#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct Query {
    /// query results
    pub results: Option<Vec<Node>>,
    /// fail message
    pub error: Option<String>,
}

/// Chunked Query data
pub type ChunkedQuery<'de, T> = serde_json::StreamDeserializer<'de, T, Query>;

/// Query data node
#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct Node {
    /// id
    pub statement_id: Option<u64>,
    /// series
    pub series: Option<Vec<Series>>,
}

/// Query data series
#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct Series {
    /// measurement
    pub name: Option<String>,
    /// tag
    pub tags: Option<serde_json::Map<String, serde_json::Value>>,
    /// field names and time
    pub columns: Vec<String>,
    /// values
    pub values: Option<Vec<Vec<serde_json::Value>>>,
}

/// Time accuracy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    /// n
    Nanoseconds,
    /// u
    Microseconds,
    /// ms
    Milliseconds,
    /// s
    Seconds,
    /// m
    Minutes,
    /// h
    Hours,
}

impl Precision {
    /// Convert Precision to &str
    pub fn to_str(&self) -> &str {
        match *self {
            Precision::Nanoseconds => "n",
            Precision::Microseconds => "u",
            Precision::Milliseconds => "ms",
            Precision::Seconds => "s",
            Precision::Minutes => "m",
            Precision::Hours => "h",
        }
    }
}

/// Create Points by macro
#[macro_export]
macro_rules! points {
    ($($x:expr),+) => {
        {
            let mut temp_vec = Vec::new();
            $(temp_vec.push($x);)*
            Points { point: temp_vec }
        }
    };
}

/// Create Point by macro
#[macro_export]
macro_rules! point {
    ($x:expr) => {{
        Point::new($x)
    }};
    ($x:expr, $y:expr, $z:expr) => {{
        Point {
            measurement: String::from($x),
            tags: $y,
            fields: $z,
            timestamp: None,
        }
    }};
    ($x:expr, $y:expr, $z:expr, $a:expr) => {{
        Point {
            measurement: String::from($x),
            tags: $y,
            fields: $z,
            timestamp: Some($a),
        }
    }};
}

impl From<String> for Value {
    fn from(v: String) -> Value {
        Value::String(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Value {
        Value::String(v.to_string())
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Value {
        Value::Integer(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Value {
        Value::Float(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Value {
        Value::Boolean(v)
    }
}
