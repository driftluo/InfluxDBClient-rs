use serde_json;
use hyper::client::Client as hyper_client;
use hyper::net::HttpsConnector;
use hyper::client::{ RequestBuilder, };
use hyper_native_tls::native_tls::TlsConnector;
use hyper_native_tls::NativeTlsClient;
use hyper::Url;
use std::io::Read;
use std::time::Duration;
use std::net::UdpSocket;
use std::iter::FromIterator;
use std::net::{ ToSocketAddrs, SocketAddr };
use { error, serialization, Precision, Point, Points, Node, Query };

/// The client to influxdb
#[derive(Debug)]
pub struct Client {
    host: String,
    db: String,
    authentication: Option<(String, String)>,
    client: HttpClient
}

unsafe impl Send for Client {}

impl Client {
    /// Create a new influxdb client with http
    pub fn new<T>(host: T, db: T) -> Self
        where T: ToString {
        Client {
            host: host.to_string(),
            db: db.to_string(),
            authentication: None,
            client: HttpClient::default()
        }
    }

    /// Create a new influxdb client with https
    pub fn new_with_option<T: ToString>(host: T, db: T, tls_option: Option<TLSOption>) -> Self {
        Client {
            host: host.to_string(),
            db: db.to_string(),
            authentication: None,
            client: HttpClient::new_with_option(tls_option)
        }
    }

    /// Set the read timeout value, unit "s"
    pub fn set_read_timeout(&mut self, timeout: u64) {
        self.client.set_read_timeout(Duration::from_secs(timeout));
    }

    /// Set the write timeout value, unit "s"
    pub fn set_write_timeout(&mut self, timeout: u64) {
        self.client.set_write_timeout(Duration::from_secs(timeout));
    }

    /// Change the client's database
    pub fn swith_database<T>(&mut self, database: T) where T: ToString {
        self.db = database.to_string();
    }

    /// Change the client's user
    pub fn set_authentication<T>(mut self, user: T, passwd: T) -> Self where T: Into<String> {
        self.authentication = Some((user.into(), passwd.into()));
        self
    }

    /// Change http to https, but don't leave the read write timeout setting
    pub fn set_tls(mut self, connector: Option<TLSOption>) -> Self {
        self.client = HttpClient::new_with_option(connector);
        self
    }

    /// View the current db name
    pub fn get_db(&self) -> String {
        self.db.to_owned()
    }

    /// Query whether the corresponding database exists, return bool
    pub fn ping(&self) -> bool {
        let url = self.build_url("ping", None);
        let res = self.client.get(url).send().unwrap();
        match res.status_raw().0 {
            204 => true,
            _ => false,
        }
    }

    /// Query the version of the database and return the version number
    pub fn get_version(&self) -> Option<String> {
        let url = self.build_url("ping", None);
        let res = self.client.get(url).send().unwrap();
        match res.status_raw().0 {
            204 => match res.headers.get_raw("X-Influxdb-Version") {
                Some(i) => Some(String::from_utf8(i[0].to_vec()).unwrap()),
                None => Some(String::from("Don't know"))
            },
            _ => None,
        }
    }

    /// Write a point to the database
    pub fn write_point(&self, point: Point, precision: Option<Precision>, rp: Option<&str>) -> Result<(), error::Error> {
        let points = Points::new(point);
        self.write_points(points, precision, rp)
    }

    /// Write multiple points to the database
    pub fn write_points(&self, points: Points, precision: Option<Precision>, rp: Option<&str>) -> Result<(), error::Error> {
        let line = serialization::line_serialization(points);

        let mut param = vec![("db", self.db.as_str())];

        match precision {
            Some(ref t) => param.push(("precision", t.to_str())),
            None => param.push(("precision", "s")),
        };

        match rp {
            Some(t) => param.push(("rp", t)),
            None => (),
        };

        let url = self.build_url("write", Some(param));

        let mut res = self.client.post(url).body(&line).send()?;
        let mut err = String::new();
        let _ = res.read_to_string(&mut err);

        match res.status_raw().0 {
            204 => Ok(()),
            400 => Err(error::Error::SyntaxError(serialization::conversion(err))),
            401 | 403 => Err(error::Error::InvalidCredentials("Invalid authentication credentials.".to_string())),
            404 => Err(error::Error::DataBaseDoesNotExist(serialization::conversion(err))),
            500 => Err(error::Error::RetentionPolicyDoesNotExist(err)),
            _ => Err(error::Error::Unknow("There is something wrong".to_string()))
        }
    }

    /// Query and return data, the data type is `Option<Vec<Node>>`
    pub fn query(&self, q: &str, epoch: Option<Precision>) -> Result<Option<Vec<Node>>, error::Error> {
        match self.query_raw(q, epoch) {
            Ok(t) => Ok(t.results),
            Err(e) => Err(e)
        }
    }

    /// Drop measurement
    pub fn drop_measurement(&self, measurement: &str) -> Result<(), error::Error> {
        let sql = format!("Drop measurement {}", serialization::quote_ident(measurement));

        match self.query_raw(&sql, None) {
            Ok(_) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Create a new database in InfluxDB.
    pub fn create_database(&self, dbname: &str) -> Result<(), error::Error> {
        let sql = format!("Create database {}", serialization::quote_ident(dbname));

        match self.query_raw(&sql, None) {
            Ok(_) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Drop a database from InfluxDB.
    pub fn drop_database(&self, dbname: &str) -> Result<(), error::Error> {
        let sql = format!("Drop database {}", serialization::quote_ident(dbname));

        match self.query_raw(&sql, None) {
            Ok(_) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Create a new user in InfluxDB.
    pub fn create_user(&self, user: &str, passwd: &str, admin: bool) -> Result<(), error::Error> {
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
            Ok(_) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Drop a user from InfluxDB.
    pub fn drop_user(&self, user: &str) -> Result<(), error::Error> {
        let sql = format!("Drop user {}", serialization::quote_ident(user));

        match self.query_raw(&sql, None) {
            Ok(_) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Change the password of an existing user.
    pub fn set_user_password(&self, user: &str, passwd: &str) -> Result<(), error::Error> {
        let sql = format!("Set password for {}={}", serialization::quote_ident(user),
                          serialization::quote_literal(passwd));

        match self.query_raw(&sql, None) {
            Ok(_) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Grant cluster administration privileges to a user.
    pub fn grant_admin_privileges(&self, user: &str) -> Result<(), error::Error> {
        let sql = format!("Grant all privileges to {}", serialization::quote_ident(user));

        match self.query_raw(&sql, None) {
            Ok(_) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Revoke cluster administration privileges from a user.
    pub fn revoke_admin_privileges(&self, user: &str) -> Result<(), error::Error> {
        let sql = format!("Revoke all privileges from {}", serialization::quote_ident(user));

        match self.query_raw(&sql, None) {
            Ok(_) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Grant a privilege on a database to a user.
    /// :param privilege: the privilege to grant, one of 'read', 'write'
    /// or 'all'. The string is case-insensitive
    pub fn grant_privilege(&self, user: &str, db: &str, privilege: &str) -> Result<(), error::Error> {
        let sql = format!("Grant {} on {} to {}", privilege, serialization::quote_ident(db),
                          serialization::quote_ident(user));

        match self.query_raw(&sql, None) {
            Ok(_) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Revoke a privilege on a database from a user.
    /// :param privilege: the privilege to grant, one of 'read', 'write'
    /// or 'all'. The string is case-insensitive
    pub fn revoke_privilege(&self, user: &str, db: &str, privilege: &str) -> Result<(), error::Error> {
        let sql = format!("Revoke {0} on {1} from {2}", privilege, serialization::quote_ident(db),
                          serialization::quote_ident(user));

        match self.query_raw(&sql, None) {
            Ok(_) => Ok(()),
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
    pub fn create_retention_policy(&self, name: &str, duration: &str, replication: &str, default: bool, db: Option<&str>) -> Result<(), error::Error> {
        let database = {
            if let Some(t) = db {
                t
            } else {
                &self.db
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
            Ok(_) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Drop an existing retention policy for a database.
    pub fn drop_retention_policy(&self, name: &str, db: Option<&str>) -> Result<(), error::Error> {
        let database = {
            if let Some(t) = db {
                t
            } else {
                &self.db
            }
        };

        let sql = format!("Drop retention policy {} on {}", serialization::quote_ident(name),
                          serialization::quote_ident(database));

        match self.query_raw(&sql, None) {
            Ok(_) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Query and return to the native json structure
    fn query_raw(&self, q: &str, epoch: Option<Precision>) -> Result<Query, error::Error> {
        let mut param = vec![("db", self.db.as_str()), ("q", q)];

        match epoch {
            Some(ref t) => param.push(("epoch", t.to_str())),
            None => ()
        }

        let url = self.build_url("query", Some(param));

        let q_lower = q.to_lowercase();
        let mut res = {
            if q_lower.starts_with("select") && !q_lower.contains("into") || q_lower.starts_with("show") {
                self.client.get(url).send()?
            } else {
                self.client.post(url).send()?
            }
        };

        let mut context = String::new();
        let _ = res.read_to_string(&mut context);

        let json_data: Query = serde_json::from_str(&context).unwrap();

        match res.status_raw().0 {
            200 => Ok(json_data),
            400 => Err(error::Error::SyntaxError(serialization::conversion(json_data.error.unwrap()))),
            401 | 403 => Err(error::Error::InvalidCredentials("Invalid authentication credentials.".to_string())),
            _ => Err(error::Error::Unknow("There is something wrong".to_string()))
        }
    }

    /// Constructs the full URL for an API call.
    fn build_url(&self, key: &str, param: Option<Vec<(&str, &str)>>) -> Url {
        let url = Url::parse(&self.host).unwrap().join(key).unwrap();

        let mut authentication = Vec::new();

        match self.authentication {
            Some(ref t) => {
                authentication.push(("u", &t.0));
                authentication.push(("p", &t.1));
            }
            None => {}
        }

        let url = Url::parse_with_params(url.as_str(), authentication).unwrap();

        if param.is_some() {
            Url::parse_with_params(url.as_str(), param.unwrap()).unwrap()
        } else {
            url
        }
    }
}

impl Default for Client {
    /// connecting for default database `test` and host `http://localhost:8086`
    fn default() -> Self {
        Client::new("http://localhost:8086", "test")
    }
}

/// Option for configuring the behavior of a `Client`.
#[derive(Default, Clone)]
pub struct TLSOption {
    /// A `native_tls::TlsConnector` configured as desired for HTTPS connections.
    pub connector: Option<TlsConnector>,
}

impl TLSOption {
    /// Create a new Tls_option
    pub fn new(connector: TlsConnector) -> Self {
        TLSOption { connector: Some(connector) }
    }

    fn get_connector(self) -> TlsConnector {
        self.connector.unwrap()
    }
}

#[derive(Debug)]
struct HttpClient {
    client: hyper_client
}

impl HttpClient {
    /// Constructs a new `HttpClient`.
    fn new() -> Self {
        HttpClient {
            client: hyper_client::new()
        }
    }

    /// Constructs a new `HttpClient` with option config.
    fn new_with_option(tls_option: Option<TLSOption>) -> Self {
        let connector = match tls_option {
            Some(tls_connector) => {
                let native_tls_client = NativeTlsClient::from(tls_connector.get_connector());
                HttpsConnector::new(native_tls_client)
            }
            None => {
                let ssl = NativeTlsClient::new().unwrap();
                HttpsConnector::new(ssl)
            }
        };

        HttpClient {
            client: hyper_client::with_connector(connector)
        }
    }

    /// Set the read timeout value for all requests.
    fn set_read_timeout(&mut self, timeout: Duration) {
        self.client.set_read_timeout(Some(timeout));
    }

    /// Set the write timeout value for all requests.
    fn set_write_timeout(&mut self, timeout: Duration) {
        self.client.set_write_timeout(Some(timeout));
    }

    /// Make a GET request to influxdb
    fn get(&self, url: Url) -> RequestBuilder {
        self.client.get(url)
    }

    /// Make a POST request to influxdb
    fn post(&self, url: Url) -> RequestBuilder {
        self.client.post(url)
    }
}

impl Default for HttpClient {
    /// Create a default `HttpClient`
    fn default() -> Self {
        HttpClient::new()
    }
}

/// Udp client
pub struct UdpClient {
    hosts: Vec<SocketAddr>,
}

impl UdpClient {
    /// Create a new udp client.
    /// panic when T can't convert to SocketAddr
    pub fn new<T: Into<String>>(address: T) -> Self {
        UdpClient {
            hosts: vec![address.into().to_socket_addrs().unwrap().next().unwrap()]
        }
    }

    /// add udp host.
    /// panic when T can't convert to SocketAddr
    pub fn add_host<T: Into<String>>(&mut self, address: T) {
        self.hosts.push(address.into()
            .to_socket_addrs().unwrap().next().unwrap())
    }

    /// View current hosts
    pub fn get_host(&self) -> Vec<SocketAddr> {
        self.hosts.to_owned()
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
        let points = Points{ point: vec![point] };
        self.write_points(points)
    }
}

impl FromIterator<SocketAddr> for UdpClient {
    /// Create udp client from iterator.
    fn from_iter<I: IntoIterator<Item=SocketAddr>>(iter: I) -> Self {
        let mut hosts = Vec::new();

        for i in iter {
            hosts.push(i);
        }

        UdpClient {
            hosts
        }
    }
}
