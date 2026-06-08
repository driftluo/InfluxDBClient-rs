#![cfg(feature = "reqwest")]

use influx_db_client::{Client, Point, Points, Precision, UdpClient, Url, point, points, reqwest};
use std::fs::File;
use std::io::Read;
use std::thread::sleep;
use std::time::Duration;

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().unwrap().block_on(f)
}

#[test]
fn create_and_delete_database() {
    block_on(async {
        let client = Client::default().set_authentication("root", "root");

        client.create_database("temporary").await.unwrap();

        client.drop_database("temporary").await.unwrap();
    });
}

#[test]
fn create_and_delete_measurement() {
    block_on(async {
        let mut client = Client::default().set_authentication("root", "root");
        client.switch_database("test_create_and_delete_measurement");
        client.create_database(client.get_db()).await.unwrap();
        let point = Point::new("temporary")
            .add_field("foo", "bar")
            .add_field("integer", 11)
            .add_field("float", 22.3)
            .add_field("'boolean'", false);

        tokio::spawn(client.write_point(point, Some(Precision::Seconds), None))
            .await
            .unwrap()
            .unwrap();

        client.drop_measurement("temporary").await.unwrap();
        client.drop_database(client.get_db()).await.unwrap();
    });
}

#[test]
fn use_points() {
    block_on(async {
        let mut client = Client::default().set_authentication("root", "root");
        client.switch_database("test_use_points");
        client.create_database(client.get_db()).await.unwrap();
        let point = Point::new("test1")
            .add_field("foo", "bar")
            .add_field("integer", 11)
            .add_field("float", 22.3)
            .add_field("'boolean'", false);

        let point1 = Point::new("test2")
            .add_tag("tags", "'=213w")
            .add_tag("number", 12)
            .add_tag("float", 12.6)
            .add_field("fd", "'3'".to_string());

        let points = Points::create_new(vec![point1, point]);

        tokio::spawn(client.write_points(points, Some(Precision::Seconds), None))
            .await
            .unwrap()
            .unwrap();

        sleep(Duration::from_secs(3));

        client.drop_measurement("test1").await.unwrap();
        client.drop_measurement("test2").await.unwrap();
        client.drop_database(client.get_db()).await.unwrap();
    });
}

#[test]
fn query() {
    block_on(async {
        let dbname = "test_query";
        let mut client = Client::default().set_authentication("root", "root");
        client.switch_database(dbname);
        client.create_database(client.get_db()).await.unwrap();
        let point = Point::new("test3").add_field("foo", "bar");
        let point1 = point.clone();
        let point = point.add_timestamp(1_508_981_970);
        let point1 = point1.add_timestamp(1_508_982_026);

        client.write_point(point, None, None).await.unwrap();
        client.query("select * from test3", None).await.unwrap();
        client.write_point(point1, None, None).await.unwrap();
        client.drop_measurement("test3").await.unwrap();
        client.drop_database(client.get_db()).await.unwrap();
    });
}

#[test]
fn use_macro() {
    block_on(async {
        let mut client = Client::default().set_authentication("root", "root");
        client.switch_database("use_macro");
        client.create_database(client.get_db()).await.unwrap();
        let point = point!("test4").add_field("foo", "bar");
        let point1 = point.clone();
        let point = point.add_timestamp(1_508_981_970);
        let point1 = point1.add_timestamp(1_508_982_026);

        let points = points![point, point1];
        client.write_points(points, None, None).await.unwrap();

        client.query("select * from test4", None).await.unwrap();

        client.drop_measurement("test4").await.unwrap();
    });
}

#[test]
fn use_udp() {
    block_on(async {
        let mut udp = UdpClient::new("127.0.0.1:8089".parse().unwrap());
        udp.add_host("127.0.0.1:8090".parse().unwrap());
        let mut client = Client::default().set_authentication("root", "root");

        client.create_database("udp").await.unwrap();
        client.create_database("telegraf").await.unwrap();

        let point = point!("test").add_field("foo", "bar");

        udp.write_point(point).unwrap();

        sleep(Duration::from_secs(1));
        client.switch_database("udp");
        client.drop_measurement("test").await.unwrap();
        client.switch_database("telegraf");
        client.drop_measurement("test").await.unwrap();
    });
}

#[test]
fn use_https() {
    use std::fs;
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::Duration;

    use tempdir::TempDir;

    fn run_openssl(args: &[&str]) {
        let output = Command::new("openssl")
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "openssl {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // https://docs.influxdata.com/influxdb/v1.5/administration/https_setup/#setup-https-with-a-self-signed-certificate
    let dir = TempDir::new("test_use_https").unwrap();
    let dir_path: String = dir.path().to_str().unwrap().to_owned();
    let ca_key_path: String = dir.path().join("test-ca.key").to_str().unwrap().to_owned();
    let ca_cert_path: String = dir.path().join("test-ca.cert").to_str().unwrap().to_owned();
    let tls_key_filename = "influxdb-selfsigned.key";
    let tls_key_path: String = dir
        .path()
        .join(tls_key_filename)
        .to_str()
        .unwrap()
        .to_owned();
    let tls_csr_path: String = dir
        .path()
        .join("influxdb-selfsigned.csr")
        .to_str()
        .unwrap()
        .to_owned();
    let tls_cert_filename = "influxdb-selfsigned.cert";
    let tls_cert_path: String = dir
        .path()
        .join(tls_cert_filename)
        .to_str()
        .unwrap()
        .to_owned();
    let tls_ext_path: String = dir
        .path()
        .join("influxdb-selfsigned.ext")
        .to_str()
        .unwrap()
        .to_owned();

    run_openssl(&[
        "req",
        "-x509",
        "-nodes",
        "-newkey",
        "rsa:2048",
        "-days",
        "10",
        "-sha256",
        "-subj",
        "/C=GB/ST=London/L=London/O=Global Security/OU=IT Department/CN=InfluxDB Test CA",
        "-addext",
        "basicConstraints=critical,CA:TRUE",
        "-addext",
        "keyUsage=critical,keyCertSign,cRLSign",
        "-addext",
        "subjectKeyIdentifier=hash",
        "-keyout",
        ca_key_path.as_str(),
        "-out",
        ca_cert_path.as_str(),
    ]);

    run_openssl(&[
        "req",
        "-nodes",
        "-newkey",
        "rsa:2048",
        "-subj",
        "/C=GB/ST=London/L=London/O=Global Security/OU=IT Department/CN=localhost",
        "-keyout",
        tls_key_path.as_str(),
        "-out",
        tls_csr_path.as_str(),
    ]);

    let mut tls_ext_file = fs::File::create(tls_ext_path.as_str()).unwrap();
    tls_ext_file
        .write_all(
            br#"basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
subjectAltName=DNS:localhost,IP:127.0.0.1
subjectKeyIdentifier=hash
authorityKeyIdentifier=keyid,issuer
"#,
        )
        .unwrap();
    drop(tls_ext_file);

    run_openssl(&[
        "x509",
        "-req",
        "-in",
        tls_csr_path.as_str(),
        "-CA",
        ca_cert_path.as_str(),
        "-CAkey",
        ca_key_path.as_str(),
        "-CAcreateserial",
        "-days",
        "10",
        "-sha256",
        "-extfile",
        tls_ext_path.as_str(),
        "-out",
        tls_cert_path.as_str(),
    ]);

    let influxdb_config_path: String = dir
        .path()
        .join("influxdb.conf")
        .to_str()
        .unwrap()
        .to_owned();

    let http_port = 9086;
    let influxdb_config_content = format!(
        r#"
bind-address = "127.0.0.1:{rpc_port}"
[meta]
  dir = "{dir}/meta"
[data]
  dir = "{dir}/data"
  wal-dir = "{dir}/wal"
[http]
  bind-address = ":{http_port}"
  https-enabled = true
  https-certificate = "{tls_cert_path}"
  https-private-key = "{tls_key_path}"
"#,
        rpc_port = 9088,
        http_port = http_port,
        dir = dir_path,
        tls_cert_path = tls_cert_path,
        tls_key_path = tls_key_path
    );
    let mut f = fs::File::create(influxdb_config_path.as_str()).unwrap();
    f.write_all(influxdb_config_content.as_bytes()).unwrap();
    f.sync_all().unwrap();
    drop(f);

    for path in [
        &ca_key_path,
        &ca_cert_path,
        &tls_key_path,
        &tls_csr_path,
        &tls_cert_path,
        &tls_ext_path,
        &influxdb_config_path,
    ]
    .iter()
    {
        assert!(fs::metadata(path).is_ok());
    }

    let mut influxdb_server = Command::new("influxd")
        .arg("-config")
        .arg(influxdb_config_path.as_str())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    while std::net::TcpStream::connect(("127.0.0.1", http_port)).is_err() {
        thread::sleep(Duration::from_millis(500));
    }

    let mut ca_cert_file = File::open(ca_cert_path.as_str()).unwrap();
    let mut ca_cert_buffer = Vec::new();
    ca_cert_file.read_to_end(&mut ca_cert_buffer).unwrap();

    let cert = reqwest::Certificate::from_pem(&ca_cert_buffer).unwrap();

    let http_client = reqwest::Client::builder()
        .add_root_certificate(cert)
        .build()
        .unwrap();

    block_on(async {
        let ping_url = format!("https://127.0.0.1:{http_port}/ping");
        let mut last_error = None;

        for _ in 0..20 {
            match http_client.get(&ping_url).send().await {
                Ok(response) if response.status() == 204 => return,
                Ok(response) => {
                    last_error = Some(format!("unexpected /ping status {}", response.status()));
                }
                Err(err) => {
                    last_error = Some(format!("{err:?}"));
                }
            }

            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        panic!(
            "https influxdb server did not become ready: {}",
            last_error.unwrap_or_else(|| "unknown error".to_string())
        );
    });

    let host = format!("https://127.0.0.1:{}", http_port);
    let client = Client::new_with_client(
        Url::parse(host.as_str()).unwrap(),
        "test_use_https",
        http_client,
    );

    block_on(async {
        client.create_database(client.get_db()).await.unwrap();

        let point = point!("foo").add_field("foo", "bar");

        client.write_point(point, None, None).await.unwrap();

        client.query("select * from foo", None).await.unwrap();

        client.drop_measurement("foo").await.unwrap();
        client.drop_database(client.get_db()).await.unwrap();
    });

    influxdb_server.kill().unwrap();
    influxdb_server.wait().unwrap();
}
