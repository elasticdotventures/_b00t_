//! Minimal RESP2 wire protocol — just enough to be a drop-in for the
//! `redis` crate's client (aio + tokio-comp + connection-manager features).
//! Hand-rolled rather than pulling in a RESP crate: the protocol is small,
//! and a b00t-native component running on internet-facing hive nodes
//! benefits from an auditable, dependency-light implementation.
//!
//! Reference: https://redis.io/docs/latest/develop/reference/protocol-spec/

use std::io;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

#[derive(Debug, Clone, PartialEq)]
pub enum RespValue {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Option<Vec<u8>>),
    Array(Option<Vec<RespValue>>),
}

impl RespValue {
    pub fn ok() -> Self {
        RespValue::SimpleString("OK".to_string())
    }

    pub fn nil() -> Self {
        RespValue::BulkString(None)
    }

    pub fn bulk(s: impl Into<Vec<u8>>) -> Self {
        RespValue::BulkString(Some(s.into()))
    }

    pub fn encode(&self, out: &mut Vec<u8>) {
        match self {
            RespValue::SimpleString(s) => {
                out.push(b'+');
                out.extend_from_slice(s.as_bytes());
                out.extend_from_slice(b"\r\n");
            }
            RespValue::Error(e) => {
                out.push(b'-');
                out.extend_from_slice(e.as_bytes());
                out.extend_from_slice(b"\r\n");
            }
            RespValue::Integer(i) => {
                out.push(b':');
                out.extend_from_slice(i.to_string().as_bytes());
                out.extend_from_slice(b"\r\n");
            }
            RespValue::BulkString(None) => {
                out.extend_from_slice(b"$-1\r\n");
            }
            RespValue::BulkString(Some(b)) => {
                out.push(b'$');
                out.extend_from_slice(b.len().to_string().as_bytes());
                out.extend_from_slice(b"\r\n");
                out.extend_from_slice(b);
                out.extend_from_slice(b"\r\n");
            }
            RespValue::Array(None) => {
                out.extend_from_slice(b"*-1\r\n");
            }
            RespValue::Array(Some(items)) => {
                out.push(b'*');
                out.extend_from_slice(items.len().to_string().as_bytes());
                out.extend_from_slice(b"\r\n");
                for item in items {
                    item.encode(out);
                }
            }
        }
    }
}

/// Read one client request off the wire. Real clients (including the
/// `redis` crate) always send requests as a multibulk array of bulk
/// strings (`*<n>\r\n$<len>\r\n<bytes>\r\n...`) — the legacy "inline
/// command" format (plain text line, no `*`/`$` framing) is not supported,
/// matching what every RESP2 client library actually emits.
pub async fn read_command<R: AsyncBufReadExt + Unpin>(
    reader: &mut R,
) -> io::Result<Option<Vec<Vec<u8>>>> {
    let mut line = String::new();
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        return Ok(None); // clean EOF
    }
    let line = line.trim_end_matches(['\r', '\n']);
    let Some(count_str) = line.strip_prefix('*') else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("expected multibulk array ('*'), got: {line:?}"),
        ));
    };
    let count: i64 = count_str
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad multibulk count"))?;
    if count <= 0 {
        return Ok(Some(Vec::new()));
    }

    let mut args = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let mut len_line = String::new();
        reader.read_line(&mut len_line).await?;
        let len_line = len_line.trim_end_matches(['\r', '\n']);
        let Some(len_str) = len_line.strip_prefix('$') else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("expected bulk string ('$'), got: {len_line:?}"),
            ));
        };
        let len: i64 = len_str
            .parse()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad bulk length"))?;
        if len < 0 {
            args.push(Vec::new());
            continue;
        }
        let mut buf = vec![0u8; len as usize + 2]; // +2 for trailing \r\n
        reader.read_exact(&mut buf).await?;
        buf.truncate(len as usize);
        args.push(buf);
    }
    Ok(Some(args))
}

pub async fn write_reply<W: AsyncWriteExt + Unpin>(writer: &mut W, value: &RespValue) -> io::Result<()> {
    let mut buf = Vec::new();
    value.encode(&mut buf);
    writer.write_all(&buf).await?;
    writer.flush().await
}

pub type Reader = BufReader<tokio::net::tcp::OwnedReadHalf>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn parses_multibulk_command() {
        let input = b"*2\r\n$3\r\nGET\r\n$3\r\nfoo\r\n".to_vec();
        let mut reader = BufReader::new(&input[..]);
        let cmd = read_command(&mut reader).await.unwrap().unwrap();
        assert_eq!(cmd, vec![b"GET".to_vec(), b"foo".to_vec()]);
    }

    #[tokio::test]
    async fn returns_none_on_clean_eof() {
        let input: Vec<u8> = Vec::new();
        let mut reader = BufReader::new(&input[..]);
        assert_eq!(read_command(&mut reader).await.unwrap(), None);
    }

    #[tokio::test]
    async fn rejects_inline_command_format() {
        let input = b"PING\r\n".to_vec();
        let mut reader = BufReader::new(&input[..]);
        assert!(read_command(&mut reader).await.is_err());
    }

    #[test]
    fn encodes_all_reply_types() {
        let mut buf = Vec::new();
        RespValue::ok().encode(&mut buf);
        assert_eq!(buf, b"+OK\r\n");

        let mut buf = Vec::new();
        RespValue::Error("ERR bad".into()).encode(&mut buf);
        assert_eq!(buf, b"-ERR bad\r\n");

        let mut buf = Vec::new();
        RespValue::Integer(42).encode(&mut buf);
        assert_eq!(buf, b":42\r\n");

        let mut buf = Vec::new();
        RespValue::nil().encode(&mut buf);
        assert_eq!(buf, b"$-1\r\n");

        let mut buf = Vec::new();
        RespValue::bulk("hi").encode(&mut buf);
        assert_eq!(buf, b"$2\r\nhi\r\n");

        let mut buf = Vec::new();
        RespValue::Array(Some(vec![RespValue::bulk("a"), RespValue::Integer(1)])).encode(&mut buf);
        assert_eq!(buf, b"*2\r\n$1\r\na\r\n:1\r\n");
    }
}
