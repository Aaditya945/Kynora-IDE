
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    io::{
        BufRead,
        BufReader,
    },
    path::{Path, PathBuf},
    process::{
        ChildStderr,
        ChildStdin,
        ChildStdout,
    },
    sync::{
        atomic::Ordering,
        Arc,
        Mutex,
    },
    thread,
};

use tauri::{
    ipc::Channel,
    AppHandle,
    Emitter,
};

use crate::jdtls::{
    path_to_file_uri,
    server_settings,
    value_at_path,
    JdtlsDiagnostic,
    JdtlsDiagnosticsEvent,
};

use crate::jdtls_session::{
    PendingRequests,
};

use crate::jdtls_trace::{
    emit_log,
    trace_error,
    trace_json,
    trace_log,
    trace_shape,
    trace_warning,
    TRACE_METRICS,
};

use crate::jdtls_transport::{
    read_lsp_message,
    write_lsp_message,
};

const DIAGNOSTICS_EVENT: &str =
    "jdtls_diagnostics";

/*
 * ============================================================
 * SERVER REQUEST HANDLING
 * ============================================================
 */

fn respond_to_server_request(
    app:
        &AppHandle,
    stdin:
        &Arc<Mutex<ChildStdin>>,
    root:
        &Path,
    message:
        &Value,
) -> Result<bool, String> {
    let method =
        match message
            .get("method")
            .and_then(Value::as_str)
        {
            Some(value) =>
                value,

            None =>
                return Ok(false),
        };

    let id =
        match message
            .get("id")
        {
            Some(value) =>
                value.clone(),

            None =>
                return Ok(false),
        };

    trace_log(format!(
        "LSP SERVER REQUEST: method={} id={} payload={}",
        method,
        id,
        trace_json(message),
    ));

    let root_uri =
        path_to_file_uri(
            root,
        );

    let result =
        match method {
            "workspace/configuration" => {
                let items =
                    message
                        .get("params")
                        .and_then(|v| {
                            v.get("items")
                        })
                        .and_then(
                            Value::as_array,
                        )
                        .cloned()
                        .unwrap_or_default();

                let settings =
                    server_settings(
                        None,
                    );

                Value::Array(
                    items
                        .iter()
                        .map(
                            |item| {
                                let section =
                                    item
                                        .get(
                                            "section",
                                        )
                                        .and_then(
                                            Value::as_str,
                                        )
                                        .unwrap_or(
                                            "",
                                        );

                                value_at_path(
                                    &settings,
                                    section,
                                )
                            },
                        )
                        .collect(),
                )
            }

            "workspace/workspaceFolders" => {
                json!([
                    {
                        "uri":
                            root_uri,

                        "name":
                            root
                                .file_name()
                                .and_then(
                                    |v|
                                        v.to_str(),
                                )
                                .unwrap_or(
                                    "Kynora Workspace",
                                )
                    }
                ])
            }

            "client/registerCapability"
            | "client/unregisterCapability" => {
                Value::Null
            }

            "window/workDoneProgress/create" => {
                Value::Null
            }

            "workspace/applyEdit" => {
                json!({
                    "applied": false,

                    "failureReason":
                        "Kynora workspace edits are not enabled in this first JDTLS milestone."
                })
            }

            "window/showMessageRequest" => {
                Value::Null
            }

            _ => {
                emit_log(
                    app,
                    format!(
                        "Unhandled JDTLS server request: {method}"
                    ),
                );

                Value::Null
            }
        };

    let response =
        json!({
            "jsonrpc":
                "2.0",

            "id":
                id,

            "result":
                result,
        });

    write_lsp_message(
        stdin,
        &response,
    )?;

    trace_log(format!(
        "LSP SERVER RESPONSE SENT: method={} id={}",
        method,
        id,
    ));

    Ok(true)
}

/*
 * ============================================================
 * STDERR READER
 * ============================================================
 */

pub(crate) fn spawn_stderr_reader(
    app:
        AppHandle,
    stderr:
        ChildStderr,
) {
    thread::spawn(
        move || {
            let mut reader =
                BufReader::new(
                    stderr,
                );

            let mut buffer =
                String::new();

            loop {
                buffer.clear();

                match reader.read_line(
                    &mut buffer,
                ) {
                    Ok(0) =>
                        break,

                    Ok(_) => {
                        let line =
                            buffer
                                .trim_end()
                                .to_string();

                        if !line.is_empty() {
                            emit_log(
                                &app,
                                format!(
                                    "JDTLS stderr: {line}"
                                ),
                            );
                        }
                    }

                    Err(error) => {
                        trace_error(
                            format!(
                                "JDTLS stderr reader error: {error}"
                            ),
                        );

                        break;
                    }
                }
            }
        },
    );
}

/*
 * ============================================================
 * STDOUT / LSP READER
 * ============================================================
 */

pub(crate) fn spawn_stdout_reader(
    app:
        AppHandle,
    stdout:
        ChildStdout,
    stdin:
        Arc<Mutex<ChildStdin>>,
    pending:
        Arc<Mutex<PendingRequests>>,
    cancelled_requests:
        Arc<Mutex<HashSet<u64>>>,
    root:
        PathBuf,
    diagnostics_channel:
        Arc<
            Mutex<
                Option<
                    Channel<
                        JdtlsDiagnosticsEvent,
                    >,
                >,
            >,
        >,
) {
    thread::spawn(
        move || {
            let mut reader =
                BufReader::new(
                    stdout,
                );

            loop {
                trace_log(
                    "LSP READER: waiting for next Content-Length frame"
                        .to_string(),
                );

                let (
                    message,
                    body_len,
                ) =
                    match read_lsp_message(
                        &mut reader,
                    ) {
                        Ok(Some(pair)) =>
                            pair,

                        Ok(None) => {
                            emit_log(
                                &app,
                                "LSP READER: EOF from JDTLS stdout"
                                    .to_string(),
                            );

                            break;
                        }

                        Err(error) => {
                            trace_error(
                                format!(
                                    "LSP READER: PARSE ERROR: {error}"
                                ),
                            );

                            break;
                        }
                    };

                TRACE_METRICS
                    .lsp_in
                    .fetch_add(
                        1,
                        Ordering::Relaxed,
                    );

                trace_log(format!(
                    "LSP IN: body_bytes={} {} payload={}",
                    body_len,
                    trace_shape(
                        &message,
                    ),
                    trace_json(
                        &message,
                    ),
                ));

                if let Some(method) =
                    message
                        .get("method")
                        .and_then(
                            Value::as_str,
                        )
                {
                    if method ==
                        "textDocument/publishDiagnostics"
                    {
                        TRACE_METRICS
                            .diagnostics
                            .fetch_add(
                                1,
                                Ordering::Relaxed,
                            );

                        if let Some(params) =
                            message.get(
                                "params",
                            )
                        {
                            let uri =
                                params
                                    .get(
                                        "uri",
                                    )
                                    .and_then(
                                        Value::as_str,
                                    )
                                    .unwrap_or_default()
                                    .to_string();

                            let diagnostics =
                                params
                                    .get(
                                        "diagnostics",
                                    )
                                    .and_then(
                                        Value::as_array,
                                    )
                                    .cloned()
                                    .unwrap_or_default()
                                    .into_iter()
                                    .filter_map(
                                        |item| {
                                            serde_json::from_value::<
                                                JdtlsDiagnostic,
                                            >(
                                                item,
                                            )
                                            .ok()
                                        },
                                    )
                                    .collect::<Vec<_>>();

                            let diagnostic_count =
                                diagnostics.len();

                            let mut errors =
                                0usize;

                            let mut warnings =
                                0usize;

                            let mut infos =
                                0usize;

                            let mut hints =
                                0usize;

                            for diagnostic
                                in &diagnostics
                            {
                                match diagnostic
                                    .severity
                                {
                                    Some(1) =>
                                        errors += 1,

                                    Some(2) =>
                                        warnings += 1,

                                    Some(3) =>
                                        infos += 1,

                                    Some(4) =>
                                        hints += 1,

                                    _ => {}
                                }
                            }

                            trace_log(format!(
                                "DIAGNOSTICS RECEIVED: uri={} count={} errors={} warnings={} infos={} hints={}",
                                uri,
                                diagnostic_count,
                                errors,
                                warnings,
                                infos,
                                hints,
                            ));

                            let event =
                                JdtlsDiagnosticsEvent {
                                    uri,
                                    diagnostics,
                                };

                            let event_for_channel =
                                event.clone();

                            if let Ok(
                                guard,
                            ) =
                                diagnostics_channel
                                    .lock()
                            {
                                if let Some(
                                    channel,
                                ) =
                                    guard.as_ref()
                                {
                                    match channel
                                        .send(
                                            event_for_channel,
                                        )
                                    {
                                        Ok(()) =>
                                            emit_log(
                                                &app,
                                                format!(
                                                    "JDTLS DIAGNOSTICS CHANNEL: delivered to frontend count={diagnostic_count}"
                                                ),
                                            ),

                                        Err(
                                            error,
                                        ) =>
                                            emit_log(
                                                &app,
                                                format!(
                                                    "JDTLS DIAGNOSTICS CHANNEL: FAILED to deliver to frontend: {error}"
                                                ),
                                            ),
                                    }
                                } else {
                                    emit_log(
                                        &app,
                                        "JDTLS DIAGNOSTICS CHANNEL: no active frontend channel"
                                            .to_string(),
                                    );
                                }
                            } else {
                                emit_log(
                                    &app,
                                    "JDTLS DIAGNOSTICS CHANNEL: lock failed"
                                        .to_string(),
                                );
                            }

                            if std::env::var_os(
                                "KYNORA_JDTLS_LEGACY_EVENT",
                            )
                            .map(
                                |value| {
                                    value
                                        == "1"
                                },
                            )
                            .unwrap_or(
                                false,
                            )
                            {
                                let _ =
                                    app.emit(
                                        DIAGNOSTICS_EVENT,
                                        event,
                                    );
                            }
                        }

                        continue;
                    }

                    if method ==
                        "window/logMessage"
                        || method ==
                        "window/showMessage"
                    {
                        if let Some(
                            message_text,
                        ) =
                            message
                                .get(
                                    "params",
                                )
                                .and_then(
                                    |v| {
                                        v.get(
                                            "message",
                                        )
                                    },
                                )
                                .and_then(
                                    Value::as_str,
                                )
                        {
                            emit_log(
                                &app,
                                format!(
                                    "JDTLS SERVER MESSAGE: {message_text}"
                                ),
                            );
                        }

                        continue;
                    }

                    if method ==
                        "$/progress"
                        || method ==
                        "language/status"
                        || method ==
                        "telemetry/event"
                    {
                        continue;
                    }

                    if message
                        .get("id")
                        .is_some()
                    {
                        let _ =
                            respond_to_server_request(
                                &app,
                                &stdin,
                                &root,
                                &message,
                            );

                        continue;
                    }
                }

                if let Some(id) =
                    message
                        .get("id")
                        .and_then(
                            Value::as_u64,
                        )
                {
                    let pending_request =
                        pending
                            .lock()
                            .ok()
                            .and_then(
                                |mut map| {
                                    map.remove(
                                        &id,
                                    )
                                },
                            );

                    if let Some(
                        request,
                    ) =
                        pending_request
                    {
                        let elapsed =
                            request
                                .started_at
                                .elapsed();

                        if let Ok(
                            mut cancelled,
                        ) =
                            cancelled_requests
                                .lock()
                        {
                            cancelled.remove(
                                &id,
                            );
                        }

                        if message
                            .get("error")
                            .is_some()
                        {
                            trace_warning(
                                format!(
                                    "LSP RESPONSE ERROR: id={} method={} elapsed_ms={} error={}",
                                    id,
                                    request.method,
                                    elapsed.as_millis(),
                                    trace_json(
                                        message
                                            .get(
                                                "error",
                                            )
                                            .unwrap_or(
                                                &Value::Null,
                                            ),
                                    ),
                                ),
                            );
                        } else {
                            trace_log(
                                format!(
                                    "LSP RESPONSE: id={} method={} elapsed_ms={} status=success",
                                    id,
                                    request.method,
                                    elapsed.as_millis(),
                                ),
                            );
                        }

                        if request
                            .method
                            == "textDocument/completion"
                        {
                            TRACE_METRICS
                                .completion_responses
                                .fetch_add(
                                    1,
                                    Ordering::Relaxed,
                                );

                            let item_count =
                                message
                                    .get(
                                        "result",
                                    )
                                    .and_then(
                                        |result| {
                                            if let Some(
                                                items,
                                            ) =
                                                result
                                                    .as_array()
                                            {
                                                Some(
                                                    items
                                                        .len(),
                                                )
                                            } else {
                                                result
                                                    .get(
                                                        "items",
                                                    )
                                                    .and_then(
                                                        Value::as_array,
                                                    )
                                                    .map(
                                                        |items| {
                                                            items
                                                                .len()
                                                        },
                                                    )
                                            }
                                        },
                                    )
                                    .unwrap_or(
                                        0,
                                    );

                            let incomplete =
                                message
                                    .get(
                                        "result",
                                    )
                                    .and_then(
                                        |result| {
                                            result
                                                .get(
                                                    "isIncomplete",
                                                )
                                        },
                                    )
                                    .and_then(
                                        Value::as_bool,
                                    )
                                    .unwrap_or(
                                        false,
                                    );

                            trace_log(
                                format!(
                                    "COMPLETION RESPONSE META: id={} items={} incomplete={} elapsed_ms={}",
                                    id,
                                    item_count,
                                    incomplete,
                                    elapsed
                                        .as_millis(),
                                ),
                            );
                        }

                        let _ =
                            request
                                .sender
                                .send(
                                    message,
                                );
                    } else {
                        TRACE_METRICS
                            .unmatched_responses
                            .fetch_add(
                                1,
                                Ordering::Relaxed,
                            );

                        trace_warning(
                            format!(
                                "LSP RESPONSE UNMATCHED: id={} payload={}",
                                id,
                                trace_json(
                                    &message,
                                ),
                            ),
                        );
                    }
                }
            }
        },
    );
}
