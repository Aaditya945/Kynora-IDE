mod java_runner;
mod jdtls;
mod jdtls_documents;
mod jdtls_io;
mod jdtls_runtime;
mod jdtls_session;
mod jdtls_trace;
mod jdtls_transport;
mod jdtls_completion;
mod jdtls_process;

use serde::Serialize;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{
        SystemTime,
        UNIX_EPOCH,
    },
};

use java_runner::JavaProcessState;

fn java_home() -> Option<PathBuf> {
    std::env::var_os(
        "JAVA_HOME",
    )
    .map(PathBuf::from)
}

fn java_binary() -> PathBuf {
    if let Some(home) =
        java_home()
    {
        let executable =
            if cfg!(windows) {
                "java.exe"
            } else {
                "java"
            };

        let candidate =
            home
                .join("bin")
                .join(executable);

        if candidate.exists() {
            return candidate;
        }
    }

    PathBuf::from(
        if cfg!(windows) {
            "java.exe"
        } else {
            "java"
        },
    )
}

fn javac_binary() -> PathBuf {
    if let Some(home) =
        java_home()
    {
        let executable =
            if cfg!(windows) {
                "javac.exe"
            } else {
                "javac"
            };

        let candidate =
            home
                .join("bin")
                .join(executable);

        if candidate.exists() {
            return candidate;
        }
    }

    PathBuf::from(
        if cfg!(windows) {
            "javac.exe"
        } else {
            "javac"
        },
    )
}

fn create_run_directory()
    -> Result<PathBuf, String>
{
    let base =
        std::env::temp_dir()
            .join("kynora-java");

    fs::create_dir_all(
        &base,
    )
    .map_err(|e| {
        format!(
            "Could not create Kynora temporary directory: {e}"
        )
    })?;

    let suffix =
        SystemTime::now()
            .duration_since(
                UNIX_EPOCH,
            )
            .map_err(|e| {
                format!(
                    "Could not create unique run directory: {e}"
                )
            })?
            .as_nanos();

    let directory =
        base.join(
            format!(
                "run-{suffix}"
            ),
        );

    fs::create_dir_all(
        &directory,
    )
    .map_err(|e| {
        format!(
            "Could not create temporary Java run directory: {e}"
        )
    })?;

    Ok(directory)
}

#[derive(Debug, Clone, Serialize)]
pub struct JavaDiagnostic {
    pub line: u32,
    pub column: u32,
    pub severity: String,
    pub message: String,
}

fn parse_java_diagnostics(
    stderr: &str,
    source_name: &str,
) -> Vec<JavaDiagnostic> {
    let mut diagnostics =
        Vec::new();

    for line in stderr.lines() {
        let trimmed =
            line.trim();

        let Some(rest) =
            trimmed
                .strip_prefix(
                    source_name,
                )
        else {
            continue;
        };

        let Some(rest) =
            rest.strip_prefix(':')
        else {
            continue;
        };

        let Some(
            (line_number, rest),
        ) =
            rest.split_once(':')
        else {
            continue;
        };

        let Ok(line_number) =
            line_number.parse::<u32>()
        else {
            continue;
        };

        let Some(rest) =
            rest.strip_prefix(' ')
        else {
            continue;
        };

        let (
            severity,
            message,
        ) =
            if let Some(message) =
                rest.strip_prefix(
                    "error: ",
                )
            {
                (
                    "error",
                    message.to_string(),
                )
            } else if let Some(message) =
                rest.strip_prefix(
                    "warning: ",
                )
            {
                (
                    "warning",
                    message.to_string(),
                )
            } else {
                continue;
            };

        diagnostics.push(
            JavaDiagnostic {
                line:
                    line_number,
                column:
                    1,
                severity:
                    severity.to_string(),
                message,
            },
        );
    }

    diagnostics
}

#[tauri::command]
fn diagnose_java(
    class_name: String,
    classpath: String,
    source_code: String,
) -> Result<
    Vec<JavaDiagnostic>,
    String,
> {
    if source_code
        .trim()
        .is_empty()
    {
        return Ok(
            Vec::new(),
        );
    }

    let run_dir =
        create_run_directory()?;

    let source_path =
        run_dir.join(
            format!(
                "{class_name}.java"
            ),
        );

    fs::write(
        &source_path,
        source_code.as_bytes(),
    )
    .map_err(|e| {
        format!(
            "Could not write temporary Java source: {e}"
        )
    })?;

    let mut command =
        Command::new(
            javac_binary(),
        );

    command
        .arg("-encoding")
        .arg("UTF-8")
        .arg("-Xdiags:verbose")
        .arg("-proc:none")
        .arg("-d")
        .arg(&run_dir);

    if !classpath
        .trim()
        .is_empty()
    {
        command
            .arg("-cp")
            .arg(&classpath);
    }

    command.arg(
        &source_path,
    );

    let output =
        command
            .output()
            .map_err(|e| {
                format!(
                    "Could not start javac: {e}"
                )
            })?;

    let stderr =
        String::from_utf8_lossy(
            &output.stderr,
        )
        .to_string();

    let diagnostics =
        parse_java_diagnostics(
            &stderr,
            &format!(
                "{class_name}.java"
            ),
        );

    let _ =
        fs::remove_dir_all(
            &run_dir,
        );

    Ok(diagnostics)
}

#[cfg_attr(
    mobile,
    tauri::mobile_entry_point
)]
pub fn run() {
    tauri::Builder::default()
        .manage(
            JavaProcessState::default(),
        )
        .manage(
            jdtls::JdtlsState::default(),
        )
        .invoke_handler(
            tauri::generate_handler![
                diagnose_java,

                java_runner::run_java_class,
                java_runner::send_java_input,

                jdtls::jdtls_start,

                jdtls_documents::jdtls_did_open,
                jdtls_documents::jdtls_refresh_diagnostics,

                jdtls_completion::jdtls_completion,
                jdtls_completion::jdtls_cancel_request,
                jdtls_completion::jdtls_completion_resolve,

                jdtls_documents::jdtls_did_change,
                jdtls_documents::jdtls_did_close,

                jdtls::jdtls_shutdown,
            ],
        )
        .run(
            tauri::generate_context!(),
        )
        .expect(
            "error while running Kynora",
        );
}