use serde_json::{json, Value};
use std::{
    sync::atomic::Ordering,
    time::Duration,
};

use tauri::State;

use crate::{
    jdtls::{
        send_request_and_wait,
        JdtlsState,
    },
    jdtls_trace::{
        trace_json,
        trace_log,
        TRACE_METRICS,
    },
    jdtls_transport::write_lsp_message,
};

#[tauri::command]
pub fn jdtls_completion(
    uri: String,
    line: u32,
    character: u32,
    request_id: u64,
    trigger_kind: u8,
    trigger_character: Option<String>,
    document_version: u32,
    completion_generation: u64,
    state: State<'_, JdtlsState>,
) -> Result<Value, String> {
    let (
        stdin,
        pending,
        cancelled_requests,
    ) = {
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

        (
            session.stdin.clone(),
            session.pending.clone(),
            session.cancelled_requests.clone(),
        )
    };

    let normalized_trigger_kind =
        match trigger_kind {
            2 | 3 => trigger_kind,
            _ => 1,
        };

    let mut context =
        json!({
            "triggerKind":
                normalized_trigger_kind
        });

    if normalized_trigger_kind == 2 {
        if let Some(character) =
            trigger_character.clone()
        {
            context[
                "triggerCharacter"
            ] =
                Value::String(
                    character,
                );
        }
    }

    TRACE_METRICS
        .completion_requests
        .fetch_add(
            1,
            Ordering::Relaxed,
        );

    trace_log(format!(
        "JDTLS COMPLETION: id={} uri={} line={} character={} trigger_kind={} trigger_character={} document_version={} generation={}",
        request_id,
        uri,
        line,
        character,
        normalized_trigger_kind,
        trigger_character
            .as_deref()
            .unwrap_or("<none>"),
        document_version,
        completion_generation,
    ));

    let params =
        json!({
            "textDocument": {
                "uri":
                    uri
            },

            "position": {
                "line":
                    line,

                "character":
                    character
            },

            "context":
                context
        });

    let response =
        send_request_and_wait(
            &stdin,
            &pending,
            &cancelled_requests,
            request_id,
            "textDocument/completion",
            params,
            Duration::from_secs(2),
        )?;

    if let Some(error) =
        response.get("error")
    {
        return Err(format!(
            "JDTLS completion failed: {error}"
        ));
    }

    let result =
        response
            .get("result")
            .cloned()
            .unwrap_or(Value::Null);

    let normalized =
        if let Some(items) =
            result.as_array()
        {
            json!({
                "isIncomplete":
                    false,

                "items":
                    items
            })
        } else if result.is_object() {
            let is_incomplete =
                result
                    .get("isIncomplete")
                    .and_then(
                        Value::as_bool,
                    )
                    .unwrap_or(false);

            let items =
                result
                    .get("items")
                    .and_then(
                        Value::as_array,
                    )
                    .cloned()
                    .unwrap_or_default();

            json!({
                "isIncomplete":
                    is_incomplete,

                "items":
                    items
            })
        } else {
            json!({
                "isIncomplete":
                    false,

                "items":
                    []
            })
        };

    trace_log(format!(
        "JDTLS COMPLETION RESPONSE: id={} item_count={} incomplete={} document_version={} generation={}",
        request_id,

        normalized
            .get("items")
            .and_then(
                Value::as_array,
            )
            .map(
                |items|
                    items.len(),
            )
            .unwrap_or(
                0,
            ),

        normalized
            .get("isIncomplete")
            .and_then(
                Value::as_bool,
            )
            .unwrap_or(false),

        document_version,
        completion_generation,
    ));

    Ok(
        normalized,
    )
}

#[tauri::command]
pub fn jdtls_cancel_request(
    request_id: u64,
    state: State<'_, JdtlsState>,
) -> Result<(), String> {
    let (
        stdin,
        pending,
        cancelled_requests,
    ) = {
        let guard =
            state
                .session
                .lock()
                .map_err(|_| {
                    "JDTLS state lock was poisoned"
                        .to_string()
                })?;

        let session =
            guard
                .as_ref()
                .ok_or_else(|| {
                    "JDTLS is not running"
                        .to_string()
                })?;

        (
            session.stdin.clone(),
            session.pending.clone(),
            session.cancelled_requests.clone(),
        )
    };

    let (
        should_send,
        reason,
    ) = {
        let pending_guard =
            pending
                .lock()
                .map_err(|_| {
                    "JDTLS pending request lock was poisoned"
                        .to_string()
                })?;

        if !pending_guard
            .contains_key(
                &request_id,
            )
        {
            (
                false,
                "not-pending",
            )
        } else {
            let mut cancelled_guard =
                cancelled_requests
                    .lock()
                    .map_err(|_| {
                        "JDTLS cancellation lock was poisoned"
                            .to_string()
                    })?;

            if cancelled_guard
                .contains(
                    &request_id,
                )
            {
                (
                    false,
                    "already-cancelled",
                )
            } else {
                cancelled_guard.insert(
                    request_id,
                );

                (
                    true,
                    "active",
                )
            }
        }
    };

    if !should_send {
        if reason ==
            "already-cancelled"
        {
            TRACE_METRICS
                .duplicate_cancels_suppressed
                .fetch_add(
                    1,
                    Ordering::Relaxed,
                );

            trace_log(format!(
                "JDTLS CANCEL SUPPRESSED: id={} reason={}",
                request_id,
                reason,
            ));
        } else {
            trace_log(format!(
                "JDTLS CANCEL SKIPPED: id={} reason={}",
                request_id,
                reason,
            ));
        }

        return Ok(());
    }

    TRACE_METRICS
        .completion_cancels
        .fetch_add(
            1,
            Ordering::Relaxed,
        );

    let message =
        json!({
            "jsonrpc":
                "2.0",

            "method":
                "$/cancelRequest",

            "params": {
                "id":
                    request_id
            }
        });

    trace_log(format!(
        "JDTLS CANCEL: id={} reason=active payload={}",
        request_id,
        reason,
    ));

    trace_log(format!(
        "JDTLS CANCEL PAYLOAD: {}",
        trace_json(
            &message,
        ),
    ));

    write_lsp_message(
        &stdin,
        &message,
    )?;

    Ok(())
}

fn completion_item_label(
    item:
        &Value,
) -> String {
    match item.get("label") {
        Some(Value::String(label)) =>
            label.clone(),

        Some(Value::Object(object)) =>
            object
                .get("label")
                .and_then(
                    Value::as_str,
                )
                .unwrap_or("<unknown>")
                .to_string(),

        _ =>
            "<unknown>".to_string(),
    }
}

#[tauri::command]
pub fn jdtls_completion_resolve(
    item:
        Value,
    state:
        State<'_, JdtlsState>,
) -> Result<Value, String> {
    let (
        stdin,
        pending,
        cancelled_requests,
        id,
    ) = {
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
            session
                .next_request_id;

        session.next_request_id =
            session
                .next_request_id
                .saturating_add(1);

        (
            session.stdin.clone(),
            session.pending.clone(),
            session.cancelled_requests.clone(),
            id,
        )
    };

    TRACE_METRICS
        .completion_resolves
        .fetch_add(
            1,
            Ordering::Relaxed,
        );

    trace_log(format!(
        "JDTLS COMPLETION RESOLVE: id={} label={}",
        id,
        completion_item_label(
            &item,
        ),
    ));

    let response =
        send_request_and_wait(
            &stdin,
            &pending,
            &cancelled_requests,
            id,
            "completionItem/resolve",
            item,
            Duration::from_secs(2),
        )?;

    if let Some(error) =
        response.get("error")
    {
        return Err(format!(
            "JDTLS completionItem/resolve failed: {error}"
        ));
    }

    let result =
        response
            .get("result")
            .cloned()
            .unwrap_or(Value::Null);

    let documentation =
        result
            .get("documentation")
            .is_some();

    let additional_edits =
        result
            .get("additionalTextEdits")
            .and_then(
                Value::as_array,
            )
            .map(
                |edits|
                    edits.len(),
            )
            .unwrap_or(
                0,
            );

    trace_log(format!(
        "COMPLETION RESOLVE RESPONSE: id={} documentation={} additionalTextEdits={}",
        id,
        documentation,
        additional_edits,
    ));

    Ok(
        result,
    )
}