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
        BorrowChunkedHttpResponse, BorrowHttpClient, BorrowHttpResponse, ChunkedHttpResponse,
        HttpClient, HttpMethod, HttpRequest, HttpResponse,
    },
    keys::{ensure_query_success, query_syntax_error},
    serialization,
};

#[cfg(feature = "reqwest")]
fn reqwest_request_builder(
    client: &ReqwestClient,
    method: HttpMethod,
    request: HttpRequest<'_>,
) -> reqwest::RequestBuilder {
    let (url, body, bearer_token) = request.into_parts();
    let builder = match method {
        HttpMethod::Get => client.get(url),
        HttpMethod::Post => client.post(url),
    };
    let builder = if let Some(token) = bearer_token {
        builder.bearer_auth(token.as_ref())
    } else {
        builder
    };

    if let Some(body) = body {
        builder.body(body.into_owned())
    } else {
        builder
    }
}

#[cfg(feature = "reqwest")]
impl BorrowHttpResponse for ReqwestResponse {
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
        T: DeserializeOwned + 'static,
    {
        Ok(reqwest::Response::json(self).await?)
    }
}

#[cfg(feature = "reqwest")]
impl BorrowChunkedHttpResponse for ReqwestResponse {
    type Stream =
        std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, error::Error>> + Send + 'static>>;

    async fn into_chunk_stream(self) -> Result<Self::Stream, error::Error> {
        Ok(Box::pin(
            reqwest::Response::bytes_stream(self).map_err(Into::into),
        ))
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
impl HttpClient for ReqwestClient {
    type Response = ReqwestResponse;

    fn send(
        &self,
        method: HttpMethod,
        request: HttpRequest<'static>,
    ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static + use<> {
        let client = self.clone();

        async move {
            Ok(reqwest_request_builder(&client, method, request)
                .send()
                .await?)
        }
    }
}

#[cfg(feature = "reqwest")]
impl BorrowHttpClient for ReqwestClient {
    type Response = ReqwestResponse;

    fn send<'a>(
        &'a self,
        method: HttpMethod,
        request: HttpRequest<'a>,
    ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'a> {
        async move {
            Ok(reqwest_request_builder(self, method, request)
                .send()
                .await?)
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

    fn build_request(&self, url: Url) -> HttpRequest<'_> {
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

async fn check_query_status_borrow<R, F, Fut>(res: R, parse_json: F) -> Result<R, error::Error>
where
    R: BorrowHttpResponse,
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

fn map_write_response(status: u16, body: String) -> Result<(), error::Error> {
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

fn map_version_header(header: Result<Option<&str>, error::Error>) -> Option<String> {
    match header {
        Ok(Some(header)) => Some(header.to_owned()),
        Ok(None) => Some(String::from("Don't know")),
        Err(_) => None,
    }
}

impl<T> Client<T> {
    fn build_ping_request(&self) -> HttpRequest<'_> {
        self.build_request(self.build_url("ping", None))
    }

    /// Query whether the corresponding database exists.
    pub fn ping(&self) -> impl Future<Output = bool> + Send + 'static + use<T>
    where
        T: HttpClient,
    {
        let response_future = self
            .client
            .send(HttpMethod::Get, self.build_ping_request().into_owned());

        async move {
            response_future
                .await
                .map(|res| matches!(res.status(), 204))
                .unwrap_or(false)
        }
    }

    /// Borrowing variant of [`Client::ping`].
    pub fn ping_borrow(&self) -> impl Future<Output = bool> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
        let request_future = self.client.send(HttpMethod::Get, self.build_ping_request());

        async move {
            request_future
                .await
                .map(|res| matches!(res.status(), 204))
                .unwrap_or(false)
        }
    }

    /// Query the version of the database and return the version number.
    pub fn get_version(&self) -> impl Future<Output = Option<String>> + Send + 'static + use<T>
    where
        T: HttpClient,
    {
        let response_future = self
            .client
            .send(HttpMethod::Get, self.build_ping_request().into_owned());

        async move {
            if let Ok(res) = response_future.await {
                match res.status() {
                    204 => map_version_header(res.header("X-Influxdb-Version")),
                    _ => None,
                }
            } else {
                None
            }
        }
    }

    /// Borrowing variant of [`Client::get_version`].
    pub fn get_version_borrow(&self) -> impl Future<Output = Option<String>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
        let request_future = self.client.send(HttpMethod::Get, self.build_ping_request());

        async move {
            if let Ok(res) = request_future.await {
                match res.status() {
                    204 => map_version_header(res.header("X-Influxdb-Version")),
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
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
    {
        let line = serialization::line_serialization(std::iter::once(&point));
        self.write_line(line, precision, rp)
    }

    /// Write a point to the database through a future that borrows the configured HTTP client.
    pub fn write_point_borrow<'a>(
        &self,
        point: Point<'a>,
        precision: Option<Precision>,
        rp: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>>
    where
        T: BorrowHttpClient,
    {
        let line = serialization::line_serialization(std::iter::once(&point));
        self.write_line_borrow(line, precision, rp)
    }

    /// Write multiple points to the database
    pub fn write_points<'a, I, P>(
        &self,
        points: I,
        precision: Option<Precision>,
        rp: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static + use<T, I, P>
    where
        I: IntoIterator<Item = P>,
        P: Borrow<Point<'a>>,
        T: HttpClient,
    {
        let line = serialization::line_serialization(points);
        self.write_line(line, precision, rp)
    }

    /// Write multiple points through a future that borrows the configured HTTP client.
    pub fn write_points_borrow<'a, I, P>(
        &self,
        points: I,
        precision: Option<Precision>,
        rp: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>>
    where
        I: IntoIterator<Item = P>,
        P: Borrow<Point<'a>>,
        T: BorrowHttpClient,
    {
        let line = serialization::line_serialization(points);
        self.write_line_borrow(line, precision, rp)
    }

    fn build_write_request(
        &self,
        line: String,
        precision: Option<Precision>,
        rp: Option<&str>,
    ) -> HttpRequest<'_> {
        let mut param = vec![("db", self.db.as_str())];

        match precision {
            Some(ref t) => param.push(("precision", t.to_str())),
            None => param.push(("precision", Precision::Nanoseconds.to_str())),
        };

        if let Some(t) = rp {
            param.push(("rp", t))
        }

        let url = self.build_url("write", Some(param));
        self.build_request(url).with_body(line)
    }

    fn write_line(
        &self,
        line: String,
        precision: Option<Precision>,
        rp: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
    {
        let request = self.build_write_request(line, precision, rp).into_owned();
        let response_future = self.client.send(HttpMethod::Post, request);

        async move {
            let response = response_future.await?;
            let status = response.status();

            if status == 204 {
                return Ok(());
            }

            let body = response.text().await?;
            map_write_response(status, body)
        }
    }

    fn write_line_borrow(
        &self,
        line: String,
        precision: Option<Precision>,
        rp: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
        let request = self.build_write_request(line, precision, rp);
        let response_future = self.client.send(HttpMethod::Post, request);

        async move {
            let response = response_future.await?;
            let status = response.status();

            if status == 204 {
                return Ok(());
            }

            let body = response.text().await?;
            map_write_response(status, body)
        }
    }

    /// Query and return data, the data type is `Option<Vec<Node>>`
    pub fn query(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<Output = Result<Option<Vec<Node>>, error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
    {
        let raw_future = self.query_raw(q, epoch);
        async move { Ok(raw_future.await?.results) }
    }

    /// Query and return data through a future that borrows the configured HTTP client.
    pub fn query_borrow(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<Output = Result<Option<Vec<Node>>, error::Error>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
        self.query_raw_borrow(q, epoch).map_ok(|t| t.results)
    }

    /// Query and return chunked query documents.
    pub fn query_chunked(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<
        Output = Result<
            ChunkedQuery<<<T as HttpClient>::Response as ChunkedHttpResponse>::Stream>,
            error::Error,
        >,
    >
    + Send
    + 'static
    + use<T>
    where
        T: HttpClient,
        <T as HttpClient>::Response: ChunkedHttpResponse,
    {
        self.query_raw_chunked(q, epoch)
    }

    /// Query and return chunked query documents through a future that borrows the configured HTTP client.
    pub fn query_chunked_borrow(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<
        Output = Result<
            ChunkedQuery<<<T as BorrowHttpClient>::Response as BorrowChunkedHttpResponse>::Stream>,
            error::Error,
        >,
    > + use<'_, T>
    where
        T: BorrowHttpClient,
        <T as BorrowHttpClient>::Response: BorrowChunkedHttpResponse,
    {
        self.query_raw_chunked_borrow(q, epoch)
    }

    /// Drop measurement
    pub fn drop_measurement(
        &self,
        measurement: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
    {
        let sql = format!(
            "Drop measurement {}",
            serialization::quote_ident(measurement)
        );

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::drop_measurement`].
    pub fn drop_measurement_borrow(
        &self,
        measurement: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
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
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
    {
        let sql = format!("Create database {}", serialization::quote_ident(dbname));

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::create_database`].
    pub fn create_database_borrow(
        &self,
        dbname: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
        let sql = format!("Create database {}", serialization::quote_ident(dbname));

        self.execute_query_borrow(sql)
    }

    /// Drop a database from InfluxDB.
    pub fn drop_database(
        &self,
        dbname: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
    {
        let sql = format!("Drop database {}", serialization::quote_ident(dbname));

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::drop_database`].
    pub fn drop_database_borrow(
        &self,
        dbname: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
        let sql = format!("Drop database {}", serialization::quote_ident(dbname));

        self.execute_query_borrow(sql)
    }

    /// Create a new user in InfluxDB.
    pub fn create_user(
        &self,
        user: &str,
        passwd: &str,
        admin: bool,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
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

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::create_user`].
    pub fn create_user_borrow(
        &self,
        user: &str,
        passwd: &str,
        admin: bool,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
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
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
    {
        let sql = format!("Drop user {}", serialization::quote_ident(user));

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::drop_user`].
    pub fn drop_user_borrow(
        &self,
        user: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
        let sql = format!("Drop user {}", serialization::quote_ident(user));

        self.execute_query_borrow(sql)
    }

    /// Change the password of an existing user.
    pub fn set_user_password(
        &self,
        user: &str,
        passwd: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
    {
        let sql = format!(
            "Set password for {}={}",
            serialization::quote_ident(user),
            serialization::quote_literal(passwd)
        );

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::set_user_password`].
    pub fn set_user_password_borrow(
        &self,
        user: &str,
        passwd: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
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
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
    {
        let sql = format!(
            "Grant all privileges to {}",
            serialization::quote_ident(user)
        );

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::grant_admin_privileges`].
    pub fn grant_admin_privileges_borrow(
        &self,
        user: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
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
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
    {
        let sql = format!(
            "Revoke all privileges from {}",
            serialization::quote_ident(user)
        );

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::revoke_admin_privileges`].
    pub fn revoke_admin_privileges_borrow(
        &self,
        user: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
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
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
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
                futures::future::Either::Right(self.query_raw(&sql, None).map_ok(|_| ()))
            }
        }
    }

    /// Borrowing variant of [`Client::grant_privilege`].
    pub fn grant_privilege_borrow(
        &self,
        user: &str,
        db: &str,
        privilege: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
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
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
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
                futures::future::Either::Right(self.query_raw(&sql, None).map_ok(|_| ()))
            }
        }
    }

    /// Borrowing variant of [`Client::revoke_privilege`].
    pub fn revoke_privilege_borrow(
        &self,
        user: &str,
        db: &str,
        privilege: &str,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
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
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
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

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::create_retention_policy`].
    pub fn create_retention_policy_borrow(
        &self,
        name: &str,
        duration: &str,
        replication: &str,
        default: bool,
        db: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
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
    ) -> impl Future<Output = Result<(), error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
    {
        let database = { if let Some(t) = db { t } else { &self.db } };

        let sql = format!(
            "Drop retention policy {} on {}",
            serialization::quote_ident(name),
            serialization::quote_ident(database)
        );

        self.query_raw(&sql, None).map_ok(|_| ())
    }

    /// Borrowing variant of [`Client::drop_retention_policy`].
    pub fn drop_retention_policy_borrow(
        &self,
        name: &str,
        db: Option<&str>,
    ) -> impl Future<Output = Result<(), error::Error>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
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
    ) -> (HttpRequest<'_>, bool) {
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
    ) -> impl Future<Output = Result<<T as BorrowHttpClient>::Response, error::Error>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
        let (request, is_read_query) = self.build_query_request(q, epoch, chunked);
        let method = if is_read_query {
            HttpMethod::Get
        } else {
            HttpMethod::Post
        };
        let request_future = self.client.send(method, request);

        async move {
            let res = request_future.await?;
            check_query_status_borrow(res, |r| r.json::<Query>()).await
        }
    }

    fn send_request(
        &self,
        method: HttpMethod,
        request: HttpRequest<'static>,
    ) -> impl Future<Output = Result<<T as HttpClient>::Response, error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
    {
        let response_future = self.client.send(method, request);

        async move {
            let res = response_future.await?;
            check_query_status(res, |r| r.json::<Query>()).await
        }
    }

    /// Query and return to the native json structure through the spawn-safe query path.
    fn query_raw(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<Output = Result<Query, error::Error>> + Send + 'static + use<T>
    where
        T: HttpClient,
    {
        let (request, is_read_query) = self.build_query_request(&q, epoch, false);
        let method = if is_read_query {
            HttpMethod::Get
        } else {
            HttpMethod::Post
        };
        let resp_future = self.send_request(method, request.into_owned());
        async move {
            let query = resp_future.await?.json().await?;
            ensure_query_success(query)
        }
    }

    /// Query and return to the native json structure through a future that borrows the HTTP client.
    fn query_raw_borrow(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<Output = Result<Query, error::Error>> + use<'_, T>
    where
        T: BorrowHttpClient,
    {
        let resp_future = self.send_request_borrow(q, epoch, false);
        async move {
            let query = resp_future.await?.json().await?;
            ensure_query_success(query)
        }
    }

    async fn execute_query_borrow(&self, sql: String) -> Result<(), error::Error>
    where
        T: BorrowHttpClient,
    {
        self.query_raw_borrow(&sql, None).await?;
        Ok(())
    }

    /// Query and return chunked query documents through the spawn-safe query path.
    fn query_raw_chunked(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<
        Output = Result<
            ChunkedQuery<<<T as HttpClient>::Response as ChunkedHttpResponse>::Stream>,
            error::Error,
        >,
    >
    + Send
    + 'static
    + use<T>
    where
        T: HttpClient,
        <T as HttpClient>::Response: ChunkedHttpResponse,
    {
        let (request, is_read_query) = self.build_query_request(&q, epoch, true);
        let method = if is_read_query {
            HttpMethod::Get
        } else {
            HttpMethod::Post
        };
        let resp_future = self.send_request(method, request.into_owned());
        async move {
            let response = resp_future.await?;
            let stream = response.into_chunk_stream().await?;
            Ok(ChunkedQuery::new(stream))
        }
    }

    /// Query and return chunked query documents through a future that borrows the HTTP client.
    fn query_raw_chunked_borrow(
        &self,
        q: &str,
        epoch: Option<Precision>,
    ) -> impl Future<
        Output = Result<
            ChunkedQuery<<<T as BorrowHttpClient>::Response as BorrowChunkedHttpResponse>::Stream>,
            error::Error,
        >,
    > + use<'_, T>
    where
        T: BorrowHttpClient,
        <T as BorrowHttpClient>::Response: BorrowChunkedHttpResponse,
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
            BorrowChunkedHttpResponse, BorrowHttpClient, BorrowHttpResponse, ChunkedHttpResponse,
            HttpClient, HttpMethod, HttpRequest, HttpResponse,
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

    impl BorrowHttpResponse for FakeResponse {
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
            T: DeserializeOwned + 'static,
        {
            serde_json::from_str(&self.body)
                .map_err(|err| error::Error::Communication(err.to_string()))
        }
    }

    impl HttpResponse for OwnedOnlyResponse {
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
            request: HttpRequest<'_>,
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

    struct BorrowingWriteHttpClient {
        requests: Rc<Cell<u16>>,
        last_request: Rc<std::cell::RefCell<Option<RecordedRequest>>>,
        response: FakeResponse,
    }

    impl BorrowingWriteHttpClient {
        fn new() -> Self {
            Self::with_response(FakeResponse::empty(204))
        }

        fn with_response(response: FakeResponse) -> Self {
            Self {
                requests: Rc::new(Cell::new(0)),
                last_request: Rc::new(std::cell::RefCell::new(None)),
                response,
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

    #[derive(Clone)]
    struct OwnedOnlyResponse {
        status: u16,
        headers: HashMap<String, String>,
        body: String,
    }

    impl OwnedOnlyResponse {
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

        fn with_header(status: u16, name: &str, value: &str) -> Self {
            Self {
                status,
                headers: HashMap::from([(name.to_string(), value.to_string())]),
                body: String::new(),
            }
        }
    }

    #[derive(Clone)]
    struct OwnedOnlyChunkedResponse {
        status: u16,
        segments: Vec<Vec<u8>>,
    }

    #[derive(Clone)]
    struct OwnedOnlyHttpClient {
        requests: Arc<Mutex<Vec<RecordedRequest>>>,
        responses: Arc<Mutex<Vec<OwnedOnlyResponse>>>,
    }

    #[derive(Clone)]
    struct OwnedOnlyChunkedHttpClient {
        response: OwnedOnlyChunkedResponse,
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

    impl BorrowHttpClient for RecordingHttpClient {
        type Response = FakeResponse;

        fn send<'a>(
            &'a self,
            method: HttpMethod,
            request: HttpRequest<'a>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'a> {
            let requests = Arc::clone(&self.requests);
            let responses = Arc::clone(&self.responses);
            let method = match method {
                HttpMethod::Get => "GET",
                HttpMethod::Post => "POST",
            };

            async move { Self::record_with(requests, responses, method, request) }
        }
    }

    impl HttpClient for OwnedOnlyHttpClient {
        type Response = OwnedOnlyResponse;

        fn send(
            &self,
            method: HttpMethod,
            request: HttpRequest<'static>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static + use<>
        {
            let requests = Arc::clone(&self.requests);
            let responses = Arc::clone(&self.responses);

            async move {
                Self::record_with(
                    requests,
                    responses,
                    match method {
                        HttpMethod::Get => "GET",
                        HttpMethod::Post => "POST",
                    },
                    request,
                )
            }
        }
    }

    impl HttpClient for OwnedOnlyChunkedHttpClient {
        type Response = OwnedOnlyChunkedResponse;

        fn send(
            &self,
            method: HttpMethod,
            _request: HttpRequest<'static>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static + use<>
        {
            let response = self.response.clone();
            let error = error::Error::Unknow("POST is not used in this test".to_string());

            async move {
                match method {
                    HttpMethod::Get => Ok(response),
                    HttpMethod::Post => Err(error),
                }
            }
        }
    }

    impl HttpClient for RecordingHttpClient {
        type Response = FakeResponse;

        fn send(
            &self,
            method: HttpMethod,
            request: HttpRequest<'static>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static + use<>
        {
            let requests = Arc::clone(&self.requests);
            let responses = Arc::clone(&self.responses);

            async move {
                Self::record_with(
                    requests,
                    responses,
                    match method {
                        HttpMethod::Get => "GET",
                        HttpMethod::Post => "POST",
                    },
                    request,
                )
            }
        }
    }

    impl BorrowHttpClient for NonSyncWriteHttpClient {
        type Response = FakeResponse;

        fn send<'a>(
            &'a self,
            method: HttpMethod,
            request: HttpRequest<'a>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'a> {
            let requests = Arc::clone(&self.requests);
            let status = self.status.get();

            async move {
                if method != HttpMethod::Post {
                    return Err(error::Error::Unknow(
                        "GET is not used in this test".to_string(),
                    ));
                }

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

    impl HttpClient for NonSyncWriteHttpClient {
        type Response = FakeResponse;

        fn send(
            &self,
            method: HttpMethod,
            request: HttpRequest<'static>,
        ) -> impl std::future::Future<Output = Result<Self::Response, error::Error>>
        + Send
        + 'static
        + use<> {
            let requests = Arc::clone(&self.requests);
            let status = self.status.get();

            async move {
                if method != HttpMethod::Post {
                    return Err(error::Error::Unknow(
                        "GET is not used in this test".to_string(),
                    ));
                }

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

    impl BorrowHttpClient for BorrowingGetHttpClient {
        type Response = FakeResponse;

        fn send<'a>(
            &'a self,
            method: HttpMethod,
            _request: HttpRequest<'a>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'a> {
            if method == HttpMethod::Get {
                self.get_calls.set(self.get_calls.get() + 1);
            }

            async move {
                match method {
                    HttpMethod::Get => Ok(FakeResponse::json(
                        200,
                        &format!(r#"{{"value":"{}"}}"#, self.select_response),
                    )),
                    HttpMethod::Post => Err(error::Error::Unknow(
                        "POST is not used in this test".to_string(),
                    )),
                }
            }
        }
    }

    impl BorrowHttpClient for BorrowingPostHttpClient {
        type Response = FakeResponse;

        fn send<'a>(
            &'a self,
            method: HttpMethod,
            _request: HttpRequest<'a>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'a> {
            if method == HttpMethod::Post {
                self.post_calls.set(self.post_calls.get() + 1);
            }

            async move {
                match method {
                    HttpMethod::Get => Err(error::Error::Unknow(
                        "GET is not used in this test".to_string(),
                    )),
                    HttpMethod::Post => Ok(FakeResponse::json(200, r#"{"results":[]}"#)),
                }
            }
        }
    }

    impl BorrowHttpClient for BorrowingWriteHttpClient {
        type Response = FakeResponse;

        fn send<'a>(
            &'a self,
            method: HttpMethod,
            request: HttpRequest<'a>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'a> {
            if method != HttpMethod::Post {
                return futures::future::Either::Left(async {
                    Err(error::Error::Unknow(
                        "GET is not used in this test".to_string(),
                    ))
                });
            }

            self.requests.set(self.requests.get() + 1);
            let response = self.response.clone();
            self.last_request.replace(Some(RecordedRequest {
                method: "POST",
                url: request.url().to_string(),
                bearer_token: request.bearer_token().map(str::to_owned),
                body: request.body().map(str::to_owned),
            }));

            futures::future::Either::Right(async move { Ok(response) })
        }
    }

    impl BorrowHttpClient for BorrowingQueryGetHttpClient {
        type Response = FakeResponse;

        fn send<'a>(
            &'a self,
            method: HttpMethod,
            _request: HttpRequest<'a>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'a> {
            if method == HttpMethod::Get {
                self.get_calls.set(self.get_calls.get() + 1);
            }

            async move {
                match method {
                    HttpMethod::Get => Ok(FakeResponse::json(
                        200,
                        r#"{"results":[{"statement_id":0,"series":null}]}"#,
                    )),
                    HttpMethod::Post => Err(error::Error::Unknow(
                        "POST is not used in this test".to_string(),
                    )),
                }
            }
        }
    }

    impl BorrowHttpClient for SplitQueryHttpClient {
        type Response = FakeResponse;

        fn send<'a>(
            &'a self,
            method: HttpMethod,
            _request: HttpRequest<'a>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'a> {
            match method {
                HttpMethod::Get => self.get_calls.set(self.get_calls.get() + 1),
                HttpMethod::Post => self.post_calls.set(self.post_calls.get() + 1),
            }

            async move {
                match method {
                    HttpMethod::Get => Ok(FakeResponse::json(
                        200,
                        &format!(
                            r#"{{"results":[{{"statement_id":0,"value":"{}"}}]}}"#,
                            self.response_body
                        ),
                    )),
                    HttpMethod::Post => Ok(FakeResponse::json(200, r#"{"results":[]}"#)),
                }
            }
        }
    }

    impl HttpClient for SplitQueryHttpClient {
        type Response = FakeResponse;

        fn send(
            &self,
            method: HttpMethod,
            _request: HttpRequest<'static>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static + use<>
        {
            let response_body = self.response_body.clone();

            async move {
                match method {
                    HttpMethod::Get => Ok(FakeResponse::json(
                        200,
                        &format!(
                            r#"{{"results":[{{"statement_id":0,"value":"{}"}}]}}"#,
                            response_body
                        ),
                    )),
                    HttpMethod::Post => Ok(FakeResponse::json(200, r#"{"results":[]}"#)),
                }
            }
        }
    }

    impl BorrowHttpResponse for NonSendResponse {
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

    impl BorrowHttpClient for NonSendResponseHttpClient {
        type Response = NonSendResponse;

        fn send<'a>(
            &'a self,
            method: HttpMethod,
            _request: HttpRequest<'a>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'a> {
            let body = self.response_body.clone();

            async move {
                match method {
                    HttpMethod::Get => Ok(NonSendResponse {
                        status: 200,
                        body: Rc::new(body),
                    }),
                    HttpMethod::Post => Err(error::Error::Unknow(
                        "POST is not used in this test".to_string(),
                    )),
                }
            }
        }
    }

    impl BorrowHttpResponse for NonSendBodyFutureResponse {
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

    impl BorrowHttpClient for NonSendBodyFutureHttpClient {
        type Response = NonSendBodyFutureResponse;

        fn send<'a>(
            &'a self,
            method: HttpMethod,
            _request: HttpRequest<'a>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'a> {
            let body = self.response_body.clone();

            async move {
                match method {
                    HttpMethod::Get => Ok(NonSendBodyFutureResponse {
                        status: 200,
                        body: Rc::new(body),
                    }),
                    HttpMethod::Post => Err(error::Error::Unknow(
                        "POST is not used in this test".to_string(),
                    )),
                }
            }
        }
    }

    impl BorrowHttpResponse for VersionHeaderResponse {
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

    impl BorrowHttpClient for VersionHeaderHttpClient {
        type Response = VersionHeaderResponse;

        fn send<'a>(
            &'a self,
            method: HttpMethod,
            _request: HttpRequest<'a>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'a> {
            let response = VersionHeaderResponse {
                status: self.response.status,
                version_header: match &self.response.version_header {
                    VersionHeader::Missing => VersionHeader::Missing,
                    VersionHeader::Valid(value) => VersionHeader::Valid(value.clone()),
                    VersionHeader::InvalidUtf8 => VersionHeader::InvalidUtf8,
                },
            };

            async move {
                match method {
                    HttpMethod::Get => Ok(response),
                    HttpMethod::Post => Err(error::Error::Unknow(
                        "POST is not used in this test".to_string(),
                    )),
                }
            }
        }
    }

    impl BorrowHttpResponse for ChunkedFakeResponse {
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

    impl BorrowChunkedHttpResponse for ChunkedFakeResponse {
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
            T: DeserializeOwned + 'static,
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
            BorrowChunkedHttpResponse::into_chunk_stream(self).await
        }
    }

    impl HttpResponse for OwnedOnlyChunkedResponse {
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
            T: DeserializeOwned + 'static,
        {
            Err(error::Error::Unknow(
                "json is not used in this test".to_string(),
            ))
        }
    }

    impl ChunkedHttpResponse for OwnedOnlyChunkedResponse {
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

    impl OwnedOnlyHttpClient {
        fn new(responses: Vec<OwnedOnlyResponse>) -> Self {
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
            responses: Arc<Mutex<Vec<OwnedOnlyResponse>>>,
            method: &'static str,
            request: HttpRequest<'static>,
        ) -> Result<OwnedOnlyResponse, error::Error> {
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

    impl OwnedOnlyChunkedHttpClient {
        fn new(response: OwnedOnlyChunkedResponse) -> Self {
            Self { response }
        }
    }

    impl BorrowHttpClient for ChunkedHttpClient {
        type Response = ChunkedFakeResponse;

        fn send<'a>(
            &'a self,
            method: HttpMethod,
            _request: HttpRequest<'a>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'a> {
            let response = self.response.clone();
            async move {
                match method {
                    HttpMethod::Get => Ok(response),
                    HttpMethod::Post => Err(error::Error::Unknow(
                        "POST is not used in this test".to_string(),
                    )),
                }
            }
        }
    }

    impl HttpClient for ChunkedHttpClient {
        type Response = ChunkedFakeResponse;

        fn send(
            &self,
            method: HttpMethod,
            _request: HttpRequest<'static>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + Send + 'static + use<>
        {
            let response = self.response.clone();
            let error = error::Error::Unknow("POST is not used in this test".to_string());
            async move {
                match method {
                    HttpMethod::Get => Ok(response),
                    HttpMethod::Post => Err(error),
                }
            }
        }
    }

    impl BorrowHttpClient for BorrowingChunkedHttpClient {
        type Response = ChunkedFakeResponse;

        fn send<'a>(
            &'a self,
            method: HttpMethod,
            _request: HttpRequest<'a>,
        ) -> impl Future<Output = Result<Self::Response, error::Error>> + use<'a> {
            let response = self.response.clone();
            async move {
                match method {
                    HttpMethod::Get => Ok(response),
                    HttpMethod::Post => Err(error::Error::Unknow(
                        "POST is not used in this test".to_string(),
                    )),
                }
            }
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
    fn ping_borrow_preserves_jwt_token() {
        let http_client = RecordingHttpClient::new(vec![FakeResponse::empty(204)]);
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            http_client,
        )
        .set_jwt_token("jwt-token");

        let ping = block_on(client.ping_borrow());

        assert!(ping);

        let requests = client.client.take_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[0].bearer_token.as_deref(), Some("jwt-token"));
        assert!(requests[0].url.contains("/ping"));
    }

    #[test]
    fn get_version_borrow_preserves_jwt_token() {
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

        let version = block_on(client.get_version_borrow());

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
    fn custom_http_client_write_points_default_to_nanosecond_precision() {
        let http_client = RecordingHttpClient::new(vec![FakeResponse::empty(204)]);
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            http_client,
        );
        let point = Point::new("cpu").add_field("value", 1).add_timestamp(42);

        block_on(client.write_point(point, None, None)).unwrap();

        let requests = client.client.take_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].body.as_deref(), Some("cpu value=1i 42\n"));
        assert!(requests[0].url.contains("/write"));
        assert!(requests[0].url.contains("precision=n"));
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
    fn write_point_future_does_not_borrow_point_fields() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            RecordingHttpClient::new(vec![FakeResponse::empty(204)]),
        );
        let host = String::from("edge-a");
        let point = Point::new("cpu")
            .add_tag("host", host.as_str())
            .add_field("value", 1)
            .add_timestamp(42);
        let future = client.write_point(point, Some(Precision::Seconds), None);

        drop(host);

        block_on(async {
            tokio::spawn(future).await.unwrap().unwrap();
        });

        let requests = client.client.take_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(
            requests[0].body.as_deref(),
            Some("cpu,host=edge-a value=1i 42\n")
        );
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
    fn owned_query_future_does_not_borrow_sql_input() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            RecordingHttpClient::new(vec![FakeResponse::json(
                200,
                r#"{"results":[{"statement_id":0,"series":null}]}"#,
            )]),
        );
        let sql = String::from("select value from cpu");
        let future = client.query(sql.as_str(), None);

        drop(sql);

        block_on(async {
            let result = tokio::spawn(future).await.unwrap().unwrap();
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
            BorrowHttpResponse::json(
                client
                    .send_request_borrow("select value from cpu", None, false)
                    .await
                    .unwrap(),
            )
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

        let query: Query = block_on(async {
            BorrowHttpResponse::json(
                client
                    .send_request_borrow("create database metrics", None, false)
                    .await
                    .unwrap(),
            )
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
    fn write_point_borrow_supports_non_spawn_http_clients() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            BorrowingWriteHttpClient::new(),
        )
        .set_jwt_token("jwt-token");
        let point = Point::new("cpu").add_field("value", 1).add_timestamp(42);

        block_on(client.write_point_borrow(point, Some(Precision::Seconds), None)).unwrap();

        assert_eq!(client.client.requests.get(), 1);
        let request = client.client.last_request.borrow().clone().unwrap();
        assert_eq!(request.method, "POST");
        assert_eq!(request.bearer_token.as_deref(), Some("jwt-token"));
        assert_eq!(request.body.as_deref(), Some("cpu value=1i 42\n"));
        assert!(request.url.contains("precision=s"));
    }

    #[test]
    fn write_points_borrow_serializes_multiple_points() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            BorrowingWriteHttpClient::new(),
        );
        let points = vec![
            Point::new("cpu").add_field("value", 1).add_timestamp(42),
            Point::new("mem").add_field("value", 2).add_timestamp(43),
        ];

        block_on(client.write_points_borrow(points, None, None)).unwrap();

        assert_eq!(client.client.requests.get(), 1);
        let request = client.client.last_request.borrow().clone().unwrap();
        assert_eq!(
            request.body.as_deref(),
            Some("cpu value=1i 42\nmem value=2i 43\n")
        );
        assert!(request.url.contains("precision=n"));
    }

    #[test]
    fn write_point_maps_database_missing_for_owned_transports() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            RecordingHttpClient::new(vec![FakeResponse::json(404, r#""missing db""#)]),
        );
        let point = Point::new("cpu").add_field("value", 1).add_timestamp(42);

        let err = block_on(client.write_point(point, Some(Precision::Seconds), None)).unwrap_err();

        assert_eq!(
            err,
            error::Error::DataBaseDoesNotExist("missing db".to_string())
        );
    }

    #[test]
    fn write_point_borrow_maps_retention_policy_missing() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            BorrowingWriteHttpClient::with_response(FakeResponse::json(500, "rp missing")),
        );
        let point = Point::new("cpu").add_field("value", 1).add_timestamp(42);

        let err =
            block_on(client.write_point_borrow(point, Some(Precision::Seconds), None)).unwrap_err();

        assert_eq!(
            err,
            error::Error::RetentionPolicyDoesNotExist("rp missing".to_string())
        );
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

        let version = block_on(client.get_version_borrow());

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

        let version = block_on(client.get_version_borrow());

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

        let version = block_on(client.get_version_borrow());

        assert_eq!(version.as_deref(), Some("1.8.10"));
    }

    #[test]
    fn owned_http_client_ping_uses_spawn_safe_transport() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            OwnedOnlyHttpClient::new(vec![OwnedOnlyResponse::empty(204)]),
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
    fn owned_http_client_get_version_uses_spawn_safe_transport() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            OwnedOnlyHttpClient::new(vec![OwnedOnlyResponse::with_header(
                204,
                "X-Influxdb-Version",
                "1.8.10",
            )]),
        );

        let version = block_on(client.get_version());

        assert_eq!(version.as_deref(), Some("1.8.10"));

        let requests = client.client.take_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
        assert!(requests[0].url.contains("/ping"));
    }

    #[test]
    fn owned_chunked_query_only_needs_owned_chunked_traits() {
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            OwnedOnlyChunkedHttpClient::new(OwnedOnlyChunkedResponse {
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
    fn owned_http_client_can_back_query_and_write_apis_without_extra_traits() {
        let http_client = OwnedOnlyHttpClient::new(vec![
            OwnedOnlyResponse::json(200, r#"{"results":[{"statement_id":0,"series":null}]}"#),
            OwnedOnlyResponse::empty(204),
        ]);
        let client = Client::new_with_client(
            Url::parse("http://localhost:8086").unwrap(),
            "metrics",
            http_client,
        );
        let point = Point::new("cpu").add_field("value", 1).add_timestamp(42);

        block_on(async {
            let result = client.query("select value from cpu", None).await.unwrap();
            assert_eq!(result.unwrap()[0].statement_id, Some(0));

            client
                .write_point(point, Some(Precision::Seconds), None)
                .await
                .unwrap();
        });

        let requests = client.client.take_requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[1].method, "POST");
        assert_eq!(requests[1].body.as_deref(), Some("cpu value=1i 42\n"));
    }
}
