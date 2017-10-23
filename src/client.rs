use serde_json;
use hyper::client::Client;
use hyper::net::HttpsConnector;
use hyper_native_tls::native_tls::TlsConnector;
use hyper_native_tls::NativeTlsClient;
use hyper::Url;
use std::io::Read;
use std::time::Duration;
use { error, serialization, Precision, Point, Points };

#[derive(Clone)]
pub struct InfluxdbClient {
    ip: String,
    db: String,
    authentication: Option<(String, String)>,
    tls_option: TLSOption,
    read_timeout: Option<u64>,
    write_timeout: Option<u64>,
}

#[derive(Default, Clone)]
struct TLSOption {
    tls: bool,
    connector: Option<TlsConnector>,
}

impl TLSOption {
    fn get_tls(&self) -> bool {
        self.tls
    }

    fn get_connector(&self) -> Option<TlsConnector> {
        self.connector.clone()
    }

    fn set_connector(&mut self, connector: TlsConnector) {
        self.connector = Some(connector);
    }

    fn set_tls(&mut self) {
        self.tls = true;
    }
}

unsafe impl Send for InfluxdbClient {}

impl InfluxdbClient {
    /// Create a new influxdb client
    pub fn new<T>(ip: T, db: T) -> Self
        where T: ToString {
        InfluxdbClient {
            ip: ip.to_string(),
            db: db.to_string(),
            ..Default::default()
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
    pub fn swith_database<T>(&mut self, database: T) where T: ToString {
        self.db = database.to_string();
    }

    /// Change the client's user
    pub fn set_authentication<T>(mut self, user: T, passwd: T) -> Self where T: Into<String> {
        self.authentication = Some((user.into(), passwd.into()));
        self
    }

    pub fn set_tls(mut self, connector: Option<TlsConnector>) -> Self {
        match connector {
            Some(tls_connector) => {
                self.tls_option.set_connector(tls_connector);
            }
            None => {}
        };
        self.tls_option.set_tls();
        self
    }

    /// Query whether the corresponding database exists, return bool
    pub fn ping(&self) -> bool {
        let ping = self.build_client();
        let url = self.build_url("ping", None);
        let res = ping.get(url).send().unwrap();
        match res.status_raw().0 {
            204 => true,
            _ => false,
        }
    }

    /// Query the version of the database and return the version number
    pub fn get_version(&self) -> Option<String> {
        let ping = self.build_client();
        let url = self.build_url("ping", None);
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
    pub fn write_point(&self, point: Point, precision: Option<Precision>, rp: Option<&str>) -> Result<bool, error::Error> {
        let points = Points::new(point);
        self.write_points(points, precision, rp)
    }

    /// Write multiple points to the database
    pub fn write_points(&self, points: Points, precision: Option<Precision>, rp: Option<&str>) -> Result<bool, error::Error> {
        let line = serialization::line_serialization(points);

        let mut param = vec![("db", self.db.as_str())];

        match self.authentication {
            Some(ref t) => {
                param.push(("u", &t.0));
                param.push(("p", &t.1));
            }
            None => {}
        }

        match precision {
            Some(ref t) => param.push(("precision", t.to_str())),
            None => param.push(("precision", "s")),
        };

        match rp {
            Some(t) => param.push(("rp", t)),
            None => (),
        };

        let mut client = self.build_client();

        match self.read_timeout {
            Some(t) => client.set_read_timeout(Some(Duration::new(t, 0))),
            None => ()
        }

        match self.write_timeout {
            Some(t) => client.set_write_timeout(Some(Duration::new(t, 0))),
            None => ()
        }

        let url = self.build_url("write", Some(param));

        let mut res = client.post(url).body(&line).send().unwrap();
        let mut err = String::new();
        let _ = res.read_to_string(&mut err);

        match res.status_raw().0 {
            204 => Ok(true),
            400 => Err(error::Error::SyntaxError(serialization::conversion(err))),
            401 => Err(error::Error::InvalidCredentials("Invalid authentication credentials.".to_string())),
            404 => Err(error::Error::DataBaseDoesNotExist(serialization::conversion(err))),
            500 => Err(error::Error::RetentionPolicyDoesNotExist(err)),
            _ => Err(error::Error::Unknow("There is something wrong".to_string()))
        }
    }

    /// Query and return data, the data type is `Vec<serde_json::Value>`
    pub fn query(&self, q: &str, epoch: Option<Precision>) -> Result<Vec<serde_json::Value>, error::Error> {
        match self.query_raw(q, epoch) {
            Ok(t) => Ok(t.get("results").unwrap().as_array().unwrap().to_vec()),
            Err(e) => Err(e)
        }
    }

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
    fn query_raw(&self, q: &str, epoch: Option<Precision>) -> Result<serde_json::Value, error::Error> {
        let mut param = vec![("db", self.db.as_str()), ("q", q)];

        match self.authentication {
            Some(ref t) => {
                param.push(("u", &t.0));
                param.push(("p", &t.1));
            }
            None => {}
        }

        match epoch {
            Some(ref t) => param.push(("epoch", t.to_str())),
            None => ()
        }

        let mut client = self.build_client();

        match self.read_timeout {
            Some(t) => client.set_read_timeout(Some(Duration::from_secs(t))),
            None => ()
        }

        match self.write_timeout {
            Some(t) => client.set_write_timeout(Some(Duration::from_secs(t))),
            None => ()
        }

        let url = self.build_url("query", Some(param));

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
            400 => Err(error::Error::SyntaxError(serialization::conversion(json_data.get("error").unwrap().to_string()))),
            401 => Err(error::Error::InvalidCredentials("Invalid authentication credentials.".to_string())),
            _ => Err(error::Error::Unknow("There is something wrong".to_string()))
        }
    }

    fn build_url(&self, key: &str, param: Option<Vec<(&str, &str)>>) -> Url {
        let url = match self.tls_option.get_tls() {
            true => {
                let host = String::from("https://") + &self.ip;
                Url::parse(&host).unwrap().join(key).unwrap()
            }
            false => {
                let host = String::from("http://") + &self.ip;
                Url::parse(&host).unwrap().join(key).unwrap()
            }
        };

        if param.is_some() {
            Url::parse_with_params(url.as_str(), param.unwrap()).unwrap()
        } else {
            url
        }
    }

    fn build_client(&self) -> Client {
        match self.tls_option.get_tls() {
            true => {
                match self.tls_option.get_connector() {
                    Some(tls_connector) => {
                        let native_tls_client = NativeTlsClient::from(tls_connector);
                        let connector = HttpsConnector::new(native_tls_client);
                        Client::with_connector(connector)
                    }
                    None => {
                        let ssl = NativeTlsClient::new().unwrap();
                        let connector = HttpsConnector::new(ssl);
                        Client::with_connector(connector)
                    }
                }
            }
            false => {
                Client::new()
            }
        }
    }
}

impl Default for InfluxdbClient {
    /// connecting for default database `test` and host `htt://localhost:8086`
    fn default() -> Self {
        InfluxdbClient::new(String::from("localhost:8086"), String::from("test"))
    }
}
