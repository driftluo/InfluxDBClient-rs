use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Float(f64),
    Integer(i64),
    Boolean(bool)
}

#[derive(Debug, Clone)]
pub struct Point {
    pub measurement: String,
    pub tags: HashMap<String, Value>,
    pub fields: HashMap<String, Value>,
    pub timestamp: Option<i64>
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
    pub fn add_tag<T: ToString>(&mut self, tag: T, value: Value) {
        self.tags.insert(tag.to_string(), value);
    }

    /// Add a field and its value
    pub fn add_field<T: ToString>(&mut self, field: T, value: Value) {
        self.fields.insert(field.to_string(), value);
    }

    /// Set the specified timestamp
    pub fn add_timestamp(&mut self, timestamp: i64) {
        self.timestamp = Some(timestamp);
    }

    /// Create a complete point
    pub fn create_new(measuremnet: &str, tags: HashMap<String, Value>, fields: HashMap<String, Value>, timestamp: i64) -> Self {
        Point {
            measurement: String::from(measuremnet),
            tags: tags,
            fields: fields,
            timestamp: Some(timestamp),
        }
    }
}

#[derive(Debug)]
pub struct Points {
    pub point: Vec<Point>
}

impl Points {
    /// Create a new points
    pub fn new(point: Point) -> Points {
        let mut points = Vec::new();
        points.push(point);
        Points {
            point: points,
        }
    }

    /// Insert point into already existing points
    pub fn push(&mut self, point: Point) {
        self.point.push(point)
    }

    /// Create a multi Points more directly
    pub fn create_new(points: Vec<Point>) -> Points {
        Points {
            point: points
        }
    }
}

#[derive(Debug)]
pub enum Precision {
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
    Minutes,
    Hours
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
            Precision::Hours => "h"
        }
    }
}
