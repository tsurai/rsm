use std::time::Duration;
use std::io::{BufRead, Write};
use std::net::{TcpStream, ToSocketAddrs};
use native_tls::TlsConnector;
use failure::*;
use bufstream::BufStream;
use json;

pub struct SnippetRow {
    pub id: i64,
    pub name: String,
    pub content: String,
    pub deleted: i64,
    pub last_updated: i64,
}

impl Into<json::JsonValue> for SnippetRow {
    fn into(self) -> json::JsonValue {
        object! {
            "id" => self.id,
            "name" => self.name,
            "content" => self.content,
            "deleted" => self.deleted,
            "last_updated" => self.last_updated,
        }
    }
}

pub struct TagRow {
    pub id: i64,
    pub name: String,
    pub deleted: i64,
    pub last_updated: i64,
}

impl Into<json::JsonValue> for TagRow {
    fn into(self) -> json::JsonValue {
        object! {
            "id" => self.id,
            "name" => self.name,
            "deleted" => self.deleted,
            "last_updated" => self.last_updated,
        }
    }
}

pub struct SnippetTagRow {
    pub id: i64,
    pub snippet_id: i64,
    pub tag_id: i64,
    pub deleted: i64,
    pub last_updated: i64,
}

impl Into<json::JsonValue> for SnippetTagRow {
    fn into(self) -> json::JsonValue {
        object! {
            "id" => self.id,
            "snippet_id" => self.snippet_id,
            "tag_id" => self.tag_id,
            "deleted" => self.deleted,
            "last_updated" => self.last_updated,
        }
    }
}

pub fn sync_data<T: ToSocketAddrs>(dest: T, domain: &str, last_synced: i64, data: &str) -> Result<(), Error> {
    let connector = TlsConnector::builder().danger_accept_invalid_certs(true).build()
        .context("failed to create TLS connector")?;

    let stream = TcpStream::connect(dest)
        .context("failed to connect to remote host")?;

    let duration = Duration::new(60, 0);
    stream.set_read_timeout(Some(duration))
        .context("failed to set read timeout")?;
    stream.set_write_timeout(Some(duration))
        .context("failed to set read timeout")?;

    let mut stream = BufStream::new(connector.connect(domain, stream)
        .context("failed to perform TLS handshake")?);

    let token = "dummy";
    let data_size = data.len();

    let upload_data = format!(
        "{}\n{}\n{}\n{}\n",
        token,
        last_synced,
        data_size,
        data);

    stream.write_all(upload_data.as_bytes())
        .context("failed to send data to upstream server")?;

    stream.flush()
        .context("failed to flush stream")?;

    let mut buf = String::new();
    stream.read_line(&mut buf)
        .context("server is not reponding")?;

    let response = json::parse(buf.as_str())
        .context("failed to parse server response")?;

    if let Some(err) = response["error"].as_str() {
        bail!(err.to_string())
    }

    Ok(())
}
