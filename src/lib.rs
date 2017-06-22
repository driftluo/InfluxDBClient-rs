//! # InfluxDB Client
//! InfluxDB is an open source time series database with no external dependencies.
//! It's useful for recording metrics, events, and performing analytics.
//!
//! ## Usage
//!
//! ```Rust
//! extern crate influx_db_client;
//!
//! use influx_db_client::{InfluxdbClient, Point, Points, Value, InfluxClient, Precision};
//!
//! fn main() {
//!     let mut client = InfluxdbClient::new("http://localhost:8086", "test", "root", "root");
//!     client.set_write_timeout(10);
//!     client.set_read_timeout(10);
//!
//!     let mut point = Point::new("test");
//!     point.add_field("somefield", Value::Integer(65));
//!
//!     let mut point1 = Point::new("test2");
//!     point1.add_field("somefield", Value::Float(12.2));
//!     point1.add_tag("sometag", Value::Boolean(false));
//!
//!     let mut points = Points::new(point);
//!     points.push(point1);
//!
//!     // if Precision is None, the default is second
//!     // Multiple write
//!     let res = client.write_points(points, Some(Precision::Microseconds), None).unwrap();
//!     let version = client.get_version().unwrap();
//!     println!("{}\nversion:{}", res, version)
//!
//!     // query
//!     let res = client.query("select * from test", None).unwrap();
//!     println!("{:?}", res[0].get("series").unwrap()[0].get("values"))
//! }
//! ```
extern crate hyper;
extern crate serde_json;

use hyper::client::Client;
use hyper::Url;
use std::collections::BTreeMap;
use std::io::Read;
use std::time::Duration;

pub mod serialization;

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
    pub tags: BTreeMap<String, Value>,
    pub fields: BTreeMap<String, Value>,
    pub timestamp: Option<i64>
}

#[derive(Debug)]
pub struct Points {
    pub point: Vec<Point>
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

trait Tostr {
    fn to_str(&self) -> &str;
}

impl Tostr for Precision {
    /// Convert Precision to &str
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

#[derive(Debug)]
pub struct InfluxdbClient<'a> {
    host: &'a str,
    db: &'a str,
    username: &'a str,
    passwd: &'a str,
    read_timeout: Option<u64>,
    write_timeout: Option<u64>,
}

pub trait InfluxClient {
    /// Query whether the corresponding database exists, return bool
    fn ping(&self) -> bool;

    /// Query the version of the database and return the version number
    fn get_version(&self) -> Option<String>;

    /// Write a point to the database
    fn write_point(&self, point: Point, precision: Option<Precision>, rp: Option<&str>) -> Result<bool, String>;

    /// Write multiple points to the database
    ///
    /// ```Rust
    /// let mut client = InfluxdbClient::new("http://localhost:8086", "test", "root", "root");
    /// client.set_write_timeout(10);
    /// client.set_read_timeout(10);
    ///
    /// let mut point = Point::new("test");
    /// point.add_field("somefield", Value::Integer(65));
    ///
    /// let mut point1 = Point::new("test2");
    /// point1.add_field("somefield", Value::Float(12.2));
    /// point1.add_tag("sometag", Value::Boolean(false));
    ///
    /// let mut points = Points::new(point);
    /// points.push(point1);
    ///
    /// if Precision is None, the default is second
    /// Multiple write
    /// let res = client.write_points(points, Some(Precision::Microseconds), None).unwrap();
    /// ```
    fn write_points(&self, points: Points, precision: Option<Precision>, rp: Option<&str>) -> Result<bool, String>;

    /// Query and return data, the data type is `Vec<serde_json::Value>`, such as "show, select"
    ///
    /// ```Rust
    /// let client = InfluxdbClient::new("http://localhost:8086", "test", "root", "root");
    /// let res = client.query("select * from test", None).unwrap();
    /// println!("{:?}", res[0].get("series").unwrap()[0].get("values"));
    /// ```
    fn query(&self, q: &str, epoch: Option<Precision>) -> Result<Vec<serde_json::Value>, String>;

    /// Create a new database in InfluxDB.
    fn create_database(&self, dbname: &str) -> Result<(),String>;

    /// Drop a database from InfluxDB.
    fn drop_database(&self, dbname: &str) -> Result<(),String>;

    /// Create a new user in InfluxDB.
    fn create_user(&self, user: &str, passwd: &str, admin: bool) -> Result<(),String>;

    /// Drop a user from InfluxDB.
    fn drop_user(&self, user: &str) -> Result<(),String>;

    /// Change the password of an existing user.
    fn set_user_password(&self, user: &str, passwd: &str) -> Result<(),String>;

    /// Grant cluster administration privileges to a user.
    fn grant_admin_privileges(&self, user: &str) -> Result<(),String>;

    /// Revoke cluster administration privileges from a user.
    fn revoke_admin_privileges(&self, user: &str) -> Result<(),String>;

    /// Grant a privilege on a database to a user.
    /// :param privilege: the privilege to grant, one of 'read', 'write'
    /// or 'all'. The string is case-insensitive
    fn grant_privilege(&self, user: &str, db: &str, privilege: &str) -> Result<(),String>;

    /// Revoke a privilege on a database from a user.
    /// :param privilege: the privilege to grant, one of 'read', 'write'
    /// or 'all'. The string is case-insensitive
    fn revoke_privilege(&self, user: &str, db: &str, privilege: &str) -> Result<(),String>;

    /// Create a retention policy for a database.
    fn create_retention_policy(&self, name: &str, duration: &str, replication: &str, default: bool, db: Option<&str>) -> Result<(),String>;

    /// Drop an existing retention policy for a database.
    fn drop_retention_policy(&self, name: &str, db: Option<&str>) -> Result<(),String>;
}

trait Query {
    /// Query and return to the native json structure
    fn query_raw(&self, q: &str, epoch: Option<Precision>) -> Result<serde_json::Value, String>;
}

impl<'a> InfluxdbClient<'a> {
    /// Create a new influxdb client
    pub fn new(host: &'a str, db: &'a str, username: &'a str, passwd: &'a str) -> InfluxdbClient<'a> {
        InfluxdbClient {
            host: host,
            db: db,
            username: username,
            passwd: passwd,
            read_timeout: None,
            write_timeout: None,
        }
    }

    /// Set the read timeout value
    pub fn set_read_timeout(&mut self, timeout: u64) {
        self.read_timeout = Some(timeout);
    }

    /// Set the write timeout value
    pub fn set_write_timeout(&mut self, timeout: u64) {
        self.write_timeout = Some(timeout);
    }

    /// Change the client's database
    pub fn swith_database(&mut self, database: &'a str) {
        self.db = database;
    }

    /// Change the client's user
    pub fn swith_user(&mut self, user: &'a str, passwd: &'a str) {
        self.username = user;
        self.passwd = passwd;
    }
}

impl<'a> Query for InfluxdbClient<'a> {
    /// Query and return to the native json structure
    fn query_raw(&self, q: &str, epoch: Option<Precision>) -> Result<serde_json::Value, String> {
        let mut param = vec![("db", self.db), ("u", self.username), ("p", self.passwd), ("q", q)];

        match epoch {
            Some(ref t) => param.push(("epoch", t.to_str())),
            None => ()
        }

        let mut client = Client::new();

        match self.read_timeout {
            Some(t) => client.set_read_timeout(Some(Duration::new(t, 0))),
            None => ()
        }

        match self.write_timeout {
            Some(t) => client.set_write_timeout(Some(Duration::new(t, 0))),
            None => ()
        }

        let url = Url::parse(self.host).unwrap();
        let url = url.join("query").unwrap();
        let url = Url::parse_with_params(url.as_str(), &param).unwrap();

        let q_lower = q.to_lowercase();
        let mut res = {
            if q_lower.starts_with("select") && !q_lower.contains("into") || q_lower.starts_with("show") {
                client.get(url).send().unwrap()
            } else {
                client.post(url).send().unwrap()
            }
        };

        let mut context = String::new();
        let _ = res.read_to_string(&mut context);

        let json_data = serde_json::from_str(&context).unwrap();

        match res.status_raw().0 {
            200 => Ok(json_data),
            400 => Err(json_data.get("error").unwrap().to_string()),
            401 => Err("Invalid authentication credentials.".to_string()),
            _ => Err("There is something wrong".to_string())
        }
    }
}

impl<'a> InfluxClient for InfluxdbClient<'a> {
    /// Query whether the corresponding database exists, return bool
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

    /// Query the version of the database and return the version number
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

    /// Write a point to the database
    fn write_point(&self, point: Point, precision: Option<Precision>, rp: Option<&str>) -> Result<bool, String> {
        let points = Points::new(point);
        self.write_points(points, precision, rp)
    }

    /// Write multiple points to the database
    fn write_points(&self, points: Points, precision: Option<Precision>, rp: Option<&str>) -> Result<bool, String> {
        let line = serialization::line_serialization(points);

        let mut param = vec![("db", self.db), ("u", self.username), ("p", self.passwd)];

        match precision {
            Some(ref t) => param.push(("precision", t.to_str())),
            None => param.push(("precision", "s")),
        };

        match rp {
            Some(t) => param.push(("rp", t)),
            None => (),
        };

        let mut client = Client::new();

        match self.read_timeout {
            Some(t) => client.set_read_timeout(Some(Duration::new(t, 0))),
            None => ()
        }

        match self.write_timeout {
            Some(t) => client.set_write_timeout(Some(Duration::new(t, 0))),
            None => ()
        }

        let url = Url::parse(self.host).unwrap();
        let url = url.join("write").unwrap();
        let url = Url::parse_with_params(url.as_str(), &param).unwrap();

        let mut res = client.post(url).body(&line).send().unwrap();
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

    /// Query and return data, the data type is `Vec<serde_json::Value>`
    fn query(&self, q: &str, epoch: Option<Precision>) -> Result<Vec<serde_json::Value>, String> {
        match self.query_raw(q, epoch) {
            Ok(t) => Ok(t.get("results").unwrap().as_array().unwrap().to_vec()),
            Err(e) => Err(e)
        }
    }

    /// Create a new database in InfluxDB.
    fn create_database(&self, dbname: &str) -> Result<(),String> {
        let sql = format!("Create database {}", serialization::quote_ident(dbname));

        match self.query_raw(&sql, None) {
            Ok(t) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Drop a database from InfluxDB.
    fn drop_database(&self, dbname: &str) -> Result<(),String> {
        let sql = format!("Drop database {}", serialization::quote_ident(dbname));

        match self.query_raw(&sql, None) {
            Ok(t) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Create a new user in InfluxDB.
    fn create_user(&self, user: &str, passwd: &str, admin: bool) -> Result<(),String> {
        let sql: String = {
            if admin {
                format!("Create user {0} with password {1} with all privileges",
                        serialization::quote_ident(user), serialization::quote_literal(passwd))
            } else {
                format!("Create user {0} WITH password {1}", serialization::quote_ident(user),
                        serialization::quote_literal(passwd))
            }
        };

        match self.query_raw(&sql, None) {
            Ok(t) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Drop a user from InfluxDB.
    fn drop_user(&self, user: &str) -> Result<(),String> {
        let sql = format!("Drop user {}", serialization::quote_ident(user));

        match self.query_raw(&sql, None) {
            Ok(t) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Change the password of an existing user.
    fn set_user_password(&self, user: &str, passwd: &str) -> SResult<(),String> {
        let sql = format!("Set password for {}={}", serialization::quote_ident(user),
                          serialization::quote_literal(passwd));

        match self.query_raw(&sql, None) {
            Ok(t) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Grant cluster administration privileges to a user.
    fn grant_admin_privileges(&self, user: &str) -> Result<(),String> {
        let sql = format!("Grant all privileges to {}", serialization::quote_ident(user));

        match self.query_raw(&sql, None) {
            Ok(t) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Revoke cluster administration privileges from a user.
    fn revoke_admin_privileges(&self, user: &str) -> Result<(),String> {
        let sql = format!("Revoke all privileges from {}", serialization::quote_ident(user));

        match self.query_raw(&sql, None) {
            Ok(t) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Grant a privilege on a database to a user.
    /// :param privilege: the privilege to grant, one of 'read', 'write'
    /// or 'all'. The string is case-insensitive
    fn grant_privilege(&self, user: &str, db: &str, privilege: &str) -> Result<(),String> {
        let sql = format!("Grant {} on {} to {}", privilege, serialization::quote_ident(db),
                          serialization::quote_ident(user));

        match self.query_raw(&sql, None) {
            Ok(t) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Revoke a privilege on a database from a user.
    /// :param privilege: the privilege to grant, one of 'read', 'write'
    /// or 'all'. The string is case-insensitive
    fn revoke_privilege(&self, user: &str, db: &str, privilege: &str) -> Result<(),String> {
        let sql = format!("Revoke {0} on {1} from {2}", privilege, serialization::quote_ident(db),
                          serialization::quote_ident(user));

        match self.query_raw(&sql, None) {
            Ok(t) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Create a retention policy for a database.
    /// :param duration: the duration of the new retention policy.
    ///  Durations such as 1h, 90m, 12h, 7d, and 4w, are all supported
    ///  and mean 1 hour, 90 minutes, 12 hours, 7 day, and 4 weeks,
    ///  respectively. For infinite retention – meaning the data will
    ///  never be deleted – use 'INF' for duration.
    ///  The minimum retention period is 1 hour.
    fn create_retention_policy(&self, name: &str, duration: &str, replication: &str, default: bool, db: Option<&str>) -> Result<(),String> {
        let database = {
            if let Some(t) = db {
                t
            } else {
                self.db
            }
        };

        let sql: String = {
            if default {
                format!("Create retention policy {} on {} duration {} replication {} default",
                        serialization::quote_ident(name), serialization::quote_ident(database), duration, replication)
            } else {
                format!("Create retention policy {} on {} duration {} replication {}",
                        serialization::quote_ident(name), serialization::quote_ident(database), duration, replication)
            }
        };

        match self.query_raw(&sql, None) {
            Ok(t) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Drop an existing retention policy for a database.
    fn drop_retention_policy(&self, name: &str, db: Option<&str>) -> Result<(),String> {
        let database = {
            if let Some(t) = db {
                t
            } else {
                self.db
            }
        };

        let sql = format!("Drop retention policy {} on {}", serialization::quote_ident(name),
                          serialization::quote_ident(database));

        match self.query_raw(&sql, None) {
            Ok(t) => Ok(()),
            Err(e) => Err(e)
        }
    }
}

impl Point {
    /// Create a new point
    pub fn new(measurement: &str) -> Point {
        Point {
            measurement: String::from(measurement),
            tags: BTreeMap::new(),
            fields: BTreeMap::new(),
            timestamp: None,
        }
    }

    /// Add a tag and its value
    pub fn add_tag(&mut self, tag: &str, value: Value) {
        self.tags.insert(tag.to_string(), value);
    }

    /// Add a field and its value
    pub fn add_field(&mut self, field: &str, value: Value) {
        self.fields.insert(field.to_string(), value);
    }

    /// Set the specified timestamp
    pub fn add_timestamp(&mut self, timestamp: i64) {
        self.timestamp = Some(timestamp);
    }
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
