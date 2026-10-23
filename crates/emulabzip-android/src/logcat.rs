//! Parsing and streaming `logcat -v threadtime`.

use emulabzip_core::{LogEntry, LogLevel, LogOptions, Stream};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

/// The logcat command line for a stream that replays `tail` lines and then follows.
pub fn command(options: LogOptions) -> String {
    // `-T` needs a count of at least one; with zero we replay a single line, which the UI
    // treats as "start from now" anyway.
    let tail = options.tail.max(1);
    let level = if options.verbose { "*:V" } else { "*:I" };
    format!("logcat -v threadtime -T {tail} {level}")
}

/// Parses one `threadtime` line:
///
/// ```text
/// 10-23 14:02:11.123  1234  1250 I ActivityManager: Start proc 4321:com.example/u0a190
/// ```
///
/// Returns `None` for separators such as `--------- beginning of main`.
pub fn parse_line(line: &str) -> Option<LogEntry> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.len() < 20 || line.starts_with("---------") {
        return None;
    }

    let (date, rest) = token(line)?;
    let (time, rest) = token(rest)?;
    if !looks_like_date(date) || !time.contains(':') {
        return None;
    }
    let (pid, rest) = token(rest)?;
    let (tid, rest) = token(rest)?;
    let (level, rest) = token(rest)?;
    let pid = pid.parse().ok()?;
    let tid = tid.parse().ok()?;
    let mut letters = level.chars();
    let level = LogLevel::from_letter(letters.next()?).filter(|_| letters.next().is_none())?;
    let rest = rest.trim_start();

    // Tags may contain spaces and are padded before the colon, so split on the first ": ".
    let (tag, message) = match rest.split_once(": ") {
        Some((tag, message)) => (tag.trim_end(), message),
        None => (rest.strip_suffix(':').unwrap_or(rest).trim_end(), ""),
    };

    Some(LogEntry {
        seq: 0,
        time: format!("{date} {time}"),
        pid,
        tid,
        level,
        tag: tag.to_string(),
        message: message.to_string(),
        subsystem: None,
    })
}

/// Splits off the next whitespace separated token.
fn token(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }
    Some(s.split_once(char::is_whitespace).unwrap_or((s, "")))
}

fn looks_like_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 5 && b[2] == b'-' && b[..2].iter().chain(&b[3..]).all(u8::is_ascii_digit)
}

/// Turns the raw output of a running logcat into a stream of entries.
pub fn stream<R: AsyncRead + Unpin + Send + 'static>(reader: R) -> Stream<LogEntry> {
    Stream::spawn(2048, |tx| async move {
        let mut reader = BufReader::with_capacity(64 * 1024, reader);
        let mut buf = Vec::with_capacity(512);
        let mut seq = 0u64;
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            // Apps can log arbitrary bytes; never let one bad line end the stream.
            let line = String::from_utf8_lossy(&buf);
            if let Some(mut entry) = parse_line(&line) {
                seq += 1;
                entry.seq = seq;
                if tx.send(entry).await.is_err() {
                    break;
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_threadtime_line() {
        let e = parse_line("10-23 14:02:11.123  1234  1250 I ActivityManager: Start proc 4321:com.example/u0a190\n")
            .unwrap();
        assert_eq!(e.time, "10-23 14:02:11.123");
        assert_eq!((e.pid, e.tid), (1234, 1250));
        assert_eq!(e.level, LogLevel::Info);
        assert_eq!(e.tag, "ActivityManager");
        assert_eq!(e.message, "Start proc 4321:com.example/u0a190");
    }

    #[test]
    fn handles_padded_tags_and_tags_with_spaces() {
        let e = parse_line("10-23 14:02:11.123   812   812 W Some Tag  : careful: colons inside").unwrap();
        assert_eq!(e.tag, "Some Tag");
        assert_eq!(e.message, "careful: colons inside");
        assert_eq!(e.level, LogLevel::Warn);
    }

    #[test]
    fn handles_empty_messages() {
        let e = parse_line("10-23 14:02:11.123   812   812 D Tag:").unwrap();
        assert_eq!(e.tag, "Tag");
        assert_eq!(e.message, "");
    }

    #[test]
    fn maps_assert_to_fatal() {
        assert_eq!(
            parse_line("10-23 14:02:11.123     1     1 A libc    : Fatal signal 6").unwrap().level,
            LogLevel::Fatal
        );
    }

    #[test]
    fn skips_separators_and_garbage() {
        assert!(parse_line("--------- beginning of main").is_none());
        assert!(parse_line("").is_none());
        assert!(parse_line("this is not a log line at all, really").is_none());
    }

    #[test]
    fn builds_the_command() {
        assert_eq!(command(LogOptions { tail: 0, verbose: false }), "logcat -v threadtime -T 1 *:I");
        assert_eq!(command(LogOptions::default()), "logcat -v threadtime -T 500 *:V");
    }

    #[tokio::test]
    async fn streams_a_fixture() {
        let text = include_str!("../tests/fixtures/logcat-threadtime.txt");
        let mut s = stream(std::io::Cursor::new(text.as_bytes().to_vec()));
        let mut entries = Vec::new();
        while let Some(e) = s.next().await {
            entries.push(e);
        }
        assert!(entries.len() >= 10, "parsed {} entries", entries.len());
        assert!(entries.windows(2).all(|w| w[0].seq + 1 == w[1].seq));
        assert!(entries.iter().any(|e| e.level == LogLevel::Error));
    }
}
