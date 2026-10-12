use serde::{Deserialize, Serialize};

use serde_json::{json, Value};

use std::{
    collections::{
        HashMap,
        HashSet,
    },
    path::Path,
    process::ChildStdin,
    sync::{
        mpsc,
        Arc,
        Mutex,
    },
    time::{
        Duration,
        Instant,
    },
};

use tauri::{
    ipc::Channel,
    AppHandle,
    State,
};

use crate::jdtls_io::{
    spawn_stderr_reader,
    spawn_stdout_reader,
};

use crate::jdtls_process::{
    JdtlsProcess,
};

pub(crate) use crate::jdtls_session::JdtlsState;

use crate::jdtls_session::{
    JdtlsSession,
    PendingRequest,
    PendingRequests,
};

use crate::jdtls_trace::{
    trace_config,
    trace_enabled,
    trace_error,
    trace_json,
    trace_log,
    trace_summary,
    trace_warning,
    TRACE_METRICS,
};

use crate::jdtls_transport::{
    write_lsp_message,
};

/**
 * ============================================================
 * LSP DATA TYPES
 * ============================================================
 */

#[derive(Debug, Clone, Serialize)]
pub struct JdtlsStartResult {
    pub started: bool,
    pub document_uri: String,
    pub workspace_root: String,
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JdtlsPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JdtlsRange {
    pub start: JdtlsPosition,
    pub end: JdtlsPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JdtlsContentChange {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<JdtlsRange>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JdtlsDiagnostic {
    pub range: JdtlsRange,
    pub severity: Option<u8>,
    pub message: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JdtlsDiagnosticsEvent {
    pub uri: String,
    pub diagnostics: Vec<JdtlsDiagnostic>,
}

/**
 * ============================================================
 * PATH / URI
 * ============================================================
 */

pub(crate) fn path_to_file_uri(
    path: &Path,
) -> String {
    let mut raw =
        path.to_string_lossy()
            .replace('\\', "/");

    if let Some(rest) =
        raw.strip_prefix("//?/UNC/")
    {
        raw =
            format!("//{rest}");
    } else if let Some(rest) =
        raw.strip_prefix("//?/")
    {
        raw =
            rest.to_string();
    }

    let mut out =
        String::from("file:///");

    for byte in raw.as_bytes() {
        match *byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'.'
            | b'_'
            | b'~'
            | b'/'
            | b':' => {
                out.push(
                    *byte as char,
                );
            }
            other => {
                out.push('%');

                out.push(
                    hex(
                        (other >> 4)
                            & 0x0F,
                    ),
                );

                out.push(
                    hex(
                        other
                            & 0x0F,
                    ),
                );
            }
        }
    }

    out
}

fn hex(
    value: u8,
) -> char {
    match value {
        0..=9 =>
            (b'0' + value)
                as char,

        _ =>
            (b'A'
                + value
                - 10)
                as char,
    }
}

/**
 * ============================================================
 * JAVA / SERVER SETTINGS
 * ============================================================
 */

fn jdk_home_from_java(
    java: &Path,
) -> &Path {
    java.parent()
        .and_then(Path::parent)
        .unwrap_or(java)
}

pub(crate) fn server_settings(
    java_home: Option<&Path>,
) -> Value {
    let mut java =
        json!({
            "autobuild": {
                "enabled": false
            },
            "configuration": {
                "updateBuildConfiguration":
                    "automatic"
            },
            "completion": {
                "guessMethodArguments":
                    true
            },
            "format": {
                "enabled": true
            }
        });

    if let Some(home) =
        java_home
    {
        java["home"] =
            Value::String(
                home.to_string_lossy()
                    .to_string(),
            );

        java["configuration"]
            ["runtimes"] =
            json!([
                {
                    "name":
                        "JavaSE-21",
                    "path":
                        home
                            .to_string_lossy()
                            .to_string(),
                    "default":
                        true
                }
            ]);
    }

    json!({
        "java": java
    })
}

pub(crate) fn value_at_path(
    root: &Value,
    section: &str,
) -> Value {
    if section.is_empty() {
        return root.clone();
    }

    let mut current =
        root;

    for piece in
        section.split('.')
    {
        match current.get(piece) {
            Some(next) => {
                current = next;
            }

            None => {
                return Value::Null;
            }
        }
    }

    current.clone()
}

/**
 * ============================================================
 * REQUEST / RESPONSE CORRELATION
 * ============================================================
 */

pub(crate) fn send_request_and_wait(
    stdin: &Arc<Mutex<ChildStdin>>,
    pending: &Arc<Mutex<PendingRequests>>,
    cancelled_requests: &Arc<Mutex<HashSet<u64>>>,
    id: u64,
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value, String> {
    let started_at =
        Instant::now();

    let (sender, receiver) =
        mpsc::channel();

    {
        let mut pending_guard =
            pending
                .lock()
                .map_err(|_| {
                    "JDTLS pending request lock was poisoned"
                        .to_string()
                })?;

        if pending_guard
            .contains_key(&id)
        {
            TRACE_METRICS
                .duplicate_request_ids
                .fetch_add(
                    1,
                    std::sync::atomic::Ordering::Relaxed,
                );

            trace_error(
                format!(
                    "REQUEST ID COLLISION: id={} method={}",
                    id,
                    method,
                ),
            );

            return Err(format!(
                "Duplicate JDTLS request ID: {id}"
            ));
        }

        pending_guard.insert(
            id,
            PendingRequest {
                sender,
                method:
                    method.to_string(),
                started_at,
            },
        );
    }

    if let Ok(
        mut cancelled,
    ) =
        cancelled_requests.lock()
    {
        cancelled.remove(&id);
    }

    let message =
        json!({
            "jsonrpc":
                "2.0",
            "id":
                id,
            "method":
                method,
            "params":
                params,
        });

    let request_bytes =
        serde_json::to_vec(
            &message,
        )
        .map(|body| {
            body.len()
        })
        .unwrap_or(0);

    trace_log(format!(
        "LSP OUT: request id={} method={} body_bytes={} payload={}",
        id,
        method,
        request_bytes,
        trace_json(&message),
    ));

    if let Err(error) =
        write_lsp_message(
            stdin,
            &message,
        )
    {
        let _ =
            pending
                .lock()
                .map(
                    |mut map| {
                        map.remove(&id)
                    },
                );

        if let Ok(
            mut cancelled,
        ) =
            cancelled_requests
                .lock()
        {
            cancelled.remove(&id);
        }

        trace_error(format!(
            "LSP OUT WRITE FAILED: id={} method={} error={}",
            id,
            method,
            error,
        ));

        return Err(error);
    }

    trace_log(format!(
        "LSP WAIT: id={} method={} timeout_ms={}",
        id,
        method,
        timeout.as_millis(),
    ));

    match receiver.recv_timeout(
        timeout,
    ) {
        Ok(response) => {
            let elapsed =
                started_at.elapsed();

            trace_log(format!(
                "LSP RESPONSE DELIVERED: id={} method={} elapsed_ms={}",
                id,
                method,
                elapsed.as_millis(),
            ));

            Ok(response)
        }

        Err(error) => {
            TRACE_METRICS
                .timeouts
                .fetch_add(
                    1,
                    std::sync::atomic::Ordering::Relaxed,
                );

            let _ =
                pending
                    .lock()
                    .map(
                        |mut map| {
                            map.remove(&id)
                        },
                    );

            if let Ok(
                mut cancelled,
            ) =
                cancelled_requests
                    .lock()
            {
                cancelled.remove(&id);
            }

            let pending_ids =
                pending
                    .lock()
                    .ok()
                    .map(
                        |map| {
                            map.keys()
                                .map(
                                    ToString::to_string,
                                )
                                .collect::<Vec<_>>()
                                .join(",")
                        },
                    )
                    .unwrap_or_else(
                        || {
                            "<lock-error>"
                                .to_string()
                        },
                    );

            trace_warning(
                format!(
                    "LSP TIMEOUT: id={} method={} after_ms={} pending_ids={} recv_error={}",
                    id,
                    method,
                    timeout.as_millis(),
                    pending_ids,
                    error,
                ),
            );

            Err(format!(
                "Timed out waiting for JDTLS response to {method}"
            ))
        }
    }
}

/**
 * ============================================================
 * JDTLS START
 * ============================================================
 */

#[tauri::command]
pub fn jdtls_start(
    app: AppHandle,
    on_diagnostics: Channel<JdtlsDiagnosticsEvent>,
    state: State<'_, JdtlsState>,
) -> Result<JdtlsStartResult, String> {
    let mut state_guard =
        state
            .session
            .lock()
            .map_err(|_| {
                "JDTLS state lock was poisoned"
                    .to_string()
            })?;

    if let Some(
        session,
    ) =
        state_guard.as_mut()
    {
        if let Ok(
            mut channel,
        ) =
            session
                .diagnostics_channel
                .lock()
        {
            *channel =
                Some(
                    on_diagnostics,
                );
        } else {
            return Err(
                "JDTLS diagnostics channel lock was poisoned"
                    .to_string(),
            );
        }

        let document_uri =
            path_to_file_uri(
                &session
                    .root
                    .join(
                        "Main.java",
                    ),
            );

        trace_log(
            "JDTLS START: reusing existing session"
                .to_string(),
        );

        return Ok(
            JdtlsStartResult {
                started:
                    true,
                document_uri,
                workspace_root:
                    session
                        .root
                        .to_string_lossy()
                        .to_string(),
                data_dir:
                    session
                        .data_dir
                        .to_string_lossy()
                        .to_string(),
            },
        );
    }

    trace_log(format!(
        "TRACE CONFIG: enabled={} payloads={} max_payload_chars={} flush={}",
        trace_config().enabled,
        trace_config().payloads,
        trace_config()
            .max_payload_chars,
        trace_config().flush,
    ));

    trace_log(
        "TRACE LEGEND: DOC=didChange, COMP=completion, CANCEL=cancellation, DIAG=diagnostics, LSP=raw protocol metadata"
            .to_string(),
    );

    let JdtlsProcess {
        root,
        data_dir,
        java,
        mut child,
        stdin,
        stdout,
        stderr,
    } =
        crate::jdtls_process::start(
            &app,
        )?;

    let pending:
        Arc<Mutex<PendingRequests>> =
        Arc::new(
            Mutex::new(
                HashMap::new(),
            ),
        );

    let cancelled_requests =
        Arc::new(
            Mutex::new(
                HashSet::new(),
            ),
        );

    let diagnostics_channel =
        Arc::new(
            Mutex::new(
                Some(
                    on_diagnostics,
                ),
            ),
        );

    spawn_stdout_reader(
        app.clone(),
        stdout,
        stdin.clone(),
        pending.clone(),
        cancelled_requests.clone(),
        root.clone(),
        diagnostics_channel.clone(),
    );

    spawn_stderr_reader(
        app.clone(),
        stderr,
    );

    trace_log(
        "JDTLS TRACE: stdout/stderr readers active"
            .to_string(),
    );

    let root_uri =
        path_to_file_uri(
            &root,
        );

    let initialize_id =
        1_u64;

    let jdk_home =
        jdk_home_from_java(
            &java,
        );

    let initialize_params =
        json!({
            "processId":
                std::process::id(),

            "clientInfo": {
                "name":
                    "Kynora",
                "version":
                    "0.1.0"
            },

            "rootUri":
                root_uri,

            "workspaceFolders": [
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
                                "Kynora",
                            )
                }
            ],

            "capabilities": {
                "general": {
                    "positionEncodings": [
                        "utf-16"
                    ]
                },

                "workspace": {
                    "applyEdit":
                        true,

                    "configuration":
                        true,

                    "workspaceFolders":
                        true
                },

                "textDocument": {
                    "synchronization": {
                        "dynamicRegistration":
                            false,

                        "willSave":
                            false,

                        "willSaveWaitUntil":
                            false,

                        "didSave":
                            true
                    },

                    "completion": {
                        "completionItem": {
                            "snippetSupport":
                                true,

                            "documentationFormat": [
                                "markdown",
                                "plaintext"
                            ],

                            "commitCharactersSupport":
                                true,

                            "deprecatedSupport":
                                true,

                            "preselectSupport":
                                true,

                            "insertReplaceSupport":
                                true,

                            "resolveSupport": {
                                "properties": [
                                    "documentation",
                                    "detail",
                                    "additionalTextEdits"
                                ]
                            }
                        },

                        "contextSupport":
                            true
                    },

                    "hover": {
                        "contentFormat": [
                            "markdown",
                            "plaintext"
                        ]
                    },

                    "definition": {
                        "linkSupport":
                            true
                    },

                    "publishDiagnostics": {
                        "relatedInformation":
                            true
                    },

                    "documentSymbol": {
                        "hierarchicalDocumentSymbolSupport":
                            true
                    },

                    "rename": {
                        "prepareSupport":
                            true
                    },

                    "codeAction": {
                        "codeActionLiteralSupport": {
                            "codeActionKind": {
                                "valueSet": [
                                    "",
                                    "quickfix",
                                    "refactor",
                                    "source.organizeImports"
                                ]
                            }
                        }
                    }
                }
            },

            "initializationOptions": {
                "bundles":
                    [],

                "workspaceFolders": [
                    root_uri
                ],

                "settings":
                    server_settings(
                        Some(
                            jdk_home,
                        ),
                    ),

                "extendedClientCapabilities": {
                    "resolveAdditionalTextEditsSupport":
                        true
                }
            }
        });

    trace_log(format!(
        "JDTLS INIT: root_uri={}",
        root_uri
    ));

    trace_log(format!(
        "JDTLS INIT: document_uri={}",
        path_to_file_uri(
            &root.join(
                "Main.java",
            ),
        )
    ));

    trace_log(format!(
        "JDTLS INIT: payload={}",
        trace_json(
            &initialize_params,
        )
    ));

    let initialize_response =
        send_request_and_wait(
            &stdin,
            &pending,
            &cancelled_requests,
            initialize_id,
            "initialize",
            initialize_params,
            Duration::from_secs(
                90,
            ),
        )?;

    if initialize_response
        .get("error")
        .is_some()
    {
        trace_error(
            format!(
                "JDTLS INIT ERROR: response={}",
                trace_json(
                    &initialize_response,
                )
            ),
        );

        let _ =
            child.kill();

        let _ =
            child.wait();

        return Err(format!(
            "JDTLS initialize failed: {initialize_response}"
        ));
    }

    crate::jdtls_trace::emit_log(
        &app,
        "JDTLS INIT: initialize response accepted; sending initialized"
            .to_string(),
    );

    let initialized =
        json!({
            "jsonrpc":
                "2.0",

            "method":
                "initialized",

            "params":
                {}
        });

    write_lsp_message(
        &stdin,
        &initialized,
    )?;

    crate::jdtls_trace::emit_log(
        &app,
        "JDTLS OUT: initialized notification sent"
            .to_string(),
    );

    let configuration =
        json!({
            "jsonrpc":
                "2.0",

            "method":
                "workspace/didChangeConfiguration",

            "params": {
                "settings":
                    server_settings(
                        Some(
                            jdk_home,
                        )
                    )
            }
        });

    write_lsp_message(
        &stdin,
        &configuration,
    )?;

    trace_log(format!(
        "JDTLS OUT: workspace/didChangeConfiguration payload={}",
        trace_json(
            &configuration,
        )
    ));

    let document_uri =
        path_to_file_uri(
            &root.join(
                "Main.java",
            ),
        );

    let session =
        JdtlsSession {
            stdin,

            child:
                Arc::new(
                    Mutex::new(
                        child,
                    ),
                ),

            pending,

            cancelled_requests,

            next_request_id:
                2,

            root:
                root.clone(),

            data_dir:
                data_dir.clone(),

            document_versions:
                HashMap::new(),

            diagnostics_channel,
        };

    *state_guard =
        Some(
            session,
        );

    Ok(
        JdtlsStartResult {
            started:
                true,

            document_uri,

            workspace_root:
                root
                    .to_string_lossy()
                    .to_string(),

            data_dir:
                data_dir
                    .to_string_lossy()
                    .to_string(),
        },
    )
}

/**
 * ============================================================
 * SHUTDOWN
 * ============================================================
 */

#[tauri::command]
pub fn jdtls_shutdown(
    state: State<'_, JdtlsState>,
) -> Result<(), String> {
    let session =
        state
            .session
            .lock()
            .map_err(|_| {
                "JDTLS state lock was poisoned"
                    .to_string()
            })?
            .take();

    let Some(
        mut session,
    ) =
        session
    else {
        trace_log(
            "SHUTDOWN: no active session"
                .to_string(),
        );

        return Ok(());
    };

    emit_shutdown_summary(
        &session,
    );

    let id =
        session.next_request_id;

    session.next_request_id += 1;

    trace_log(format!(
        "SHUTDOWN REQUEST: id={}",
        id,
    ));

    let _ =
        send_request_and_wait(
            &session.stdin,
            &session.pending,
            &session.cancelled_requests,
            id,
            "shutdown",
            Value::Null,
            Duration::from_secs(
                3,
            ),
        );

    let exit =
        json!({
            "jsonrpc":
                "2.0",

            "method":
                "exit",

            "params":
                null
        });

    let _ =
        write_lsp_message(
            &session.stdin,
            &exit,
        );

    if let Ok(
        mut child,
    ) =
        session.child.lock()
    {
        let _ =
            child.kill();

        let _ =
            child.wait();
    }

    Ok(())
}

fn emit_shutdown_summary(
    session: &JdtlsSession,
) {
    if !trace_enabled() {
        return;
    }

    let pending_count =
        session
            .pending
            .lock()
            .map(
                |map|
                    map.len(),
            )
            .unwrap_or(
                0,
            );

    trace_summary(
        pending_count,
    );
}