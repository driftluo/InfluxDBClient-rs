#[macro_use]
extern crate influx_db_client;
extern crate native_tls;
extern crate tempdir;

use influx_db_client::{Client, Point, Points, Precision, TLSOption, UdpClient, Value};
use native_tls::{Certificate, TlsConnector};
use std::fs::File;
use std::io::Read;
use std::thread::sleep;
use std::time::Duration;

#[test]
fn create_and_delete_database() {
    let client = Client::default().set_authentication("root", "root");

    let _ = client.create_database("temporary").unwrap();

    let _ = client.drop_database("temporary").unwrap();
}

#[test]
fn create_and_delete_measurement() {
    let mut client = Client::default().set_authentication("root", "root");
    client.switch_database("test_create_and_delete_measurement");
    let _ = client.create_database(client.get_db().as_str()).unwrap();
    let point = Point::new("temporary")
        .add_field("foo", Value::String("bar".to_string()))
        .add_field("integer", Value::Integer(11))
        .add_field("float", Value::Float(22.3))
        .add_field("'boolean'", Value::Boolean(false))
        .to_owned();

    let _ = client
        .write_point(point, Some(Precision::Seconds), None)
        .unwrap();

    let _ = client.drop_measurement("temporary").unwrap();
    let _ = client.drop_database(client.get_db().as_str()).unwrap();
}

#[test]
fn use_points() {
    let mut client = Client::default().set_authentication("root", "root");
    client.switch_database("test_use_points");
    let _ = client.create_database(client.get_db().as_str()).unwrap();
    let point = Point::new("test1")
        .add_field("foo", Value::String("bar".to_string()))
        .add_field("integer", Value::Integer(11))
        .add_field("float", Value::Float(22.3))
        .add_field("'boolean'", Value::Boolean(false))
        .to_owned();

    let point1 = Point::new("test2")
        .add_tag("tags", Value::String(String::from("'=213w")))
        .add_tag("number", Value::Integer(12))
        .add_tag("float", Value::Float(12.6))
        .add_field("fd", Value::String("'3'".to_string()))
        .to_owned();

    let points = Points::create_new(vec![point1, point]);

    let _ = client
        .write_points(points, Some(Precision::Seconds), None)
        .unwrap();

    let _ = sleep(Duration::from_secs(3));

    let _ = client.drop_measurement("test1").unwrap();
    let _ = client.drop_measurement("test2").unwrap();
    let _ = client.drop_database(client.get_db().as_str());
}

#[test]
fn query() {
    let dbname = "test_query";
    let mut client = Client::default().set_authentication("root", "root");
    client.switch_database(dbname);
    let _ = client.create_database(client.get_db().as_str()).unwrap();
    let mut point = Point::new("test3")
        .add_field("foo", Value::String("bar".to_string()))
        .to_owned();
    let mut point1 = point.clone();
    point.add_timestamp(1508981970);
    point1.add_timestamp(1508982026);

    let _ = client.write_point(point, None, None);
    let _ = client.query("select * from test3", None).unwrap();
    let _ = client.write_point(point1, None, None).unwrap();
    let _ = client.drop_measurement("test3").unwrap();
    let _ = client.drop_database(client.get_db().as_str()).unwrap();
}

#[test]
fn use_macro() {
    let client = Client::default().set_authentication("root", "root");
    let mut point = point!("test4");
    point.add_field("foo", Value::String("bar".to_string()));
    let mut point1 = point.clone();
    point.add_timestamp(1508981970);
    point1.add_timestamp(1508982026);

    let points = points![point, point1];
    let _ = client.write_points(points, None, None);

    let _ = client.query("select * from test4", None).unwrap();

    let _ = client.drop_measurement("test4").unwrap();
}

#[test]
fn use_udp() {
    let mut udp = UdpClient::new("127.0.0.1:8089");
    udp.add_host("127.0.0.1:8090");
    let mut client = Client::default().set_authentication("root", "root");

    let mut point = point!("test");
    point.add_field("foo", Value::String(String::from("bar")));

    let _ = udp.write_point(point).unwrap();

    let _ = sleep(Duration::from_secs(1));
    client.switch_database("udp");
    let _ = client.drop_measurement("test").unwrap();
    client.switch_database("telegraf");
    let _ = client.drop_measurement("test").unwrap();
}

#[test]
fn use_https() {
    use std::fs;
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::Duration;

    use tempdir::TempDir;

    // https://docs.influxdata.com/influxdb/v1.5/administration/https_setup/#setup-https-with-a-self-signed-certificate
    let dir = TempDir::new("test_use_https").unwrap();
    let dir_path: String = dir.path().to_str().unwrap().to_owned();
    let tls_key_filename = "influxdb-selfsigned.key";
    let tls_key_path: String = dir
        .path()
        .join(tls_key_filename)
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
    let output = Command::new("openssl")
        .args(&[
            "req",
            "-x509",
            "-nodes",
            "-newkey",
            "rsa:2048",
            "-days",
            "10",
            "-subj",
            "/C=GB/ST=London/L=London/O=Global Security/OU=IT Department/CN=localhost",
            "-keyout",
            tls_key_path.as_str(),
            "-out",
            tls_cert_path.as_str(),
        ]).stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .unwrap();
    println!("openssl output: {:?}", output);

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

    for path in [&tls_key_path, &tls_cert_path, &influxdb_config_path].iter() {
        assert!(fs::metadata(path).is_ok());
    }

    let mut influxdb_server = Command::new("influxd")
        .arg("-config")
        .arg(influxdb_config_path.as_str())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    thread::sleep(Duration::from_millis(500));

    let mut ca_cert_file = File::open(tls_cert_path.as_str()).unwrap();
    let mut ca_cert_buffer = Vec::new();
    ca_cert_file.read_to_end(&mut ca_cert_buffer).unwrap();

    let mut builder = TlsConnector::builder();
    builder.add_root_certificate(Certificate::from_pem(&ca_cert_buffer).unwrap());

    let tls_connector = TLSOption::new(builder.build().unwrap());

    let host = format!("https://localhost:{}", http_port);
    let client = Client::new_with_option(host.as_str(), "test_use_https", Some(tls_connector));
    let _ = client.create_database(client.get_db().as_str()).unwrap();

    let mut point = point!("foo");
    point.add_field("foo", Value::String(String::from("bar")));

    let _ = client.write_point(point, None, None).unwrap();

    let _ = client.query("select * from foo", None).unwrap();

    let _ = client.drop_measurement("foo").unwrap();
    let _ = client.drop_database(client.get_db().as_str()).unwrap();

    influxdb_server.kill().unwrap();
}
