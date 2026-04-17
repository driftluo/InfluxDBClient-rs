#[cfg(feature = "reqwest")]
use bytes::Bytes;
use futures::prelude::*;
#[cfg(feature = "reqwest")]
use serde::de::DeserializeOwned;
use std::{
    borrow::Borrow,
    future::Future,
    iter::FromIterator,
    net::UdpSocket,
    net::{SocketAddr, ToSocketAddrs},
};
use url::Url;

#[cfg(feature = "reqwest")]
use reqwest::{Client as ReqwestClient, Response as ReqwestResponse};

use crate::{
    ChunkedQuery, Node, Point, Points, Precision, Query, error,
    http::{
        ChunkedHttpResponse, HttpClient, HttpRequest, HttpResponse, QueryChunkedHttpResponse,
        QueryHttpClient, QueryHttpResponse, WriteHttpClient,
    },
    keys::{ensure_query_success, query_syntax_error},
    serialization,
};

#[cfg(feature = "reqwest")]
impl HttpResponse for ReqwestResponse {
    fn status(&self) -> u16 {
        self.status().as_u16()
    }

    fn header(&self, name: &str) -> Result<Option<&str>, error::Error> {
        match self.headers().get(name) {
            Some(value) => value
                .to_str()
                .map(Some)
                .map_err(|err| error::Error::Communication(err.to_string())),
            None => Ok(None),
        }
    }

    async fn text(self) -> Result<String, error::Error> {
        Ok(reqwest::Response::text(self).await?)
    }

    async fn bytes(self) -> Result<Bytes, error::Error> {
        Ok(reqwest::Response::bytes(self).await?)
    }

    async fn json<T>(self) -> Result<T, error::Error>
    where
        T: DeserializeOwned,
    {
        Ok(reqwest::Response::json(self).await?)
    }
}

#[cfg(feature = "reqwest")]
impl QueryHttpResponse for ReqwestResponse {
    async fn json_send<T>(self) -> Result<T, error::Error>
    where
        T: DeserializeOwned + 'static,
    {
        Ok(reqwest::Response::json(self).await?)
    }
}

#[cfg(feature = "reqwest")]
impl ChunkedHttpResponse for ReqwestResponse {
    type Stream =
        std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, error::Error>> + Send + 'static>>;

    async fn into_chunk_stream(self) -> Result<Self::Stream, error::Error> {
        Ok(Box::pin(
            reqwest::Response::bytes_stream(self).map_err(Into::into),
        ))
    }
}

#[cfg(feature = "reqwest")]
impl QueryChunkedHttpResponse for ReqwestResponse {
    async fn into_chunk_stream_send(self) -> Result<Self::Stream, error::Error> {
        Ok(Box::pin(
            reqwest::Response::bytes_stream(self).map_err(Into::into),
        ))
    }
}

#[cfg(feature = "reqwest")]
impl WriteHttpClient for ReqwestClient {
    fn post_send(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<(u16, String), error::Error>> + Send + 'static + use<> {
        let (url, body, bearer_token) = request.into_parts();
        let builder = self.post(url);
        let builder = if let Some(token) = bearer_token {
            builder.bearer_auth(token)
        } else {
            builder
        };
        let builder = if let Some(body) = body {
            builder.body(body)
        } else {
            builder
        };

        async move {
            let res = builder.send().await?;
            let status = res.status().as_u16();
            let body = reqwest::Response::text(res).await?;
            Ok((status, body))
        }
    }
}

#[cfg(feature = "reqwest")]
impl HttpClient for ReqwestClient {
    type Response = ReqwestResponse;

    fn get(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<Self::Response, error::Error>> {
        let (url, _body, bearer_token) = request.into_parts();
        let builder = if let Some(token) = bearer_token {
            self.get(url).bearer_auth(token)
        } else {
            self.get(url)
        };

        async move { Ok(builder.send().await?) }
    }

    fn post(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'_> {
        let (url, body, bearer_token) = request.into_parts();
        let builder = self.post(url);
        let builder = if let Some(token) = bearer_token {
            builder.bearer_auth(token)
        } else {
            builder
        };
        let builder = if let Some(body) = body {
            builder.body(body)
        } else {
            builder
        };

        async move { Ok(builder.send().await?) }
    }
}

#[cfg(feature = "reqwest")]
impl QueryHttpClient for ReqwestClient {
    fn send_get(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static {
        let client = self.clone();
        let (url, _body, bearer_token) = request.into_parts();

        async move {
            let builder = if let Some(token) = bearer_token {
                client.get(url).bearer_auth(token)
            } else {
                client.get(url)
            };

            Ok(builder.send().await?)
        }
    }

    fn send_post(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static {
        let client = self.clone();
        let (url, body, bearer_token) = request.into_parts();

        async move {
            let builder = client.post(url);
            let builder = if let Some(token) = bearer_token {
                builder.bearer_auth(token)
            } else {
                builder
            };
            let builder = if let Some(body) = body {
                builder.body(body)
            } else {
                builder
            };

            Ok(builder.send().await?)
        }
    }
}

/// The client to influxdb
#[cfg(feature = "reqwest")]
#[derive(Debug, Clone)]
pub struct Client<T = ReqwestClient> {
    host: Url,
    db: String,
    authentication: Option<(String, String)>,
    jwt_token: Option<String>,
    client: T,
}

/// The client to influxdb
#[cfg(not(feature = "reqwest"))]
#[derive(Debug, Clone)]
pub struct Client<T> {
    host: Url,
    db: String,
    authentication: Option<(String, String)>,
    jwt_token: Option<String>,
    client: T,
}

#[cfg(feature = "reqwest")]
impl Client<ReqwestClient> {
    /// Create a new influxdb client with the default reqwest client.
    pub fn new<T>(host: Url, db: T) -> Self
    where
        T: Into<String>,
    {
        Client {
            host,
            db: db.into(),
            authentication: None,
            jwt_token: None,
            client: ReqwestClient::default(),
        }
    }
}

impl<T> Client<T> {
    /// Create a new influxdb client with a custom HTTP client.
    pub fn new_with_client<U>(host: Url, db: U, client: T) -> Self
    where
        U: Into<String>,
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
    pub fn switch_database<U>(&mut self, database: U)
    where
        U: Into<String>,
    {
        self.db = database.into();
    }

    /// Change the client's user
    pub fn set_authentication<U>(mut self, user: U, passwd: U) -> Self
    where
        U: Into<String>,
    {
        self.authentication = Some((user.into(), passwd.into()));
        self
    }

    /// Set the client's jwt token
    pub fn set_jwt_token<U>(mut self, token: U) -> Self
    where
        U: Into<String>,
    {
        self.jwt_token = Some(token.into());
        self
    }

    /// View the current db name
    pub fn get_db(&self) -> &str {
        self.db.as_str()
    }

    fn build_request(&self, url: Url) -> HttpRequest {
        match self.jwt_token.as_deref() {
            Some(token) => HttpRequest::new(url).with_bearer_token(token),
            None => HttpRequest::new(url),
        }
    }

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

fn validate_privilege(privilege: &str) -> Result<(), error::Error> {
    if privilege.eq_ignore_ascii_case("read")
        || privilege.eq_ignore_ascii_case("write")
        || privilege.eq_ignore_ascii_case("all")
    {
        Ok(())
    } else {
        Err(error::Error::SyntaxError(format!(
            "Invalid privilege '{}': must be one of 'read', 'write', or 'all'",
            privilege
        )))
    }
}

async fn check_query_status<R, F, Fut>(res: R, parse_json: F) -> Result<R, error::Error>
where
    R: HttpResponse,
    F: FnOnce(R) -> Fut,
    Fut: Future<Output = Result<Query, error::Error>>,
{
    match res.status() {
        200 => Ok(res),
        400 => {
            let json_data = parse_json(res).await?;
            Err(query_syntax_error(&json_data, "Bad request"))
        }
        401 | 403 => Err(error::Error::InvalidCredentials(
            "Invalid authentication credentials.".to_string(),
        )),
        _ => Err(error::Error::Unknow("There is something wrong".to_string())),
    }
}

impl<T> Client<T>
where
    T: HttpClient,
{
    /// Query whether the corresponding database exists, return bool
    pub fn ping(&self) -> impl Future<Output = bool> {
        let url = self.build_url("ping", None);
        let request_future = self.client.get(self.build_request(url));

        async move {
            request_future
                .await
                .map(|res| matches!(res.status(), 204))
                .unwrap_or(false)
        }
    }

    /// Query the version of the database and return the version number
    pub fn get_version(&self) -> impl Future<Output = Option<String>> {
        let url = self.build_url("ping", None);
        let request_future = self.client.get(self.build_request(url));

        async move {
            if let Ok(res) = request_future.await {
                match res.status() {
                    204 => match res.header("X-Influxdb-Version") {
                        Ok(Some(header)) => Some(header.to_owned()),
                        Ok(None) => Some(String::from("Don't know")),
                        Err(_) => None,
                    },
                    _ => None,
                }
            } else {
                None
            }
        }
    }

    /// Write a point to the database
    pub fn write_point<'a>(
        &self,
        point: Point<'a>,
        precision: Option<Precision>,
        rp: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static
    where
        T: WriteHttpClient,
    {
        let line = serialization::line_serialization(std::iter::once(&point));
        self.write_line(line, precision, rp)
    }

    /// Write multiple points to the database
    pub fn write_points<'a, I: IntoIterator<Item = impl Borrow<Point<'a>>>>(
        &self,
        points: I,
        precision: Option<Precision>,
        rp: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static
    where
        T: WriteHttpClient,
    {
        let line = serialization::line_serialization(points);
        self.write_line(line, precision, rp)
    }

    fn write_line(
        &self,
        line: String,
        precision: Option<Precision>,
        rp: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static
    where
        T: WriteHttpClient,
    {
        let mut param = vec![("db", self.db.as_str())];

        match precision {
            Some(ref t) => param.push(("precision", t.to_str())),
            None => param.push(("precision", "s")),
        };

        if let Some(t) = rp {
            param.push(("rp", t))
        }

        let url = self.build_url("write", Some(param));
        let request = self.build_request(url).with_body(line);
        let post_future = self.client.post_send(request);

        async move {
            let (status, body) = post_future.await?;

            if status == 204 {
                return Ok(());
            }

            match status {
                400 => Err(error::Error::SyntaxError(serialization::conversion(&body))),
                401 | 403 => Err(error::Error::InvalidCredentials(
                    "Invalid authentication credentials.".to_string(),
                )),
                404 => Err(error::Error::DataBaseDoesNotExist(
                    serialization::conversion(&body),
                )),
                500 => Err(error::Error::RetentionPolicyDoesNotExist(body)),
                status => Err(error::Error::Unknow(format!(
                    "Received status code {}",
                    status
                ))),
            }
        }
    }

    /// Query and return data, the data type is `Option<Vec<Node>>`
    pub fn query(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<Output = Result<Option<Vec<Node>>, error::Error>> + Send + 'static
    where
        T: QueryHttpClient,
    {
        let raw_future = self.query_raw_owned(q.to_owned(), epoch);
        async move { Ok(raw_future.await?.results) }
    }

    /// Query and return data through a future that borrows the configured HTTP client.
    pub fn query_borrow(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<Output = Result<Option<Vec<Node>>, error::Error>> + use<'_, T> {
        self.query_raw_borrow(q, epoch).map_ok(|t| t.results)
    }

    /// Query and return chunked query documents.
    pub fn query_chunked(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<
        Output = Result<ChunkedQuery<<T::Response as ChunkedHttpResponse>::Stream>, error::Error>,
    > + Send
    + 'static
    where
        T: QueryHttpClient,
        T::Response: QueryChunkedHttpResponse,
    {
        self.query_raw_chunked_owned(q.to_owned(), epoch)
    }

    /// Query and return chunked query documents through a future that borrows the configured HTTP client.
    pub fn query_chunked_borrow(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<
        Output = Result<ChunkedQuery<<T::Response as ChunkedHttpResponse>::Stream>, error::Error>,
    > + use<'_, T>
    where
        T::Response: ChunkedHttpResponse,
    {
        self.query_raw_chunked_borrow(q, epoch)
    }

    /// Drop measurement
    pub fn drop_measurement(
        &self,
        measurement: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static
    where
        T: QueryHttpClient,
    {
        let sql = format!(
            "Drop measurement {}",
            serialization::quote_ident(measurement)
        );

        self.query_raw_owned(sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::drop_measurement`].
    pub fn drop_measurement_borrow(
        &self,
        measurement: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T> {
        let sql = format!(
            "Drop measurement {}",
            serialization::quote_ident(measurement)
        );

        self.execute_query_borrow(sql)
    }

    /// Create a new database in InfluxDB.
    pub fn create_database(
        &self,
        dbname: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static
    where
        T: QueryHttpClient,
    {
        let sql = format!("Create database {}", serialization::quote_ident(dbname));

        self.query_raw_owned(sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::create_database`].
    pub fn create_database_borrow(
        &self,
        dbname: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T> {
        let sql = format!("Create database {}", serialization::quote_ident(dbname));

        self.execute_query_borrow(sql)
    }

    /// Drop a database from InfluxDB.
    pub fn drop_database(
        &self,
        dbname: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static
    where
        T: QueryHttpClient,
    {
        let sql = format!("Drop database {}", serialization::quote_ident(dbname));

        self.query_raw_owned(sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::drop_database`].
    pub fn drop_database_borrow(
        &self,
        dbname: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T> {
        let sql = format!("Drop database {}", serialization::quote_ident(dbname));

        self.execute_query_borrow(sql)
    }

    /// Create a new user in InfluxDB.
    pub fn create_user(
        &self,
        user: &str,
        passwd: &str,
        admin: bool,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static
    where
        T: QueryHttpClient,
    {
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

        self.query_raw_owned(sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::create_user`].
    pub fn create_user_borrow(
        &self,
        user: &str,
        passwd: &str,
        admin: bool,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T> {
        let sql: String = if admin {
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
        };

        self.execute_query_borrow(sql)
    }

    /// Drop a user from InfluxDB.
    pub fn drop_user(
        &self,
        user: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static
    where
        T: QueryHttpClient,
    {
        let sql = format!("Drop user {}", serialization::quote_ident(user));

        self.query_raw_owned(sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::drop_user`].
    pub fn drop_user_borrow(
        &self,
        user: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T> {
        let sql = format!("Drop user {}", serialization::quote_ident(user));

        self.execute_query_borrow(sql)
    }

    /// Change the password of an existing user.
    pub fn set_user_password(
        &self,
        user: &str,
        passwd: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static
    where
        T: QueryHttpClient,
    {
        let sql = format!(
            "Set password for {}={}",
            serialization::quote_ident(user),
            serialization::quote_literal(passwd)
        );

        self.query_raw_owned(sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::set_user_password`].
    pub fn set_user_password_borrow(
        &self,
        user: &str,
        passwd: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T> {
        let sql = format!(
            "Set password for {}={}",
            serialization::quote_ident(user),
            serialization::quote_literal(passwd)
        );

        self.execute_query_borrow(sql)
    }

    /// Grant cluster administration privileges to a user.
    pub fn grant_admin_privileges(
        &self,
        user: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static
    where
        T: QueryHttpClient,
    {
        let sql = format!(
            "Grant all privileges to {}",
            serialization::quote_ident(user)
        );

        self.query_raw_owned(sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::grant_admin_privileges`].
    pub fn grant_admin_privileges_borrow(
        &self,
        user: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T> {
        let sql = format!(
            "Grant all privileges to {}",
            serialization::quote_ident(user)
        );

        self.execute_query_borrow(sql)
    }

    /// Revoke cluster administration privileges from a user.
    pub fn revoke_admin_privileges(
        &self,
        user: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static
    where
        T: QueryHttpClient,
    {
        let sql = format!(
            "Revoke all privileges from {}",
            serialization::quote_ident(user)
        );

        self.query_raw_owned(sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::revoke_admin_privileges`].
    pub fn revoke_admin_privileges_borrow(
        &self,
        user: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T> {
        let sql = format!(
            "Revoke all privileges from {}",
            serialization::quote_ident(user)
        );

        self.execute_query_borrow(sql)
    }

    /// Grant a privilege on a database to a user.
    /// :param privilege: the privilege to grant, one of 'read', 'write'
    /// or 'all'. The string is case-insensitive
    pub fn grant_privilege(
        &self,
        user: &str,
        db: &str,
        privilege: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static
    where
        T: QueryHttpClient,
    {
        match validate_privilege(privilege) {
            Err(e) => futures::future::Either::Left(futures::future::ready(Err(e))),
            Ok(()) => {
                let sql = format!(
                    "Grant {} on {} to {}",
                    privilege,
                    serialization::quote_ident(db),
                    serialization::quote_ident(user)
                );
                futures::future::Either::Right(self.query_raw_owned(sql, None).map_ok(|_| ()))
            }
        }
    }

    /// Borrowing variant of [`Client::grant_privilege`].
    pub fn grant_privilege_borrow(
        &self,
        user: &str,
        db: &str,
        privilege: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T> {
        let validated = validate_privilege(privilege);
        let sql = format!(
            "Grant {} on {} to {}",
            privilege,
            serialization::quote_ident(db),
            serialization::quote_ident(user)
        );

        async move {
            validated?;
            self.execute_query_borrow(sql).await
        }
    }

    /// Revoke a privilege on a database from a user.
    /// :param privilege: the privilege to grant, one of 'read', 'write'
    /// or 'all'. The string is case-insensitive
    pub fn revoke_privilege(
        &self,
        user: &str,
        db: &str,
        privilege: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static
    where
        T: QueryHttpClient,
    {
        match validate_privilege(privilege) {
            Err(e) => futures::future::Either::Left(futures::future::ready(Err(e))),
            Ok(()) => {
                let sql = format!(
                    "Revoke {0} on {1} from {2}",
                    privilege,
                    serialization::quote_ident(db),
                    serialization::quote_ident(user)
                );
                futures::future::Either::Right(self.query_raw_owned(sql, None).map_ok(|_| ()))
            }
        }
    }

    /// Borrowing variant of [`Client::revoke_privilege`].
    pub fn revoke_privilege_borrow(
        &self,
        user: &str,
        db: &str,
        privilege: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T> {
        let validated = validate_privilege(privilege);
        let sql = format!(
            "Revoke {0} on {1} from {2}",
            privilege,
            serialization::quote_ident(db),
            serialization::quote_ident(user)
        );

        async move {
            validated?;
            self.execute_query_borrow(sql).await
        }
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
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static
    where
        T: QueryHttpClient,
    {
        let database = { if let Some(t) = db { t } else { &self.db } };

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

        self.query_raw_owned(sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::create_retention_policy`].
    pub fn create_retention_policy_borrow(
        &self,
        name: &str,
        duration: &str,
        replication: &str,
        default: bool,
        db: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T> {
        let database = if let Some(t) = db { t } else { &self.db };

        let sql: String = if default {
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
        };

        self.execute_query_borrow(sql)
    }

    /// Drop an existing retention policy for a database.
    pub fn drop_retention_policy(
        &self,
        name: &str,
        db: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static
    where
        T: QueryHttpClient,
    {
        let database = { if let Some(t) = db { t } else { &self.db } };

        let sql = format!(
            "Drop retention policy {} on {}",
            serialization::quote_ident(name),
            serialization::quote_ident(database)
        );

        self.query_raw_owned(sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::drop_retention_policy`].
    pub fn drop_retention_policy_borrow(
        &self,
        name: &str,
        db: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T> {
        let database = if let Some(t) = db { t } else { &self.db };

        let sql = format!(
            "Drop retention policy {} on {}",
            serialization::quote_ident(name),
            serialization::quote_ident(database)
        );

        self.execute_query_borrow(sql)
    }

    fn build_query_request(
        &self,
        q: &str,
        epoch: Option<Precision>,
        chunked: bool,
    ) -> (HttpRequest, bool) {
        let mut param = vec![("db", self.db.as_str()), ("q", q)];

        if let Some(ref t) = epoch {
            param.push(("epoch", t.to_str()))
        }

        if chunked {
            param.push(("chunked", "true"));
        }

        let url = self.build_url("query", Some(param));
        let request = self.build_request(url);

        let q_lower = q.to_ascii_lowercase();
        let is_read_query = q_lower.starts_with("select") && !q_lower.contains("into")
            || q_lower.starts_with("show");

        (request, is_read_query)
    }

    fn send_request_borrow(
        &self,
        q: &str,
        epoch: Option<Precision>,
        chunked: bool,
    ) -> impl Future<Output = Result<T::Response, error::Error>> + use<'_, T> {
        let (request, is_read_query) = self.build_query_request(q, epoch, chunked);
        let request_future = if is_read_query {
            futures::future::Either::Left(self.client.get(request))
        } else {
            futures::future::Either::Right(self.client.post(request))
        };

        async move {
            let res = request_future.await?;
            check_query_status(res, |r| r.json::<Query>()).await
        }
    }

    async fn send_request_owned(
        client: T,
        request: HttpRequest,
        is_read_query: bool,
    ) -> Result<T::Response, error::Error>
    where
        T: QueryHttpClient,
    {
        let res = if is_read_query {
            client.send_get(request).await?
        } else {
            client.send_post(request).await?
        };

        check_query_status(res, |r| r.json_send::<Query>()).await
    }

    /// Query and return to the native json structure through the owned query path.
    fn query_raw_owned(
        &self,
        q: String,
        epoch: Option<Precision>,
    ) -> impl Future<Output = Result<Query, error::Error>> + Send + 'static
    where
        T: QueryHttpClient,
    {
        let (request, is_read_query) = self.build_query_request(&q, epoch, false);
        let client = self.client.clone();
        let resp_future = Self::send_request_owned(client, request, is_read_query);
        async move {
            let query = resp_future.await?.json_send().await?;
            ensure_query_success(query)
        }
    }

    /// Query and return to the native json structure through a future that borrows the HTTP client.
    fn query_raw_borrow(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<Output = Result<Query, error::Error>> + use<'_, T> {
        let resp_future = self.send_request_borrow(q, epoch, false);
        async move {
            let query = resp_future.await?.json().await?;
            ensure_query_success(query)
        }
    }

    async fn execute_query_borrow(&self, sql: String) -> Result<(), error::Error> {
        self.query_raw_borrow(&sql, None).await?;
        Ok(())
    }

    /// Query and return chunked query documents through the owned query path.
    fn query_raw_chunked_owned(
        &self,
        q: String,
        epoch: Option<Precision>,
    ) -> impl Future<
        Output = Result<ChunkedQuery<<T::Response as ChunkedHttpResponse>::Stream>, error::Error>,
    > + Send
    + 'static
    where
        T: QueryHttpClient,
        T::Response: QueryChunkedHttpResponse,
    {
        let (request, is_read_query) = self.build_query_request(&q, epoch, true);
        let client = self.client.clone();
        let resp_future = Self::send_request_owned(client, request, is_read_query);
        async move {
            let response = resp_future.await?;
            let stream = response.into_chunk_stream_send().await?;
            Ok(ChunkedQuery::new(stream))
        }
    }

    /// Query and return chunked query documents through a future that borrows the HTTP client.
    fn query_raw_chunked_borrow(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<
        Output = Result<ChunkedQuery<<T::Response as ChunkedHttpResponse>::Stream>, error::Error>,
    > + use<'_, T>
    where
        T::Response: ChunkedHttpResponse,
    {
        let resp_future = self.send_request_borrow(q, epoch, true);
        async move {
            let response = resp_future.await?;
            let stream = response.into_chunk_stream().await?;
            Ok(ChunkedQuery::new(stream))
        }
    }
}

#[cfg(feature = "reqwest")]
impl Default for Client<ReqwestClient> {
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

    /// Crates a new UDP client from anything that `ToSocketAddrs` can handle: e.g. a DNS name.
    pub fn with_host<TSA: ToSocketAddrs>(tsa: TSA) -> Result<Self, error::Error> {
        let result = Self {
            hosts: tsa.to_socket_addrs()?.collect(),
        };
        Ok(result)
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
        socket.send_to(line, self.hosts.as_slice())?;

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

#[cfg(test)]
mod tests {
    use super::Client;
    use crate::{
        Point, Precision, Query, error,
        http::{
            ChunkedHttpResponse, HttpClient, HttpRequest, HttpResponse, QueryChunkedHttpResponse,
            QueryHttpClient, QueryHttpResponse, WriteHttpClient,
        },
    };
    use bytes::Bytes;
    use futures::StreamExt;
    use serde::{Deserialize, de::DeserializeOwned};
    use std::cell::Cell;
    use std::collections::HashMap;
    #[cfg(feature = "reqwest")]
    use std::io::{Read, Write};
    use std::marker::PhantomData;
    #[cfg(feature = "reqwest")]
    use std::net::TcpListener;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};
    #[cfg(feature = "reqwest")]
    use std::thread;
    use url::Url;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RecordedRequest {
        method: &'static str,
        url: String,
        bearer_token: Option<String>,
        body: Option<String>,
    }

    #[derive(Debug, Clone)]
    struct FakeResponse {
        status: u16,
        headers: HashMap<String, String>,
        body: String,
    }

    impl FakeResponse {
        fn json(status: u16, body: &str) -> Self {
            Self {
                status,
                headers: HashMap::new(),
                body: body.to_string(),
            }
        }

        fn empty(status: u16) -> Self {
            Self::json(status, "")
        }
    }

    impl HttpResponse for FakeResponse {
        fn status(&self) -> u16 {
            self.status
        }

        fn header(&self, name: &str) -> Result<Option<&str>, error::Error> {
            Ok(self.headers.get(name).map(String::as_str))
        }

        async fn text(self) -> Result<String, error::Error> {
            Ok(self.body)
        }

        async fn bytes(self) -> Result<Bytes, error::Error> {
            Ok(Bytes::from(self.body))
        }

        async fn json<T>(self) -> Result<T, error::Error>
        where
            T: DeserializeOwned,
        {
            serde_json::from_str(&self.body)
                .map_err(|err| error::Error::Communication(err.to_string()))
        }
    }

    impl QueryHttpResponse for FakeResponse {
        async fn json_send<T>(self) -> Result<T, error::Error>
        where
            T: DeserializeOwned + 'static,
        {
            serde_json::from_str(&self.body)
                .map_err(|err| error::Error::Communication(err.to_string()))
        }
    }

    #[derive(Clone)]
    struct RecordingHttpClient {
        requests: Arc<Mutex<Vec<RecordedRequest>>>,
        responses: Arc<Mutex<Vec<FakeResponse>>>,
    }

    impl RecordingHttpClient {
        fn new(responses: Vec<FakeResponse>) -> Self {
            Self {
                requests: Arc::new(Mutex::new(Vec::new())),
                responses: Arc::new(Mutex::new(responses.into_iter().rev().collect())),
            }
        }

        fn take_requests(&self) -> Vec<RecordedRequest> {
            self.requests.lock().unwrap().clone()
        }

        fn record_with(
            requests: Arc<Mutex<Vec<RecordedRequest>>>,
            responses: Arc<Mutex<Vec<FakeResponse>>>,
            method: &'static str,
            request: HttpRequest,
        ) -> Result<FakeResponse, error::Error> {
            requests.lock().unwrap().push(RecordedRequest {
                method,
                url: request.url().to_string(),
                bearer_token: request.bearer_token().map(str::to_owned),
                body: request.body().map(str::to_owned),
            });

            responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| error::Error::Unknow("missing fake response".to_string()))
        }
    }

    struct NonSyncWriteHttpClient {
        requests: Arc<Mutex<Vec<RecordedRequest>>>,
        status: Cell<u16>,
    }

    impl NonSyncWriteHttpClient {
        fn new(status: u16) -> Self {
            Self {
                requests: Arc::new(Mutex::new(Vec::new())),
                status: Cell::new(status),
            }
        }

        fn take_requests(&self) -> Vec<RecordedRequest> {
            self.requests.lock().unwrap().clone()
        }
    }

    struct BorrowingGetHttpClient {
        select_response: String,
        get_calls: Cell<u16>,
    }

    impl BorrowingGetHttpClient {
        fn new(select_response: &str) -> Self {
            Self {
                select_response: select_response.to_string(),
                get_calls: Cell::new(0),
            }
        }
    }

    struct BorrowingPostHttpClient {
        post_calls: Cell<u16>,
    }

    impl BorrowingPostHttpClient {
        fn new() -> Self {
            Self {
                post_calls: Cell::new(0),
            }
        }
    }

    struct BorrowingQueryGetHttpClient {
        get_calls: Cell<u16>,
    }

    impl BorrowingQueryGetHttpClient {
        fn new() -> Self {
            Self {
                get_calls: Cell::new(0),
            }
        }
    }

    #[derive(Clone)]
    struct SplitQueryHttpClient {
        response_body: String,
        get_calls: Cell<u16>,
        post_calls: Cell<u16>,
    }

    impl SplitQueryHttpClient {
        fn new(response_body: &str) -> Self {
            Self {
                response_body: response_body.to_string(),
                get_calls: Cell::new(0),
                post_calls: Cell::new(0),
            }
        }
    }

    struct NonSendResponseHttpClient {
        response_body: String,
    }

    impl NonSendResponseHttpClient {
        fn new(response_body: &str) -> Self {
            Self {
                response_body: response_body.to_string(),
            }
        }
    }

    struct NonSendBodyFutureHttpClient {
        response_body: String,
    }

    impl NonSendBodyFutureHttpClient {
        fn new(response_body: &str) -> Self {
            Self {
                response_body: response_body.to_string(),
            }
        }
    }

    struct NonSendResponse {
        status: u16,
        body: Rc<String>,
    }

    struct NonSendBodyFutureResponse {
        status: u16,
        body: Rc<String>,
    }

    enum VersionHeader {
        Missing,
        Valid(String),
        InvalidUtf8,
    }

    struct VersionHeaderResponse {
        status: u16,
        version_header: VersionHeader,
    }

    struct VersionHeaderHttpClient {
        response: VersionHeaderResponse,
    }

    impl VersionHeaderHttpClient {
        fn new(response: VersionHeaderResponse) -> Self {
            Self { response }
        }
    }

    #[derive(Debug, PartialEq)]
    struct NonSendQueryResult {
        value: String,
        _not_send: PhantomData<Rc<()>>,
    }

    #[derive(Clone)]
    struct ChunkedFakeResponse {
        status: u16,
        segments: Vec<Vec<u8>>,
    }

    #[derive(Clone)]
    struct ChunkedHttpClient {
        response: ChunkedFakeResponse,
    }

    struct BorrowingChunkedHttpClient {
        response: ChunkedFakeResponse,
    }

    impl<'de> Deserialize<'de> for NonSendQueryResult {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            #[derive(Deserialize)]
            struct Helper {
                value: String,
            }

            let helper = Helper::deserialize(deserializer)?;
            Ok(Self {
                value: helper.value,
                _not_send: PhantomData,
            })
        }
    }

    impl HttpClient for RecordingHttpClient {
        type Response = FakeResponse;

        fn get(
            &self,
            request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> {
            let requests = Arc::clone(&self.requests);
            let responses = Arc::clone(&self.responses);

            async move { Self::record_with(requests, responses, "GET", request) }
        }

        fn post(
            &self,
            request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> {
            let requests = Arc::clone(&self.requests);
            let responses = Arc::clone(&self.responses);

            async move { Self::record_with(requests, responses, "POST", request) }
        }
    }

    impl WriteHttpClient for RecordingHttpClient {
        fn post_send(
            &self,
            request: HttpRequest,
        ) -> impl Future<Output = Result<(u16, String), error::Error>> + Send + 'static + use<>
        {
            let requests = Arc::clone(&self.requests);
            let responses = Arc::clone(&self.responses);

            async move {
                let res = Self::record_with(requests, responses, "POST", request)?;
                Ok((res.status, res.body))
            }
        }
    }

    impl QueryHttpClient for RecordingHttpClient {
        fn send_get(
            &self,
            request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static {
            let requests = Arc::clone(&self.requests);
            let responses = Arc::clone(&self.responses);

            async move { Self::record_with(requests, responses, "GET", request) }
        }

        fn send_post(
            &self,
            request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static {
            let requests = Arc::clone(&self.requests);
            let responses = Arc::clone(&self.responses);

            async move { Self::record_with(requests, responses, "POST", request) }
        }
    }

    impl HttpClient for NonSyncWriteHttpClient {
        type Response = FakeResponse;

        async fn get(&self, _request: HttpRequest) -> Result<Self::Response, error::Error> {
            Err(error::Error::Unknow(
                "GET is not used in this test".to_string(),
            ))
        }

        fn post(
            &self,
            request: HttpRequest,
        ) -> impl std::future::Future<Output = Result<Self::Response, error::Error>> {
            let requests = Arc::clone(&self.requests);
            let status = self.status.get();

            async move {
                requests.lock().unwrap().push(RecordedRequest {
                    method: "POST",
                    url: request.url().to_string(),
                    bearer_token: request.bearer_token().map(str::to_owned),
                    body: request.body().map(str::to_owned),
                });

                Ok(FakeResponse::empty(status))
            }
        }
    }

    impl WriteHttpClient for NonSyncWriteHttpClient {
        fn post_send(
            &self,
            request: HttpRequest,
        ) -> impl std::future::Future<Output = Result<(u16, String), error::Error>>
        + Send
        + 'static
        + use<> {
            let requests = Arc::clone(&self.requests);
            let status = self.status.get();

            async move {
                requests.lock().unwrap().push(RecordedRequest {
                    method: "POST",
                    url: request.url().to_string(),
                    bearer_token: request.bearer_token().map(str::to_owned),
                    body: request.body().map(str::to_owned),
                });

                Ok((status, String::new()))
            }
        }
    }

    impl HttpClient for BorrowingGetHttpClient {
        type Response = FakeResponse;

        fn get(
            &self,
            _request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> {
            self.get_calls.set(self.get_calls.get() + 1);

            async {
                Ok(FakeResponse::json(
                    200,
                    &format!(r#"{{"value":"{}"}}"#, self.select_response),
                ))
            }
        }

        async fn post(&self, _request: HttpRequest) -> Result<Self::Response, error::Error> {
            Err(error::Error::Unknow(
                "POST is not used in this test".to_string(),
            ))
        }
    }

    impl HttpClient for BorrowingPostHttpClient {
        type Response = FakeResponse;

        async fn get(&self, _request: HttpRequest) -> Result<Self::Response, error::Error> {
            Err(error::Error::Unknow(
                "GET is not used in this test".to_string(),
            ))
        }

        async fn post(&self, _request: HttpRequest) -> Result<Self::Response, error::Error> {
            self.post_calls.set(self.post_calls.get() + 1);
            Ok(FakeResponse::json(200, r#"{"results":[]}"#))
        }
    }

    impl HttpClient for BorrowingQueryGetHttpClient {
        type Response = FakeResponse;

        fn get(
            &self,
            _request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> {
            self.get_calls.set(self.get_calls.get() + 1);

            async {
                Ok(FakeResponse::json(
                    200,
                    r#"{"results":[{"statement_id":0,"series":null}]}"#,
                ))
            }
        }

        async fn post(&self, _request: HttpRequest) -> Result<Self::Response, error::Error> {
            Err(error::Error::Unknow(
                "POST is not used in this test".to_string(),
            ))
        }
    }

    impl HttpClient for SplitQueryHttpClient {
        type Response = FakeResponse;

        fn get(
            &self,
            _request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> {
            self.get_calls.set(self.get_calls.get() + 1);

            async {
                Ok(FakeResponse::json(
                    200,
                    &format!(
                        r#"{{"results":[{{"statement_id":0,"value":"{}"}}]}}"#,
                        self.response_body
                    ),
                ))
            }
        }

        fn post(
            &self,
            _request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> {
            self.post_calls.set(self.post_calls.get() + 1);
            async { Ok(FakeResponse::json(200, r#"{"results":[]}"#)) }
        }
    }

    impl QueryHttpClient for SplitQueryHttpClient {
        fn send_get(
            &self,
            _request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static {
            let response_body = self.response_body.clone();

            async move {
                Ok(FakeResponse::json(
                    200,
                    &format!(
                        r#"{{"results":[{{"statement_id":0,"value":"{}"}}]}}"#,
                        response_body
                    ),
                ))
            }
        }

        fn send_post(
            &self,
            _request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static {
            let response = FakeResponse::json(200, r#"{"results":[]}"#);
            async move { Ok(response) }
        }
    }

    impl HttpResponse for NonSendResponse {
        fn status(&self) -> u16 {
            self.status
        }

        fn header(&self, _name: &str) -> Result<Option<&str>, error::Error> {
            Ok(None)
        }

        fn text(self) -> impl Future<Output = Result<String, error::Error>> {
            let body = (*self.body).clone();
            async move { Ok(body) }
        }

        fn bytes(self) -> impl Future<Output = Result<Bytes, error::Error>> {
            let body = (*self.body).clone();
            async move { Ok(Bytes::from(body)) }
        }

        fn json<T>(self) -> impl Future<Output = Result<T, error::Error>>
        where
            T: DeserializeOwned,
        {
            let body = (*self.body).clone();
            async move {
                serde_json::from_str(&body)
                    .map_err(|err| error::Error::Communication(err.to_string()))
            }
        }
    }

    impl HttpClient for NonSendResponseHttpClient {
        type Response = NonSendResponse;

        fn get(
            &self,
            _request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> {
            let body = self.response_body.clone();

            async move {
                Ok(NonSendResponse {
                    status: 200,
                    body: Rc::new(body),
                })
            }
        }

        async fn post(&self, _request: HttpRequest) -> Result<Self::Response, error::Error> {
            Err(error::Error::Unknow(
                "POST is not used in this test".to_string(),
            ))
        }
    }

    impl HttpResponse for NonSendBodyFutureResponse {
        fn status(&self) -> u16 {
            self.status
        }

        fn header(&self, _name: &str) -> Result<Option<&str>, error::Error> {
            Ok(None)
        }

        fn text(self) -> impl Future<Output = Result<String, error::Error>> {
            let body = self.body;
            async move { Ok((*body).clone()) }
        }

        fn bytes(self) -> impl Future<Output = Result<Bytes, error::Error>> {
            let body = self.body;
            async move { Ok(Bytes::from((*body).clone())) }
        }

        fn json<T>(self) -> impl Future<Output = Result<T, error::Error>>
        where
            T: DeserializeOwned,
        {
            let body = self.body;
            async move {
                serde_json::from_str(body.as_ref())
                    .map_err(|err| error::Error::Communication(err.to_string()))
            }
        }
    }

    impl HttpClient for NonSendBodyFutureHttpClient {
        type Response = NonSendBodyFutureResponse;

        fn get(
            &self,
            _request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> {
            let body = self.response_body.clone();

            async move {
                Ok(NonSendBodyFutureResponse {
                    status: 200,
                    body: Rc::new(body),
                })
            }
        }

        async fn post(&self, _request: HttpRequest) -> Result<Self::Response, error::Error> {
            Err(error::Error::Unknow(
                "POST is not used in this test".to_string(),
            ))
        }
    }

    impl HttpResponse for VersionHeaderResponse {
        fn status(&self) -> u16 {
            self.status
        }

        fn header(&self, name: &str) -> Result<Option<&str>, error::Error> {
            if name != "X-Influxdb-Version" {
                return Ok(None);
            }

            match &self.version_header {
                VersionHeader::Missing => Ok(None),
                VersionHeader::Valid(value) => Ok(Some(value.as_str())),
                VersionHeader::InvalidUtf8 => Err(error::Error::Communication(
                    "invalid header value".to_string(),
                )),
            }
        }

        async fn text(self) -> Result<String, error::Error> {
            Ok(String::new())
        }

        async fn bytes(self) -> Result<Bytes, error::Error> {
            Ok(Bytes::new())
        }

        async fn json<T>(self) -> Result<T, error::Error>
        where
            T: DeserializeOwned,
        {
            Err(error::Error::Unknow(
                "JSON is not used in this test".to_string(),
            ))
        }
    }

    impl HttpClient for VersionHeaderHttpClient {
        type Response = VersionHeaderResponse;

        fn get(
            &self,
            _request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> {
            let response = VersionHeaderResponse {
                status: self.response.status,
                version_header: match &self.response.version_header {
                    VersionHeader::Missing => VersionHeader::Missing,
                    VersionHeader::Valid(value) => VersionHeader::Valid(value.clone()),
                    VersionHeader::InvalidUtf8 => VersionHeader::InvalidUtf8,
                },
            };

            async move { Ok(response) }
        }

        async fn post(&self, _request: HttpRequest) -> Result<Self::Response, error::Error> {
            Err(error::Error::Unknow(
                "POST is not used in this test".to_string(),
            ))
        }
    }

    impl HttpResponse for ChunkedFakeResponse {
        fn status(&self) -> u16 {
            self.status
        }

        fn header(&self, _name: &str) -> Result<Option<&str>, error::Error> {
            Ok(None)
        }

        async fn text(self) -> Result<String, error::Error> {
            Err(error::Error::Unknow(
                "text is not used in this test".to_string(),
            ))
        }

        async fn bytes(self) -> Result<Bytes, error::Error> {
            panic!("chunked query should not eagerly buffer the whole response body")
        }

        async fn json<T>(self) -> Result<T, error::Error>
        where
            T: DeserializeOwned,
        {
            Err(error::Error::Unknow(
                "json is not used in this test".to_string(),
            ))
        }
    }

    impl ChunkedHttpResponse for ChunkedFakeResponse {
        type Stream = futures::stream::Iter<
            std::iter::Map<std::vec::IntoIter<Vec<u8>>, fn(Vec<u8>) -> Result<Bytes, error::Error>>,
        >;

        async fn into_chunk_stream(self) -> Result<Self::Stream, error::Error> {
            fn into_bytes(segment: Vec<u8>) -> Result<Bytes, error::Error> {
                Ok(Bytes::from(segment))
            }

            Ok(futures::stream::iter(self.segments.into_iter().map(
                into_bytes as fn(Vec<u8>) -> Result<Bytes, error::Error>,
            )))
        }
    }

    impl QueryHttpResponse for ChunkedFakeResponse {
        async fn json_send<T>(self) -> Result<T, error::Error>
        where
            T: DeserializeOwned + 'static,
        {
            Err(error::Error::Unknow(
                "json_send is not used in this test".to_string(),
            ))
        }
    }

    impl QueryChunkedHttpResponse for ChunkedFakeResponse {
        async fn into_chunk_stream_send(self) -> Result<Self::Stream, error::Error> {
            self.into_chunk_stream().await
        }
    }

    impl ChunkedHttpClient {
        fn new(response: ChunkedFakeResponse) -> Self {
            Self { response }
        }
    }

    impl BorrowingChunkedHttpClient {
        fn new(response: ChunkedFakeResponse) -> Self {
            Self { response }
        }
    }

    impl HttpClient for ChunkedHttpClient {
        type Response = ChunkedFakeResponse;

        fn get(
            &self,
            _request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> {
            let response = self.response.clone();
            async move { Ok(response) }
        }

        async fn post(&self, _request: HttpRequest) -> Result<Self::Response, error::Error> {
            Err(error::Error::Unknow(
                "POST is not used in this test".to_string(),
            ))
        }
    }

    impl QueryHttpClient for ChunkedHttpClient {
        fn send_get(
            &self,
            _request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static {
            let response = self.response.clone();
            async move { Ok(response) }
        }

        fn send_post(
            &self,
            _request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static {
            let error = error::Error::Unknow("POST is not used in this test".to_string());
            async move { Err(error) }
        }
    }

    impl HttpClient for BorrowingChunkedHttpClient {
        type Response = ChunkedFakeResponse;

        fn get(
            &self,
            _request: HttpRequest,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> {
            let response = self.response.clone();
            async move { Ok(response) }
        }

        async fn post(&self, _request: HttpRequest) -> Result<Self::Response, error::Error> {
            Err(error::Error::Unknow(
                "POST is not used in this test".to_string(),
            ))
        }
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Runtime::new().unwrap().block_on(future)
    }

    fn assert_send_static<F>(future: F) -> F
    where
        F: std::future::Future + Send + 'static,
    {
        future
    }

    #[test]
    fn custom_http_client_select_query_uses_get_and_preserves_jwt_token() {
        let http_client =
            RecordingHttpClient::new(vec![FakeResponse::json(200, r#"{"results":[]}"#)]);
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            http_client,
        )
        .set_jwt_token("jwt-token");

        let result = block_on(client.query("select * from cpu", None)).unwrap();

        assert_eq!(result, Some(Vec::new()));

        let requests = client.client.take_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[0].bearer_token.as_deref(), Some("jwt-token"));
        assert!(requests[0].url.contains("/query"));
        assert!(requests[0].url.contains("db=metrics"));
        assert!(requests[0].url.contains("q=select"));
    }

    #[test]
    fn ping_preserves_jwt_token() {
        let http_client = RecordingHttpClient::new(vec![FakeResponse::empty(204)]);
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            http_client,
        )
        .set_jwt_token("jwt-token");

        let ping = block_on(client.ping());

        assert!(ping);

        let requests = client.client.take_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[0].bearer_token.as_deref(), Some("jwt-token"));
        assert!(requests[0].url.contains("/ping"));
    }

    #[test]
    fn get_version_preserves_jwt_token() {
        let http_client = RecordingHttpClient::new(vec![FakeResponse {
            status: 204,
            headers: HashMap::from([("X-Influxdb-Version".to_string(), "1.8.10".to_string())]),
            body: String::new(),
        }]);
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            http_client,
        )
        .set_jwt_token("jwt-token");

        let version = block_on(client.get_version());

        assert_eq!(version.as_deref(), Some("1.8.10"));

        let requests = client.client.take_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[0].bearer_token.as_deref(), Some("jwt-token"));
        assert!(requests[0].url.contains("/ping"));
    }

    #[test]
    fn custom_http_client_non_select_query_uses_post() {
        let http_client =
            RecordingHttpClient::new(vec![FakeResponse::json(200, r#"{"results":[]}"#)]);
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            http_client,
        );

        block_on(client.query("create database metrics", None)).unwrap();

        let requests = client.client.take_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "POST");
    }

    #[test]
    fn query_returns_statement_error_from_400_response() {
        let http_client = RecordingHttpClient::new(vec![FakeResponse::json(
            400,
            r#"{"results":[{"statement_id":0,"error":"bad query"}]}"#,
        )]);
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            http_client,
        );

        let result = block_on(client.query("create database metrics", None));

        assert_eq!(
            result,
            Err(error::Error::SyntaxError("bad query".to_string()))
        );
    }

    #[test]
    fn query_returns_statement_error_from_200_response() {
        let http_client = RecordingHttpClient::new(vec![FakeResponse::json(
            200,
            r#"{"results":[{"statement_id":0,"error":"bad query"}]}"#,
        )]);
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            http_client,
        );

        let result = block_on(client.query("create database metrics", None));

        assert_eq!(
            result,
            Err(error::Error::SyntaxError("bad query".to_string()))
        );
    }

    #[test]
    fn query_chunked_uses_chunk_stream_without_buffering_the_whole_body() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            ChunkedHttpClient::new(ChunkedFakeResponse {
                status: 200,
                segments: vec![
                    br#"{"results":[{"statement_id":0"#.to_vec(),
                    br#","series":null}]}"#.to_vec(),
                ],
            }),
        );

        block_on(async {
            let mut query = client
                .query_chunked("select value from cpu", None)
                .await
                .unwrap();
            let first = query.next().await.unwrap().unwrap();

            assert_eq!(first.results.unwrap()[0].statement_id, Some(0));
            assert!(query.next().await.is_none());
        });
    }

    #[test]
    fn query_chunked_returns_statement_error_from_chunk() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            ChunkedHttpClient::new(ChunkedFakeResponse {
                status: 200,
                segments: vec![
                    br#"{"results":[{"statement_id":0"#.to_vec(),
                    br#","error":"bad query"}]}"#.to_vec(),
                ],
            }),
        );

        block_on(async {
            let mut query = client
                .query_chunked("select value from cpu", None)
                .await
                .unwrap();

            assert_eq!(
                query.next().await.unwrap(),
                Err(error::Error::SyntaxError("bad query".to_string()))
            );
            assert!(query.next().await.is_none());
        });
    }

    #[test]
    fn query_chunked_setup_can_be_spawned_with_cloneable_http_client() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            ChunkedHttpClient::new(ChunkedFakeResponse {
                status: 200,
                segments: vec![
                    br#"{"results":[{"statement_id":0"#.to_vec(),
                    br#","series":null}]}"#.to_vec(),
                ],
            }),
        );

        block_on(async {
            let mut query = tokio::spawn(client.query_chunked("select value from cpu", None))
                .await
                .unwrap()
                .unwrap();

            let first = query.next().await.unwrap().unwrap();
            assert_eq!(first.results.unwrap()[0].statement_id, Some(0));
            assert!(query.next().await.is_none());
        });
    }

    #[cfg(feature = "reqwest")]
    #[test]
    fn reqwest_query_chunked_streams_queries_on_current_thread_runtime() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).unwrap();

            let first = br#"{"results":[{"statement_id":0,"series":null}]}
"#;
            let second = br#"{"results":[{"statement_id":1,"series":null}]}
"#;

            write!(
                stream,
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Type: application/json\r\n\r\n{:X}\r\n",
                first.len()
            )
            .unwrap();
            stream.write_all(first).unwrap();
            stream.write_all(b"\r\n").unwrap();
            write!(stream, "{:X}\r\n", second.len()).unwrap();
            stream.write_all(second).unwrap();
            stream.write_all(b"\r\n0\r\n\r\n").unwrap();
            stream.flush().unwrap();
        });

        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let client =
                    Client::new(Url::parse(&format!("http://{address}")).unwrap(), "metrics");
                let mut query = client
                    .query_chunked("select value from cpu", None)
                    .await
                    .unwrap();

                let first = query.next().await.unwrap().unwrap();
                let second = query.next().await.unwrap().unwrap();

                assert_eq!(first.results.unwrap()[0].statement_id, Some(0));
                assert_eq!(second.results.unwrap()[0].statement_id, Some(1));
                assert!(query.next().await.is_none());
            });

        server.join().unwrap();
    }

    #[test]
    fn custom_http_client_write_points_preserves_jwt_token() {
        let http_client = RecordingHttpClient::new(vec![FakeResponse::empty(204)]);
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            http_client,
        )
        .set_jwt_token("jwt-token");
        let point = Point::new("cpu").add_field("value", 1).add_timestamp(42);

        block_on(client.write_point(point, Some(Precision::Seconds), None)).unwrap();

        let requests = client.client.take_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].bearer_token.as_deref(), Some("jwt-token"));
        assert_eq!(requests[0].body.as_deref(), Some("cpu value=1i 42\n"));
        assert!(requests[0].url.contains("/write"));
    }

    #[test]
    fn custom_http_client_write_points_uses_post_with_body() {
        let http_client = RecordingHttpClient::new(vec![FakeResponse::empty(204)]);
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            http_client,
        );
        let point = Point::new("cpu").add_field("value", 1).add_timestamp(42);

        block_on(client.write_point(point, Some(Precision::Seconds), None)).unwrap();

        let requests = client.client.take_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].body.as_deref(), Some("cpu value=1i 42\n"));
        assert!(requests[0].url.contains("/write"));
        assert!(requests[0].url.contains("precision=s"));
    }

    #[test]
    fn write_point_can_be_spawned_with_non_sync_http_client() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            NonSyncWriteHttpClient::new(204),
        );
        let point = Point::new("cpu").add_field("value", 1).add_timestamp(42);

        block_on(async {
            tokio::spawn(client.write_point(point, Some(Precision::Seconds), None))
                .await
                .unwrap()
                .unwrap();
        });

        let requests = client.client.take_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].body.as_deref(), Some("cpu value=1i 42\n"));
    }

    #[test]
    fn query_can_be_spawned_with_cloneable_http_client() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            RecordingHttpClient::new(vec![FakeResponse::json(
                200,
                r#"{"results":[{"statement_id":0,"series":null}]}"#,
            )]),
        );

        block_on(async {
            let result = tokio::spawn(client.query("select value from cpu", None))
                .await
                .unwrap()
                .unwrap();

            assert_eq!(result.unwrap()[0].statement_id, Some(0));
        });
    }

    #[test]
    fn owned_query_futures_require_spawn_safe_query_transport() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            SplitQueryHttpClient::new("owned"),
        );

        let _query = assert_send_static(client.query("select value from cpu", None));
        let _management = assert_send_static(client.drop_measurement("cpu"));
    }

    #[test]
    fn send_request_borrow_supports_get_futures_that_borrow_non_sync_clients() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            BorrowingGetHttpClient::new("borrowed"),
        );

        let response: NonSendQueryResult = block_on(async {
            client
                .send_request_borrow("select value from cpu", None, false)
                .await
                .unwrap()
                .json::<NonSendQueryResult>()
                .await
                .unwrap()
        });

        assert_eq!(
            response,
            NonSendQueryResult {
                value: "borrowed".to_string(),
                _not_send: PhantomData,
            }
        );
        assert_eq!(client.client.get_calls.get(), 1);
    }

    #[test]
    fn query_borrow_supports_get_futures_that_borrow_non_sync_clients() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            BorrowingQueryGetHttpClient::new(),
        );

        let result = block_on(client.query_borrow("select value from cpu", None)).unwrap();

        assert_eq!(result.unwrap()[0].statement_id, Some(0));
        assert_eq!(client.client.get_calls.get(), 1);
    }

    #[test]
    fn send_request_supports_post_futures_that_borrow_non_sync_clients() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            BorrowingPostHttpClient::new(),
        );

        let query = block_on(async {
            client
                .send_request_borrow("create database metrics", None, false)
                .await
                .unwrap()
                .json::<Query>()
                .await
                .unwrap()
        });

        assert_eq!(query.results, Some(Vec::new()));
        assert_eq!(client.client.post_calls.get(), 1);
    }

    #[test]
    fn query_borrow_supports_post_futures_that_borrow_non_sync_clients() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            BorrowingPostHttpClient::new(),
        );

        let result = block_on(client.query_borrow("create database metrics", None)).unwrap();

        assert_eq!(result, Some(Vec::new()));
        assert_eq!(client.client.post_calls.get(), 1);
    }

    #[test]
    fn query_backed_management_apis_offer_borrow_variants() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            BorrowingPostHttpClient::new(),
        );

        block_on(async {
            client.drop_measurement_borrow("cpu").await.unwrap();
            client.create_database_borrow("metrics_2").await.unwrap();
            client.drop_database_borrow("metrics_2").await.unwrap();
            client
                .create_user_borrow("alice", "secret", false)
                .await
                .unwrap();
            client.drop_user_borrow("alice").await.unwrap();
            client
                .set_user_password_borrow("alice", "secret_2")
                .await
                .unwrap();
            client.grant_admin_privileges_borrow("alice").await.unwrap();
            client
                .revoke_admin_privileges_borrow("alice")
                .await
                .unwrap();
            client
                .grant_privilege_borrow("alice", "metrics", "read")
                .await
                .unwrap();
            client
                .revoke_privilege_borrow("alice", "metrics", "read")
                .await
                .unwrap();
            client
                .create_retention_policy_borrow("rp", "1h", "1", false, None)
                .await
                .unwrap();
            client
                .drop_retention_policy_borrow("rp", None)
                .await
                .unwrap();
        });

        assert_eq!(client.client.post_calls.get(), 12);
    }

    #[test]
    fn query_chunked_borrow_supports_non_clone_http_client() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            BorrowingChunkedHttpClient::new(ChunkedFakeResponse {
                status: 200,
                segments: vec![
                    br#"{"results":[{"statement_id":0"#.to_vec(),
                    br#","series":null}]}"#.to_vec(),
                ],
            }),
        );

        block_on(async {
            let mut query = client
                .query_chunked_borrow("select value from cpu", None)
                .await
                .unwrap();
            let first = query.next().await.unwrap().unwrap();

            assert_eq!(first.results.unwrap()[0].statement_id, Some(0));
            assert!(query.next().await.is_none());
        });
    }

    #[test]
    fn query_supports_non_send_response_types() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            NonSendResponseHttpClient::new(r#"{"value":"owned"}"#),
        );

        let response: NonSendQueryResult = block_on(async {
            client
                .send_request_borrow("select value from cpu", None, false)
                .await
                .unwrap()
                .json()
                .await
                .unwrap()
        });

        assert_eq!(
            response,
            NonSendQueryResult {
                value: "owned".to_string(),
                _not_send: PhantomData,
            }
        );
    }

    #[test]
    fn query_supports_non_send_body_futures() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            NonSendBodyFutureHttpClient::new(r#"{"value":"body"}"#),
        );

        let response: NonSendQueryResult = block_on(async {
            client
                .send_request_borrow("select value from cpu", None, false)
                .await
                .unwrap()
                .json()
                .await
                .unwrap()
        });

        assert_eq!(
            response,
            NonSendQueryResult {
                value: "body".to_string(),
                _not_send: PhantomData,
            }
        );
    }

    #[test]
    fn get_version_returns_none_for_invalid_header_values() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            VersionHeaderHttpClient::new(VersionHeaderResponse {
                status: 204,
                version_header: VersionHeader::InvalidUtf8,
            }),
        );

        let version = block_on(client.get_version());

        assert_eq!(version, None);
    }

    #[test]
    fn get_version_returns_fallback_when_header_is_missing() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            VersionHeaderHttpClient::new(VersionHeaderResponse {
                status: 204,
                version_header: VersionHeader::Missing,
            }),
        );

        let version = block_on(client.get_version());

        assert_eq!(version.as_deref(), Some("Don't know"));
    }

    #[test]
    fn get_version_returns_header_value_when_it_is_valid() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            VersionHeaderHttpClient::new(VersionHeaderResponse {
                status: 204,
                version_header: VersionHeader::Valid("1.8.10".to_string()),
            }),
        );

        let version = block_on(client.get_version());

        assert_eq!(version.as_deref(), Some("1.8.10"));
    }
}
