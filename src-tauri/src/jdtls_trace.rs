use serde_json::Value;
use std::{
    io::Write,
    sync::{
        atomic::{
            AtomicU64,
            Ordering,
        },
        OnceLock,
    },
    time::Instant,
};
use tauri::{
    AppHandle,
    Emitter,
};

pub const LOG_EVENT: &str =
    "jdtls_log";

/*
 * ============================================================
 * TRACE CONFIGURATION
 * ============================================================
 *
 * Normal mode:
 *
 *   KYNORA_JDTLS_TRACE=0
 *
 * Trace mode:
 *
 *   KYNORA_JDTLS_TRACE=1
 *
 * Optional raw/bounded payload mode:
 *
 *   KYNORA_JDTLS_TRACE_PAYLOADS=1
 *
 * Optional forced flush:
 *
 *   KYNORA_JDTLS_TRACE_FLUSH=1
 *
 * Optional maximum serialized payload characters:
 *
 *   KYNORA_JDTLS_TRACE_MAX_PAYLOAD=6000
 *
 * The normal trace mode logs protocol metadata without
 * serializing huge JSON payloads.
 */

#[derive(Clone, Copy)]
pub(crate) struct TraceConfig {
    pub(crate) enabled: bool,
    pub(crate) payloads: bool,
    pub(crate) flush: bool,
    pub(crate) max_payload_chars: usize,
}

fn env_flag(
    key: &str,
) -> bool {
    std::env::var_os(key)
        .map(|value| {
            let value =
                value
                    .to_string_lossy()
                    .to_ascii_lowercase();

            value == "1"
                || value == "true"
                || value == "yes"
                || value == "on"
        })
        .unwrap_or(false)
}

fn env_usize(
    key: &str,
    default_value: usize,
    min_value: usize,
    max_value: usize,
) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|value| {
            value.parse::<usize>().ok()
        })
        .unwrap_or(default_value)
        .clamp(
            min_value,
            max_value,
        )
}

static TRACE_CONFIG: OnceLock<TraceConfig> =
    OnceLock::new();

static TRACE_START: OnceLock<Instant> =
    OnceLock::new();

pub(crate) fn trace_config() -> &'static TraceConfig {
    TRACE_CONFIG.get_or_init(|| {
        let payloads =
            env_flag(
                "KYNORA_JDTLS_TRACE_PAYLOADS",
            );

        TraceConfig {
            enabled: true,

            payloads,

            flush:
                env_flag(
                    "KYNORA_JDTLS_TRACE_FLUSH",
                ),

            max_payload_chars:
                env_usize(
                    "KYNORA_JDTLS_TRACE_MAX_PAYLOAD",
                    6000,
                    512,
                    100_000,
                ),
        }
    })
}

fn trace_start() -> &'static Instant {
    TRACE_START.get_or_init(
        Instant::now,
    )
}

pub(crate) fn trace_enabled() -> bool {
    trace_config().enabled
}

pub(crate) fn trace_payloads_enabled() -> bool {
    trace_config().payloads
}

/*
 * ============================================================
 * TRACE METRICS
 * ============================================================
 *
 * These counters make every debugging session auditable.
 */

pub(crate) struct TraceMetrics {
    pub(crate) lsp_out:
        AtomicU64,

    pub(crate) lsp_in:
        AtomicU64,

    pub(crate) did_open:
        AtomicU64,

    pub(crate) did_change:
        AtomicU64,

    pub(crate) incremental_changes:
        AtomicU64,

    pub(crate) full_resyncs:
        AtomicU64,

    pub(crate) completion_requests:
        AtomicU64,

    pub(crate) completion_responses:
        AtomicU64,

    pub(crate) completion_cancels:
        AtomicU64,

    pub(crate) duplicate_cancels_suppressed:
        AtomicU64,

    pub(crate) completion_resolves:
        AtomicU64,

    pub(crate) diagnostics:
        AtomicU64,

    pub(crate) timeouts:
        AtomicU64,

    pub(crate) protocol_errors:
        AtomicU64,

    pub(crate) unmatched_responses:
        AtomicU64,

    pub(crate) duplicate_request_ids:
        AtomicU64,

    pub(crate) log_lines:
        AtomicU64,
}

pub(crate) static TRACE_METRICS: TraceMetrics =
    TraceMetrics {
        lsp_out:
            AtomicU64::new(0),

        lsp_in:
            AtomicU64::new(0),

        did_open:
            AtomicU64::new(0),

        did_change:
            AtomicU64::new(0),

        incremental_changes:
            AtomicU64::new(0),

        full_resyncs:
            AtomicU64::new(0),

        completion_requests:
            AtomicU64::new(0),

        completion_responses:
            AtomicU64::new(0),

        completion_cancels:
            AtomicU64::new(0),

        duplicate_cancels_suppressed:
            AtomicU64::new(0),

        completion_resolves:
            AtomicU64::new(0),

        diagnostics:
            AtomicU64::new(0),

        timeouts:
            AtomicU64::new(0),

        protocol_errors:
            AtomicU64::new(0),

        unmatched_responses:
            AtomicU64::new(0),

        duplicate_request_ids:
            AtomicU64::new(0),

        log_lines:
            AtomicU64::new(0),
    };

static TRACE_SEQUENCE:
    AtomicU64 =
    AtomicU64::new(1);

fn next_trace_sequence()
    -> u64
{
    TRACE_SEQUENCE.fetch_add(
        1,
        Ordering::Relaxed,
    )
}

fn trace_line(
    level: &str,
    message: String,
) {
    let sequence =
        next_trace_sequence();

    TRACE_METRICS
        .log_lines
        .fetch_add(
            1,
            Ordering::Relaxed,
        );

    let elapsed =
        trace_start().elapsed();

    let total_millis =
        elapsed.as_millis();

    let seconds =
        total_millis / 1000;

    let millis =
        total_millis % 1000;

    println!(
        "[KYNORA/JDTLS][{level}][#{sequence:08}][+{seconds}.{millis:03}s] {message}"
    );

    if trace_config().flush {
        let _ =
            std::io::stdout().flush();
    }
}

pub(crate) fn trace_log(
    message: String,
) {
    if !trace_enabled() {
        return;
    }

    trace_line(
        "TRACE",
        message,
    );
}

pub(crate) fn emit_log(
    app: &AppHandle,
    message: String,
) {
    trace_line(
        "INFO",
        message.clone(),
    );

    /*
     * Preserve the existing Tauri log-event behavior
     * when trace mode is active.
     */
    if trace_enabled() {
        let _ =
            app.emit(
                LOG_EVENT,
                message,
            );
    }
}

pub(crate) fn trace_warning(
    message: String,
) {
    trace_line(
        "WARN",
        message,
    );
}

pub(crate) fn trace_error(
    message: String,
) {
    trace_line(
        "ERROR",
        message,
    );
}

fn truncate_chars(
    value: &str,
    maximum: usize,
) -> String {
    if value.chars().count()
        <= maximum
    {
        return value.to_string();
    }

    let mut result =
        value
            .chars()
            .take(maximum)
            .collect::<String>();

    result.push_str(
        "...<truncated>",
    );

    result
}

pub(crate) fn trace_json(
    value: &Value,
) -> String {
    if !trace_payloads_enabled() {
        return "<payloads-disabled>"
            .to_string();
    }

    match serde_json::to_string(value) {
        Ok(serialized) => {
            let maximum =
                trace_config()
                    .max_payload_chars;

            if serialized.chars().count()
                > maximum
            {
                truncate_chars(
                    &serialized,
                    maximum,
                )
            } else {
                serialized
            }
        }

        Err(_) =>
            "<json-serialization-failed>"
                .to_string(),
    }
}

pub(crate) fn trace_shape(
    value: &Value,
) -> String {
    let kind =
        match (
            value.get("method"),
            value.get("id"),
        ) {
            (Some(_), Some(_)) =>
                "request",

            (Some(_), None) =>
                "notification",

            (None, Some(_)) =>
                "response",

            (None, None) =>
                "unknown",
        };

    let method =
        value
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("<none>");

    let id =
        value
            .get("id")
            .map(Value::to_string)
            .unwrap_or_else(|| {
                "<none>".to_string()
            });

    format!(
        "kind={kind} method={method} id={id}"
    )
}

pub(crate) fn trace_summary(
    pending_count: usize,
) {
    if !trace_enabled() {
        return;
    }

    trace_log(format!(
        "TRACE SUMMARY: lsp_out={} lsp_in={} did_open={} did_change={} incremental={} full_resyncs={} completion_requests={} completion_responses={} completion_cancels={} duplicate_cancels_suppressed={} resolves={} diagnostics={} timeouts={} protocol_errors={} unmatched_responses={} duplicate_request_ids={} pending={}",
        TRACE_METRICS
            .lsp_out
            .load(Ordering::Relaxed),

        TRACE_METRICS
            .lsp_in
            .load(Ordering::Relaxed),

        TRACE_METRICS
            .did_open
            .load(Ordering::Relaxed),

        TRACE_METRICS
            .did_change
            .load(Ordering::Relaxed),

        TRACE_METRICS
            .incremental_changes
            .load(Ordering::Relaxed),

        TRACE_METRICS
            .full_resyncs
            .load(Ordering::Relaxed),

        TRACE_METRICS
            .completion_requests
            .load(Ordering::Relaxed),

        TRACE_METRICS
            .completion_responses
            .load(Ordering::Relaxed),

        TRACE_METRICS
            .completion_cancels
            .load(Ordering::Relaxed),

        TRACE_METRICS
            .duplicate_cancels_suppressed
            .load(Ordering::Relaxed),

        TRACE_METRICS
            .completion_resolves
            .load(Ordering::Relaxed),

        TRACE_METRICS
            .diagnostics
            .load(Ordering::Relaxed),

        TRACE_METRICS
            .timeouts
            .load(Ordering::Relaxed),

        TRACE_METRICS
            .protocol_errors
            .load(Ordering::Relaxed),

        TRACE_METRICS
            .unmatched_responses
            .load(Ordering::Relaxed),

        TRACE_METRICS
            .duplicate_request_ids
            .load(Ordering::Relaxed),

        pending_count,
    ));
}