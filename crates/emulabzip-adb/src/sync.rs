//! The file sync sub-protocol (`sync:`), used to list, stat and pull files.
//!
//! After `sync:` every message is a four byte id (`LIST`, `STAT`, `RECV`, …) and a little-endian
//! `u32`, which is either a length or a value depending on the message.

use emulabzip_core::{Error, Result};
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// The largest chunk the device sends in one `DATA` message.
const MAX_CHUNK: usize = 64 * 1024;

/// A file or directory on the device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteEntry {
    pub name: String,
    /// Unix mode bits, including the file type.
    pub mode: u32,
    pub size: u64,
    /// Modification time in seconds since the Unix epoch.
    pub mtime: u32,
}

impl RemoteEntry {
    pub fn is_dir(&self) -> bool {
        self.mode & 0o170000 == 0o040000
    }

    pub fn is_symlink(&self) -> bool {
        self.mode & 0o170000 == 0o120000
    }
}

/// `STAT` result. A mode of zero means the path does not exist or cannot be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteStat {
    pub mode: u32,
    pub size: u32,
    pub mtime: u32,
}

impl RemoteStat {
    pub fn exists(&self) -> bool {
        self.mode != 0
    }
}

async fn send_request<S: AsyncWrite + Unpin>(io: &mut S, id: &[u8; 4], path: &str) -> Result<()> {
    let len = u32::try_from(path.len()).map_err(|_| Error::invalid("path", path))?;
    let mut msg = Vec::with_capacity(8 + path.len());
    msg.extend_from_slice(id);
    msg.extend_from_slice(&len.to_le_bytes());
    msg.extend_from_slice(path.as_bytes());
    io.write_all(&msg).await?;
    Ok(())
}

async fn read_id<S: AsyncRead + Unpin>(io: &mut S) -> Result<[u8; 4]> {
    let mut id = [0u8; 4];
    io.read_exact(&mut id).await?;
    Ok(id)
}

async fn read_u32<S: AsyncRead + Unpin>(io: &mut S) -> Result<u32> {
    let mut b = [0u8; 4];
    io.read_exact(&mut b).await?;
    Ok(u32::from_le_bytes(b))
}

async fn read_string<S: AsyncRead + Unpin>(io: &mut S, len: u32) -> Result<String> {
    let mut buf = vec![0u8; len as usize];
    io.read_exact(&mut buf).await?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

async fn fail<S: AsyncRead + Unpin>(io: &mut S) -> Error {
    match read_u32(io).await {
        Ok(len) => match read_string(io, len.min(4096)).await {
            Ok(msg) => Error::Adb(msg),
            Err(e) => e,
        },
        Err(e) => e,
    }
}

/// Lists a directory. `.` and `..` are left out.
pub async fn list<S: AsyncRead + AsyncWrite + Unpin>(io: &mut S, path: &str) -> Result<Vec<RemoteEntry>> {
    send_request(io, b"LIST", path).await?;
    let mut entries = Vec::new();
    loop {
        match &read_id(io).await? {
            b"DENT" => {
                let mode = read_u32(io).await?;
                let size = read_u32(io).await?;
                let mtime = read_u32(io).await?;
                let name_len = read_u32(io).await?;
                let name = read_string(io, name_len).await?;
                if name != "." && name != ".." {
                    entries.push(RemoteEntry { name, mode, size: u64::from(size), mtime });
                }
            }
            b"DONE" => {
                // DONE carries the same four fields as DENT, all zero.
                let mut rest = [0u8; 16];
                io.read_exact(&mut rest).await?;
                return Ok(entries);
            }
            b"FAIL" => return Err(fail(io).await),
            other => return Err(Error::Adb(format!("unexpected sync reply `{}`", String::from_utf8_lossy(other)))),
        }
    }
}

pub async fn stat<S: AsyncRead + AsyncWrite + Unpin>(io: &mut S, path: &str) -> Result<RemoteStat> {
    send_request(io, b"STAT", path).await?;
    match &read_id(io).await? {
        b"STAT" => Ok(RemoteStat { mode: read_u32(io).await?, size: read_u32(io).await?, mtime: read_u32(io).await? }),
        b"FAIL" => Err(fail(io).await),
        other => Err(Error::Adb(format!("unexpected sync reply `{}`", String::from_utf8_lossy(other)))),
    }
}

/// Copies a remote file into `out`, returning the number of bytes written.
pub async fn pull<S, W>(io: &mut S, path: &str, out: &mut W) -> Result<u64>
where
    S: AsyncRead + AsyncWrite + Unpin,
    W: AsyncWrite + Unpin,
{
    send_request(io, b"RECV", path).await?;
    let mut total = 0u64;
    let mut buf = vec![0u8; MAX_CHUNK];
    loop {
        match &read_id(io).await? {
            b"DATA" => {
                let len = read_u32(io).await? as usize;
                if len > MAX_CHUNK {
                    return Err(Error::Adb(format!("sync chunk of {len} bytes is too large")));
                }
                io.read_exact(&mut buf[..len]).await?;
                out.write_all(&buf[..len]).await?;
                total += len as u64;
            }
            b"DONE" => {
                read_u32(io).await?;
                out.flush().await?;
                return Ok(total);
            }
            b"FAIL" => return Err(fail(io).await),
            other => return Err(Error::Adb(format!("unexpected sync reply `{}`", String::from_utf8_lossy(other)))),
        }
    }
}

/// Ends the sync session politely.
pub async fn quit<S: AsyncWrite + Unpin>(io: &mut S) -> Result<()> {
    io.write_all(b"QUIT\0\0\0\0").await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dent(name: &str, mode: u32, size: u32) -> Vec<u8> {
        let mut v = b"DENT".to_vec();
        for n in [mode, size, 1_700_000_000, name.len() as u32] {
            v.extend_from_slice(&n.to_le_bytes());
        }
        v.extend_from_slice(name.as_bytes());
        v
    }

    #[tokio::test]
    async fn lists_a_directory() {
        let (mut client, mut server) = tokio::io::duplex(4096);
        let mut reply = Vec::new();
        reply.extend(dent(".", 0o040755, 0));
        reply.extend(dent("Download", 0o040770, 4096));
        reply.extend(dent("notes.txt", 0o100660, 12));
        reply.extend(b"DONE");
        reply.extend([0u8; 16]);
        server.write_all(&reply).await.unwrap();

        let entries = list(&mut client, "/sdcard").await.unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].is_dir());
        assert_eq!(entries[1].name, "notes.txt");
        assert_eq!(entries[1].size, 12);

        let mut req = [0u8; 15];
        server.read_exact(&mut req).await.unwrap();
        assert_eq!(&req, b"LIST\x07\0\0\0/sdcard");
    }

    #[tokio::test]
    async fn pulls_a_file_in_chunks() {
        let (mut client, mut server) = tokio::io::duplex(4096);
        let mut reply = Vec::new();
        for chunk in [&b"hello "[..], &b"world"[..]] {
            reply.extend(b"DATA");
            reply.extend((chunk.len() as u32).to_le_bytes());
            reply.extend(chunk);
        }
        reply.extend(b"DONE\0\0\0\0");
        server.write_all(&reply).await.unwrap();

        let mut out = Vec::new();
        let n = pull(&mut client, "/sdcard/a.txt", &mut out).await.unwrap();
        assert_eq!(n, 11);
        assert_eq!(out, b"hello world");
    }

    #[tokio::test]
    async fn reports_failures() {
        let (mut client, mut server) = tokio::io::duplex(4096);
        let msg = b"No such file or directory";
        let mut reply = b"FAIL".to_vec();
        reply.extend((msg.len() as u32).to_le_bytes());
        reply.extend(msg);
        server.write_all(&reply).await.unwrap();

        let err = pull(&mut client, "/nope", &mut Vec::new()).await.unwrap_err();
        assert_eq!(err.to_string(), "adb: No such file or directory");
    }

    #[test]
    fn decodes_file_types() {
        let e = |mode| RemoteEntry { name: "x".into(), mode, size: 0, mtime: 0 };
        assert!(e(0o040755).is_dir());
        assert!(e(0o120777).is_symlink());
        assert!(!e(0o100644).is_dir());
    }
}
