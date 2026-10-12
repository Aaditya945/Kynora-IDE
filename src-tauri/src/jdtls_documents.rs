use serde_json::json;
use std::{
    sync::atomic::Ordering,
};

use tauri::State;

use crate::jdtls::{
    send_request_and_wait,
    JdtlsContentChange,
};
use crate::jdtls_session::JdtlsState;
use crate::jdtls_trace::{
    trace_error,
    trace_log,
    trace_payloads_enabled,
    trace_warning,
    TRACE_METRICS,
};
use crate::jdtls_transport::{
    write_lsp_message,
};

/*
 * ============================================================
 * DID OPEN
 * ============================================================
 */

#[tauri::command]
pub fn jdtls_did_open(
    uri:
        String,
    language_id:
        String,
    text:
        String,
    version:
        u32,
    state:
        State<'_, JdtlsState>,
) -> Result<u32, String> {
    let mut guard =
        state
            .session
            .lock()
            .map_err(|_| {
                "JDTLS state lock was poisoned"
                    .to_string()
            })?;

    let session =
        guard
            .as_mut()
            .ok_or_else(|| {
                "JDTLS is not running"
                    .to_string()
            })?;

    let effective_version =
        match session
            .document_versions
            .get(
                &uri,
            )
            .copied()
        {
            Some(current) =>
                current
                    .saturating_add(1)
                    .max(
                        version,
                    ),

            None =>
                version.max(1),
        };

    let text_bytes =
        text.as_bytes().len();

    let text_utf16 =
        text.encode_utf16()
            .count();

    trace_log(format!(
        "DID_OPEN: uri={} language={} requested_version={} effective_version={} text_bytes={} text_utf16={}",
        uri,
        language_id,
        version,
        effective_version,
        text_bytes,
        text_utf16,
    ));

    TRACE_METRICS
        .did_open
        .fetch_add(
            1,
            Ordering::Relaxed,
        );

    if session
        .document_versions
        .contains_key(
            &uri,
        )
    {
        let params =
            json!({
                "textDocument": {
                    "uri":
                        uri,

                    "version":
                        effective_version
                },

                "contentChanges": [
                    {
                        "text":
                            text
                    }
                ]
            });

        let message =
            json!({
                "jsonrpc":
                    "2.0",

                "method":
                    "textDocument/didChange",

                "params":
                    params
            });

        trace_log(format!(
            "DID_OPEN RECOVERY: mode=full-resync uri={} version={} text_bytes={}",
            uri,
            effective_version,
            text_bytes,
        ));

        TRACE_METRICS
            .full_resyncs
            .fetch_add(
                1,
                Ordering::Relaxed,
            );

        write_lsp_message(
            &session.stdin,
            &message,
        )?;
    } else {
        let params =
            json!({
                "textDocument": {
                    "uri":
                        uri,

                    "languageId":
                        language_id,

                    "version":
                        effective_version,

                    "text":
                        text
                }
            });

        let message =
            json!({
                "jsonrpc":
                    "2.0",

                "method":
                    "textDocument/didOpen",

                "params":
                    params
            });

        trace_log(format!(
            "DID_OPEN SENT: uri={} version={} text_bytes={} text_utf16={}",
            uri,
            effective_version,
            text_bytes,
            text_utf16,
        ));

        write_lsp_message(
            &session.stdin,
            &message,
        )?;
    }

    session
        .document_versions
        .insert(
            uri,
            effective_version,
        );

    Ok(
        effective_version,
    )
}

/*
 * ============================================================
 * DIAGNOSTIC REFRESH
 * ============================================================
 */

#[tauri::command]
pub fn jdtls_refresh_diagnostics(
    uri:
        String,
    state:
        State<'_, JdtlsState>,
) -> Result<(), String> {
    let mut guard =
        state
            .session
            .lock()
            .map_err(|_| {
                "JDTLS state lock was poisoned"
                    .to_string()
            })?;

    let session =
        guard
            .as_mut()
            .ok_or_else(|| {
                "JDTLS is not running"
                    .to_string()
            })?;

    let id =
        session.next_request_id;

    session.next_request_id =
        session
            .next_request_id
            .saturating_add(
                1,
            );

    trace_log(format!(
        "DIAGNOSTIC REFRESH REQUEST: id={} uri={}",
        id,
        uri,
    ));

    let params =
        json!({
            "command":
                "java.project.refreshDiagnostics",

            "arguments":
                [uri],
        });

    let response =
        send_request_and_wait(
            &session.stdin,
            &session.pending,
            &session.cancelled_requests,
            id,
            "workspace/executeCommand",
            params,
            std::time::Duration::from_secs(
                5,
            ),
        )?;

    if let Some(
        error,
    ) =
        response.get(
            "error",
        )
    {
        return Err(format!(
            "JDTLS diagnostic refresh failed: {error}"
        ));
    }

    Ok(())
}

/*
 * ============================================================
 * M5 INCREMENTAL DID CHANGE
 * ============================================================
 */

#[tauri::command]
pub fn jdtls_did_change(
    uri:
        String,
    version:
        u32,
    changes:
        Vec<JdtlsContentChange>,
    frontend_sync_sequence:
        u64,
    monaco_version:
        u64,
    completion_generation:
        u64,
    document_length_utf16:
        u64,
    document_line_count:
        u32,
    state:
        State<'_, JdtlsState>,
) -> Result<u32, String> {
    if changes.is_empty() {
        trace_error(
            format!(
                "M5 DID_CHANGE REJECTED: sync_seq={} reason=no-content-changes",
                frontend_sync_sequence,
            ),
        );

        return Err(
            "JDTLS didChange received no content changes."
                .to_string(),
        );
    }

    let full_document_count =
        changes
            .iter()
            .filter(
                |change|
                    change.range.is_none(),
            )
            .count();

    if full_document_count > 0
        && (
            full_document_count
                != 1
            || changes.len()
                != 1
        )
    {
        trace_error(
            format!(
                "M5 DID_CHANGE REJECTED: sync_seq={} invalid-full-document-change count={} total_changes={}",
                frontend_sync_sequence,
                full_document_count,
                changes.len(),
            ),
        );

        return Err(
            "A full-document JDTLS change must be the only content change in the notification."
                .to_string(),
        );
    }

    let mut guard =
        state
            .session
            .lock()
            .map_err(|_| {
                "JDTLS state lock was poisoned"
                    .to_string()
            })?;

    let session =
        guard
            .as_mut()
            .ok_or_else(|| {
                "JDTLS is not running"
                    .to_string()
            })?;

    let current =
        session
            .document_versions
            .get(
                &uri,
            )
            .copied()
            .unwrap_or(
                0,
            );

    let version_adjusted =
        version <= current;

    let effective_version =
        if version > current {
            version
        } else {
            current
                .saturating_add(
                    1,
                )
        };

    if version_adjusted {
        trace_warning(format!(
            "M5 VERSION ADJUSTED: sync_seq={} requested_version={} current_version={} effective_version={}",
            frontend_sync_sequence,
            version,
            current,
            effective_version,
        ));
    }

    let is_full_document =
        full_document_count == 1;

    let mode =
        if is_full_document {
            "full-resync"
        } else {
            "incremental"
        };

    let total_changed_bytes =
        changes
            .iter()
            .map(
                |change|
                    change.text.as_bytes().len(),
            )
            .sum::<usize>();

    let total_changed_utf16 =
        changes
            .iter()
            .map(
                |change|
                    change
                        .text
                        .encode_utf16()
                        .count(),
            )
            .sum::<usize>();

    TRACE_METRICS
        .did_change
        .fetch_add(
            1,
            Ordering::Relaxed,
        );

    if is_full_document {
        TRACE_METRICS
            .full_resyncs
            .fetch_add(
                1,
                Ordering::Relaxed,
            );
    } else {
        TRACE_METRICS
            .incremental_changes
        .fetch_add(
            1,
            Ordering::Relaxed,
        );
    }

    trace_log(format!(
        "M5 DID_CHANGE: sync_seq={} mode={} uri={} requested_version={} current_version={} effective_version={} version_adjusted={} changes={} changed_utf8_bytes={} changed_utf16_units={} document_utf16_units={} document_lines={} monaco_version={} generation={}",
        frontend_sync_sequence,
        mode,
        uri,
        version,
        current,
        effective_version,
        version_adjusted,
        changes.len(),
        total_changed_bytes,
        total_changed_utf16,
        document_length_utf16,
        document_line_count,
        monaco_version,
        completion_generation,
    ));

    for (
        index,
        change,
    ) in
        changes.iter().enumerate()
    {
        match &change.range {
            Some(
                range,
            ) => {
                trace_log(format!(
                    "M5 CHANGE DETAIL: sync_seq={} index={} mode=incremental start=({}, {}) end=({}, {}) utf8_bytes={} utf16_units={} text={}",
                    frontend_sync_sequence,
                    index,
                    range.start.line,
                    range.start.character,
                    range.end.line,
                    range.end.character,
                    change.text.as_bytes().len(),
                    change.text.encode_utf16().count(),
                    if trace_payloads_enabled() {
                        change
                            .text
                            .chars()
                            .take(200)
                            .collect::<String>()
                    } else {
                        "<text-hidden>".to_string()
                    },
                ));
            }

            None => {
                trace_log(format!(
                    "M5 CHANGE DETAIL: sync_seq={} index={} mode=full-resync utf8_bytes={} utf16_units={} text_visible={}",
                    frontend_sync_sequence,
                    index,
                    change.text.as_bytes().len(),
                    change.text.encode_utf16().count(),
                    trace_payloads_enabled(),
                ));
            }
        }
    }

    let params =
        json!({
            "textDocument": {
                "uri":
                    uri,

                "version":
                    effective_version
            },

            "contentChanges":
                changes
        });

    let message =
        json!({
            "jsonrpc":
                "2.0",

            "method":
                "textDocument/didChange",

            "params":
                params
        });

    let body_bytes =
        serde_json::to_vec(
            &message,
        )
        .map(|body|
            body.len())
        .unwrap_or(
            0,
        );

    trace_log(format!(
        "LSP OUT: didChange sync_seq={} mode={} version={} change_count={} framed_body_bytes={}",
        frontend_sync_sequence,
        mode,
        effective_version,
        changes.len(),
        body_bytes,
    ));

    write_lsp_message(
        &session.stdin,
        &message,
    )?;

    trace_log(format!(
        "M5 DID_CHANGE SENT: sync_seq={} mode={} version={} status=success",
        frontend_sync_sequence,
        mode,
        effective_version,
    ));

    session
        .document_versions
        .insert(
            uri,
            effective_version,
        );

    Ok(
        effective_version,
    )
}

/*
 * ============================================================
 * DID CLOSE
 * ============================================================
 */

#[tauri::command]
pub fn jdtls_did_close(
    uri:
        String,
    state:
        State<'_, JdtlsState>,
) -> Result<(), String> {
    let mut guard =
        state
            .session
            .lock()
            .map_err(|_| {
                "JDTLS state lock was poisoned"
                    .to_string()
            })?;

    let session =
        guard
            .as_mut()
            .ok_or_else(|| {
                "JDTLS is not running"
                    .to_string()
            })?;

    let message =
        json!({
            "jsonrpc":
                "2.0",

            "method":
                "textDocument/didClose",

            "params": {
                "textDocument": {
                    "uri":
                        uri
                }
            }
        });

    trace_log(format!(
        "DID_CLOSE: uri={}",
        uri,
    ));

    write_lsp_message(
        &session.stdin,
        &message,
    )?;

    session
        .document_versions
        .remove(
            &uri,
        );

    Ok(())
}