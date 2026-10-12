use serde_json::Value;
use std::{
    io::{
        self,
        BufRead,
        BufReader,
        Read,
        Write,
    },
    process::{
        ChildStdin,
        ChildStdout,
    },
    sync::{
        Arc,
        Mutex,
    },
    time::Instant,
};

use crate::jdtls_trace::{
    trace_enabled,
    trace_warning,
    TRACE_METRICS,
};

pub(crate) fn write_lsp_message(
    stdin:
        &Arc<Mutex<ChildStdin>>,
    payload:
        &Value,
) -> Result<(), String> {
    let serialize_started =
        Instant::now();

    let bytes =
        serde_json::to_vec(
            payload,
        )
        .map_err(|e| {
            format!(
                "Failed to encode LSP JSON: {e}"
            )
        })?;

    let body_len =
        bytes.len();

    let header =
        format!(
            "Content-Length: {}\r\n\r\n",
            body_len
        );

    let lock_started =
        Instant::now();

    let mut guard =
        stdin
            .lock()
            .map_err(|_| {
                "JDTLS stdin lock was poisoned"
                    .to_string()
            })?;

    let lock_elapsed =
        lock_started.elapsed();

    if trace_enabled()
        && lock_elapsed
            .as_millis()
            > 25
    {
        trace_warning(format!(
            "LSP WRITE LOCK WAIT: elapsed_ms={} body_bytes={}",
            lock_elapsed.as_millis(),
            body_len,
        ));
    }

    let io_started =
        Instant::now();

    guard
        .write_all(
            header.as_bytes(),
        )
        .and_then(|_| {
            guard.write_all(
                &bytes,
            )
        })
        .and_then(|_| {
            guard.flush()
        })
        .map_err(|e| {
            format!(
                "Failed to write to JDTLS: {e}"
            )
        })?;

    let io_elapsed =
        io_started.elapsed();

    let total_elapsed =
        serialize_started.elapsed();

    TRACE_METRICS
        .lsp_out
        .fetch_add(
            1,
            std::sync::atomic::Ordering::Relaxed,
        );

    if trace_enabled()
        && io_elapsed
            .as_millis()
            > 25
    {
        trace_warning(format!(
            "LSP WRITE SLOW: io_ms={} total_ms={} body_bytes={}",
            io_elapsed.as_millis(),
            total_elapsed.as_millis(),
            body_len,
        ));
    }

    Ok(())
}

pub(crate) fn read_lsp_message(
    reader:
        &mut BufReader<ChildStdout>,
) -> io::Result<
    Option<(Value, usize)>,
> {
    let mut content_length:
        Option<usize> =
        None;

    loop {
        let mut line =
            String::new();

        let bytes =
            reader.read_line(
                &mut line,
            )?;

        if bytes == 0 {
            return Ok(None);
        }

        let trimmed =
            line.trim_end_matches([
                '\r',
                '\n',
            ]);

        if trimmed.is_empty() {
            break;
        }

        if let Some(value) =
            trimmed.strip_prefix(
                "Content-Length:",
            )
        {
            content_length =
                Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                "Invalid Content-Length",
                            )
                        })?,
                );
        }
    }

    let length =
        content_length.ok_or_else(|| {
            TRACE_METRICS
                .protocol_errors
                .fetch_add(
                    1,
                    std::sync::atomic::Ordering::Relaxed,
                );

            io::Error::new(
                io::ErrorKind::InvalidData,
                "Missing Content-Length header",
            )
        })?;

    let mut body =
        vec![0u8; length];

    reader.read_exact(
        &mut body,
    )?;

    let value =
        serde_json::from_slice::<Value>(
            &body,
        )
        .map_err(|e| {
            TRACE_METRICS
                .protocol_errors
                .fetch_add(
                    1,
                    std::sync::atomic::Ordering::Relaxed,
                );

            io::Error::new(
                io::ErrorKind::InvalidData,
                e.to_string(),
            )
        })?;

    Ok(Some((
        value,
        length,
    )))
}