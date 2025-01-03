use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::HashMap,
    iter::{FromIterator, Iterator},
    slice::Iter,
};

/// Influxdb value, Please look at [this address](https://docs.influxdata.com/influxdb/v1.3/write_protocols/line_protocol_reference/)
#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Value<'a> {
    /// string
    String(Cow<'a, str>),
    /// Integer
    Integer(i64),
    /// float
    Float(f64),
    /// Bool
    Boolean(bool),
}

/// influxdb point
#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct Point<'a> {
    /// measurement
    pub measurement: String,
    /// tags
    pub tags: HashMap<String, Value<'a>>,
    /// fields
    pub fields: HashMap<String, Value<'a>>,
    /// timestamp
    pub timestamp: Option<i64>,
}

impl<'a> Point<'a> {
    /// Create a new point
    pub fn new(measurement: &'_ str) -> Self {
        Self {
            measurement: String::from(measurement),
            tags: HashMap::new(),
            fields: HashMap::new(),
            timestamp: None,
        }
    }

    /// Add a tag and its value
    pub fn add_tag<T: Into<String>, F: Into<Value<'a>>>(mut self, tag: T, value: F) -> Self {
        self.tags.insert(tag.into(), value.into());
        self
    }

    /// Add a field and its value
    pub fn add_field<T: Into<String>, F: Into<Value<'a>>>(mut self, field: T, value: F) -> Self {
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
pub struct Points<'a> {
    /// points
    pub point: Vec<Point<'a>>,
}

impl<'a> Points<'a> {
    /// Create a new points
    pub fn new(point: Point) -> Points {
        Points { point: vec![point] }
    }

    /// Insert point into already existing points
    pub fn push(mut self, point: Point<'a>) -> Self {
        self.point.push(point);
        self
    }

    /// Create a multi Points more directly
    pub fn create_new(points: Vec<Point>) -> Points {
        Points { point: points }
    }
}

impl<'a, 'b> IntoIterator for &'a Points<'b> {
    type Item = &'a Point<'b>;
    type IntoIter = Iter<'a, Point<'b>>;

    fn into_iter(self) -> Iter<'a, Point<'b>> {
        self.point.iter()
    }
}

impl<'a> FromIterator<Point<'a>> for Points<'a> {
    fn from_iter<T: IntoIterator<Item = Point<'a>>>(iter: T) -> Self {
        let mut points = Vec::new();

        for point in iter {
            points.push(point);
        }

        Points { point: points }
    }
}

impl<'a> Iterator for Points<'a> {
    type Item = Point<'a>;

    fn next(&mut self) -> Option<Point<'a>> {
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

impl<'a> From<String> for Value<'a> {
    fn from(v: String) -> Self {
        Self::String(Cow::Owned(v))
    }
}

impl<'a> From<&'a str> for Value<'a> {
    fn from(v: &'a str) -> Self {
        Self::String(Cow::Borrowed(v))
    }
}

impl<'a> From<i64> for Value<'a> {
    fn from(v: i64) -> Self {
        Self::Integer(v)
    }
}

impl<'a> From<i32> for Value<'a> {
    fn from(v: i32) -> Self {
        Self::Integer(v.into())
    }
}

impl<'a> From<i16> for Value<'a> {
    fn from(v: i16) -> Self {
        Self::Integer(v.into())
    }
}

impl<'a> From<i8> for Value<'a> {
    fn from(v: i8) -> Self {
        Self::Integer(v.into())
    }
}

impl<'a> From<f64> for Value<'a> {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl<'a> From<f32> for Value<'a> {
    fn from(v: f32) -> Self {
        Self::Float(v.into())
    }
}

impl<'a> From<bool> for Value<'a> {
    fn from(v: bool) -> Self {
        Self::Boolean(v)
    }
}
