use bytes::Bytes;
use futures::prelude::*;
use reqwest::{Client as HttpClient, Response, Url};
use serde_json::de::IoRead;
use std::{io::Cursor, iter::FromIterator, net::SocketAddr, net::UdpSocket};

use crate::{error, serialization, ChunkedQuery, Node, Point, Points, Precision, Query};

/// The client to influxdb
#[derive(Debug, Clone)]
pub struct Client {
    host: Url,
    db: String,
    authentication: Option<(String, String)>,
    jwt_token: Option<String>,
    client: HttpClient,
}

impl Client {
    /// Create a new influxdb client with http
    pub fn new<T>(host: Url, db: T) -> Self
    where
        T: Into<String>,
    {
        Client {
            host,
            db: db.into(),
            authentication: None,
            jwt_token: None,
            client: HttpClient::default(),
        }
    }

    /// Create a new influxdb client with custom reqwest's client.
    pub fn new_with_client<T>(host: Url, db: T, client: HttpClient) -> Self
    where
        T: Into<String>,
    {
        Client {
            host,
            db: db.into(),
            authentication: None,
            jwt_token: None,
            client,
        }
    }

    /// Change the client's database
    pub fn switch_database<T>(&mut self, database: T)
    where
        T: Into<String>,
    {
        self.db = database.into();
    }

    /// Change the client's user
    pub fn set_authentication<T>(mut self, user: T, passwd: T) -> Self
    where
        T: Into<String>,
    {
        self.authentication = Some((user.into(), passwd.into()));
        self
    }

    /// Set the client's jwt token
    pub fn set_jwt_token<T>(mut self, token: T) -> Self
    where
        T: Into<String>,
    {
        self.jwt_token = Some(token.into());
        self
    }

    /// View the current db name
    pub fn get_db(&self) -> &str {
        self.db.as_str()
    }

    /// Query whether the corresponding database exists, return bool
    pub fn ping(&self) -> impl Future<Output = bool> {
        let url = self.build_url("ping", None);
        self.client.get(url).send().map(move |res| {
            if let Ok(res) = res {
                matches!(res.status().as_u16(), 204)
            } else {
                false
            }
        })
    }

    /// Query the version of the database and return the version number
    pub fn get_version(&self) -> impl Future<Output = Option<String>> {
        let url = self.build_url("ping", None);
        self.client.get(url).send().map(|res| {
            if let Ok(res) = res {
                match res.status().as_u16() {
                    204 => match res.headers().get("X-Influxdb-Version") {
                        Some(header) => header.to_str().ok().map(str::to_owned),
                        None => Some(String::from("Don't know")),
                    },
                    _ => None,
                }
            } else {
                None
            }
        })
    }

    /// Write a point to the database
    pub fn write_point(
        &self,
        point: Point,
        precision: Option<Precision>,
        rp: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>> {
        let points = Points::new(point);
        self.write_points(points, precision, rp)
    }

    /// Write multiple points to the database
    pub fn write_points<T: Iterator<Item = Point>>(
        &self,
        points: T,
        precision: Option<Precision>,
        rp: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>> {
        let line = serialization::line_serialization(points);

        let mut param = vec![("db", self.db.as_str())];

        match precision {
            Some(ref t) => param.push(("precision", t.to_str())),
            None => param.push(("precision", "s")),
        };

        if let Some(t) = rp {
            param.push(("rp", t))
        }

        let url = self.build_url("write", Some(param));
        let fut = self.client.post(url).body(line).send();

        async move {
            let res = fut.await?;
            let status = res.status().as_u16();
            let err = res.text().await?;

            match status {
                204 => Ok(()),
                400 => Err(error::Error::SyntaxError(serialization::conversion(&err))),
                401 | 403 => Err(error::Error::InvalidCredentials(
                    "Invalid authentication credentials.".to_string(),
                )),
                404 => Err(error::Error::DataBaseDoesNotExist(
                    serialization::conversion(&err),
                )),
                500 => Err(error::Error::RetentionPolicyDoesNotExist(err)),
                status => Err(error::Error::Unknow(format!("Received status code {}", status))),
            }
        }
    }

    /// Query and return data, the data type is `Option<Vec<Node>>`
    pub fn query(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<Output = Result<Option<Vec<Node>>, error::Error>> {
        self.query_raw(q, epoch).map_ok(|t| t.results)
    }

    /// Query and return data, the data type is `Option<Vec<Node>>`
    pub fn query_chunked(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<Output = Result<ChunkedQuery<'static, IoRead<Cursor<Bytes>>>, error::Error>>
    {
        self.query_raw_chunked(q, epoch)
    }

    /// Drop measurement
    pub fn drop_measurement(
        &self,
        measurement: &str,
    ) -> impl Future<Output = Result<(), error::Error>> {
        let sql = format!(
            "Drop measurement {}",
            serialization::quote_ident(measurement)
        );

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Create a new database in InfluxDB.
    pub fn create_database(&self, dbname: &str) -> impl Future<Output = Result<(), error::Error>> {
        let sql = format!("Create database {}", serialization::quote_ident(dbname));

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Drop a database from InfluxDB.
    pub fn drop_database(&self, dbname: &str) -> impl Future<Output = Result<(), error::Error>> {
        let sql = format!("Drop database {}", serialization::quote_ident(dbname));

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Create a new user in InfluxDB.
    pub fn create_user(
        &self,
        user: &str,
        passwd: &str,
        admin: bool,
    ) -> impl Future<Output = Result<(), error::Error>> {
        let sql: String = {
            if admin {
                format!(
                    "Create user {0} with password {1} with all privileges",
                    serialization::quote_ident(user),
                    serialization::quote_literal(passwd)
                )
            } else {
                format!(
                    "Create user {0} WITH password {1}",
                    serialization::quote_ident(user),
                    serialization::quote_literal(passwd)
                )
            }
        };

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Drop a user from InfluxDB.
    pub fn drop_user(&self, user: &str) -> impl Future<Output = Result<(), error::Error>> {
        let sql = format!("Drop user {}", serialization::quote_ident(user));

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Change the password of an existing user.
    pub fn set_user_password(
        &self,
        user: &str,
        passwd: &str,
    ) -> impl Future<Output = Result<(), error::Error>> {
        let sql = format!(
            "Set password for {}={}",
            serialization::quote_ident(user),
            serialization::quote_literal(passwd)
        );

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Grant cluster administration privileges to a user.
    pub fn grant_admin_privileges(
        &self,
        user: &str,
    ) -> impl Future<Output = Result<(), error::Error>> {
        let sql = format!(
            "Grant all privileges to {}",
            serialization::quote_ident(user)
        );

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Revoke cluster administration privileges from a user.
    pub fn revoke_admin_privileges(
        &self,
        user: &str,
    ) -> impl Future<Output = Result<(), error::Error>> {
        let sql = format!(
            "Revoke all privileges from {}",
            serialization::quote_ident(user)
        );

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Grant a privilege on a database to a user.
    /// :param privilege: the privilege to grant, one of 'read', 'write'
    /// or 'all'. The string is case-insensitive
    pub fn grant_privilege(
        &self,
        user: &str,
        db: &str,
        privilege: &str,
    ) -> impl Future<Output = Result<(), error::Error>> {
        let sql = format!(
            "Grant {} on {} to {}",
            privilege,
            serialization::quote_ident(db),
            serialization::quote_ident(user)
        );

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Revoke a privilege on a database from a user.
    /// :param privilege: the privilege to grant, one of 'read', 'write'
    /// or 'all'. The string is case-insensitive
    pub fn revoke_privilege(
        &self,
        user: &str,
        db: &str,
        privilege: &str,
    ) -> impl Future<Output = Result<(), error::Error>> {
        let sql = format!(
            "Revoke {0} on {1} from {2}",
            privilege,
            serialization::quote_ident(db),
            serialization::quote_ident(user)
        );

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Create a retention policy for a database.
    /// :param duration: the duration of the new retention policy.
    ///  Durations such as 1h, 90m, 12h, 7d, and 4w, are all supported
    ///  and mean 1 hour, 90 minutes, 12 hours, 7 day, and 4 weeks,
    ///  respectively. For infinite retention – meaning the data will
    ///  never be deleted – use 'INF' for duration.
    ///  The minimum retention period is 1 hour.
    pub fn create_retention_policy(
        &self,
        name: &str,
        duration: &str,
        replication: &str,
        default: bool,
        db: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>> {
        let database = {
            if let Some(t) = db {
                t
            } else {
                &self.db
            }
        };

        let sql: String = {
            if default {
                format!(
                    "Create retention policy {} on {} duration {} replication {} default",
                    serialization::quote_ident(name),
                    serialization::quote_ident(database),
                    duration,
                    replication
                )
            } else {
                format!(
                    "Create retention policy {} on {} duration {} replication {}",
                    serialization::quote_ident(name),
                    serialization::quote_ident(database),
                    duration,
                    replication
                )
            }
        };

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Drop an existing retention policy for a database.
    pub fn drop_retention_policy(
        &self,
        name: &str,
        db: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>> {
        let database = {
            if let Some(t) = db {
                t
            } else {
                &self.db
            }
        };

        let sql = format!(
            "Drop retention policy {} on {}",
            serialization::quote_ident(name),
            serialization::quote_ident(database)
        );

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    fn send_request(
        &self,
        q: &str,
        epoch: Option<Precision>,
        chunked: bool,
    ) -> impl Future<Output = Result<Response, error::Error>> {
        let mut param = vec![("db", self.db.as_str()), ("q", q)];

        if let Some(ref t) = epoch {
            param.push(("epoch", t.to_str()))
        }

        if chunked {
            param.push(("chunked", "true"));
        }

        let url = self.build_url("query", Some(param));

        let q_lower = q.to_lowercase();
        let mut builder = if q_lower.starts_with("select") && !q_lower.contains("into")
            || q_lower.starts_with("show")
        {
            self.client.get(url)
        } else {
            self.client.post(url)
        };

        if let Some(ref token) = self.jwt_token {
            builder = builder.bearer_auth(token);
        }

        let resp_future = builder.send().boxed();

        async move {
            let res = resp_future.await?;
            match res.status().as_u16() {
                200 => Ok(res),
                400 => {
                    let json_data: Query = res.json().await?;

                    Err(error::Error::SyntaxError(serialization::conversion(
                        &json_data.error.unwrap(),
                    )))
                }
                401 | 403 => Err(error::Error::InvalidCredentials(
                    "Invalid authentication credentials.".to_string(),
                )),
                _ => Err(error::Error::Unknow("There is something wrong".to_string())),
            }
        }
    }

    /// Query and return to the native json structure
    fn query_raw(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<Output = Result<Query, error::Error>> {
        let resp_future = self.send_request(q, epoch, false);
        async move { Ok(resp_future.await?.json().await?) }
    }

    /// Query and return to the native json structure
    fn query_raw_chunked(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<Output = Result<ChunkedQuery<'static, IoRead<Cursor<Bytes>>>, error::Error>>
    {
        let resp_future = self.send_request(q, epoch, true);
        async move {
            let response = resp_future.await?;
            let bytes = Cursor::new(response.bytes().await?);
            let stream = serde_json::Deserializer::from_reader(bytes).into_iter::<Query>();
            Ok(stream)
        }
    }

    /// Constructs the full URL for an API call.
    fn build_url(&self, key: &str, param: Option<Vec<(&str, &str)>>) -> Url {
        let url = self.host.join(key).unwrap();

        let mut authentication = Vec::new();

        if let Some(ref t) = self.authentication {
            authentication.push(("u", &t.0));
            authentication.push(("p", &t.1));
        }

        let url = Url::parse_with_params(url.as_str(), authentication).unwrap();

        if let Some(param) = param {
            Url::parse_with_params(url.as_str(), param).unwrap()
        } else {
            url
        }
    }
}

impl Default for Client {
    /// connecting for default database `test` and host `http://localhost:8086`
    fn default() -> Self {
        Client::new(Url::parse("http://localhost:8086").unwrap(), "test")
    }
}

/// Udp client
pub struct UdpClient {
    hosts: Vec<SocketAddr>,
}

impl UdpClient {
    /// Create a new udp client.
    pub fn new(address: SocketAddr) -> Self {
        UdpClient {
            hosts: vec![address],
        }
    }

    /// add udp host.
    pub fn add_host(&mut self, address: SocketAddr) {
        self.hosts.push(address)
    }

    /// View current hosts
    pub fn get_host(&self) -> &[SocketAddr] {
        self.hosts.as_ref()
    }

    /// Send Points to influxdb.
    pub fn write_points(&self, points: Points) -> Result<(), error::Error> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;

        let line = serialization::line_serialization(points);
        let line = line.as_bytes();
        socket.send_to(&line, self.hosts.as_slice())?;

        Ok(())
    }

    /// Send Point to influxdb.
    pub fn write_point(&self, point: Point) -> Result<(), error::Error> {
        let points = Points { point: vec![point] };
        self.write_points(points)
    }
}

impl FromIterator<SocketAddr> for UdpClient {
    /// Create udp client from iterator.
    fn from_iter<I: IntoIterator<Item = SocketAddr>>(iter: I) -> Self {
        let mut hosts = Vec::new();

        for i in iter {
            hosts.push(i);
        }

        UdpClient { hosts }
    }
}
