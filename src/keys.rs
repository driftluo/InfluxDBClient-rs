use crate::{error::Error, serialization};
use bytes::{Bytes, BytesMut};
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::HashMap,
    iter::FromIterator,
    pin::Pin,
    slice::Iter,
    task::{Context, Poll},
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

impl<'a> IntoIterator for Points<'a> {
    type Item = Point<'a>;
    type IntoIter = std::vec::IntoIter<Point<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.point.into_iter()
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

/// Query data
#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct Query {
    /// query results
    pub results: Option<Vec<Node>>,
    /// fail message
    pub error: Option<String>,
}

impl Query {
    pub(crate) fn error_message(&self) -> Option<&str> {
        self.error.as_deref().or_else(|| {
            self.results
                .as_ref()
                .and_then(|results| results.iter().find_map(|node| node.error.as_deref()))
        })
    }
}

pub(crate) fn query_syntax_error(query: &Query, fallback: &str) -> Error {
    Error::SyntaxError(serialization::conversion(
        query.error_message().unwrap_or(fallback),
    ))
}

pub(crate) fn ensure_query_success(query: Query) -> Result<Query, Error> {
    if let Some(err) = query.error_message() {
        Err(Error::SyntaxError(serialization::conversion(err)))
    } else {
        Ok(query)
    }
}

/// Chunked query stream that yields parsed query documents and surfaces query errors.
pub struct ChunkedQuery<S> {
    inner: S,
    buffer: BytesMut,
    scanner: JsonObjectScanner,
    stream_finished: bool,
    terminated: bool,
}

impl<S> ChunkedQuery<S> {
    pub(crate) fn new(inner: S) -> Self {
        Self {
            inner,
            buffer: BytesMut::new(),
            scanner: JsonObjectScanner::default(),
            stream_finished: false,
            terminated: false,
        }
    }
}

#[derive(Debug, Default)]
struct JsonObjectScanner {
    offset: usize,
    depth: usize,
    in_string: bool,
    escaped: bool,
    started: bool,
}

impl JsonObjectScanner {
    fn next_document_end(&mut self, bytes: &[u8]) -> Option<usize> {
        while self.offset < bytes.len() {
            let byte = bytes[self.offset];
            self.offset += 1;

            if self.in_string {
                if self.escaped {
                    self.escaped = false;
                } else if byte == b'\\' {
                    self.escaped = true;
                } else if byte == b'"' {
                    self.in_string = false;
                }

                continue;
            }

            match byte {
                b'{' => {
                    self.started = true;
                    self.depth += 1;
                }
                b'}' if self.started && self.depth > 0 => {
                    self.depth -= 1;
                    if self.depth == 0 {
                        let end = self.offset;
                        self.reset();
                        return Some(end);
                    }
                }
                b'"' if self.started => self.in_string = true,
                b if b.is_ascii_whitespace() && !self.started => {}
                _ if !self.started => self.started = true,
                _ => {}
            }
        }

        None
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl<S> Stream for ChunkedQuery<S>
where
    S: Stream<Item = Result<Bytes, Error>>,
{
    type Item = Result<Query, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            {
                let this = unsafe { self.as_mut().get_unchecked_mut() };

                if this.terminated {
                    return Poll::Ready(None);
                }

                if !this.buffer.is_empty() {
                    if let Some(end) = this.scanner.next_document_end(&this.buffer) {
                        let document = this.buffer.split_to(end).freeze();
                        match serde_json::from_slice::<Query>(&document) {
                            Ok(query) => return Poll::Ready(Some(ensure_query_success(query))),
                            Err(err) => {
                                this.terminated = true;
                                return Poll::Ready(Some(Err(Error::Communication(
                                    err.to_string(),
                                ))));
                            }
                        }
                    }

                    if this.stream_finished {
                        if this.buffer.iter().all(u8::is_ascii_whitespace) {
                            this.terminated = true;
                            return Poll::Ready(None);
                        }

                        match serde_json::from_slice::<Query>(&this.buffer) {
                            Ok(query) => {
                                this.buffer.clear();
                                this.scanner.reset();
                                return Poll::Ready(Some(ensure_query_success(query)));
                            }
                            Err(err) => {
                                this.terminated = true;
                                return Poll::Ready(Some(Err(Error::Communication(
                                    err.to_string(),
                                ))));
                            }
                        }
                    }
                } else if this.stream_finished {
                    this.terminated = true;
                    return Poll::Ready(None);
                }
            }

            match unsafe { self.as_mut().map_unchecked_mut(|this| &mut this.inner) }.poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    let this = unsafe { self.as_mut().get_unchecked_mut() };
                    this.buffer.extend_from_slice(&bytes);
                }
                Poll::Ready(Some(Err(err))) => {
                    let this = unsafe { self.as_mut().get_unchecked_mut() };
                    this.terminated = true;
                    return Poll::Ready(Some(Err(err)));
                }
                Poll::Ready(None) => {
                    let this = unsafe { self.as_mut().get_unchecked_mut() };
                    this.stream_finished = true;
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

/// Query data node
#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct Node {
    /// id
    pub statement_id: Option<u64>,
    /// fail message
    pub error: Option<String>,
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
    ($x:expr) => {{ Point::new($x) }};
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

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures::{StreamExt, executor::block_on, stream};

    #[test]
    fn json_object_scanner_tracks_boundary_across_chunks() {
        let mut scanner = JsonObjectScanner::default();
        let mut buffer = br#"{"results":[{"statement_id":0"#.to_vec();

        assert_eq!(scanner.next_document_end(&buffer), None);

        buffer.extend_from_slice(br#","series":null}]}"#);
        assert_eq!(scanner.next_document_end(&buffer), Some(buffer.len()));
    }

    #[test]
    fn json_object_scanner_ignores_braces_inside_strings() {
        let mut scanner = JsonObjectScanner::default();
        let buffer =
            br#"{"results":[{"statement_id":0,"error":"brace } and quote \" stay inside"}]}"#;

        assert_eq!(scanner.next_document_end(buffer), Some(buffer.len()));
    }

    #[test]
    fn chunked_query_parses_back_to_back_documents_without_newline_delimiters() {
        let stream = stream::iter(vec![Ok(Bytes::from_static(
            br#"{"results":[{"statement_id":0,"series":null}]}{"results":[{"statement_id":1,"series":null}]}"#,
        ))]);
        let mut query = ChunkedQuery::new(stream);

        block_on(async {
            let first = query.next().await.unwrap().unwrap();
            let second = query.next().await.unwrap().unwrap();

            assert_eq!(first.results.unwrap()[0].statement_id, Some(0));
            assert_eq!(second.results.unwrap()[0].statement_id, Some(1));
            assert!(query.next().await.is_none());
        });
    }
}
