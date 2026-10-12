use serde::Serialize;
use std::{
    fs,
    io::{Read, Write},
    process::{ChildStdin, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
    time::Instant,
};
use tauri::{ipc::Channel, State};

use super::{
    create_run_directory,
    java_binary,
    javac_binary,
};

#[derive(Debug, Clone, Serialize)]
pub struct JavaCompileResult {
    pub compiled: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JavaExitEvent {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "event",
    content = "data",
    rename_all = "snake_case"
)]
pub enum JavaProcessEvent {
    Stdout(String),
    Stderr(String),
    Exit(JavaExitEvent),
}

#[derive(Clone, Default)]
pub struct JavaProcessState {
    pub stdin:
        Arc<
            Mutex<
                Option<
                    Arc<
                        Mutex<
                            ChildStdin,
                        >,
                    >,
                >,
            >,
        >,
}

fn read_stream<
    R: Read + Send + 'static,
>(
    mut stream: R,
    channel:
        Channel<JavaProcessEvent>,
    is_error: bool,
) {
    thread::spawn(move || {
        let mut buffer =
            [0u8; 4096];

        loop {
            match stream.read(
                &mut buffer,
            ) {
                Ok(0) =>
                    break,

                Ok(count) => {
                    let text =
                        String::from_utf8_lossy(
                            &buffer[..count],
                        )
                        .to_string();

                    let event =
                        if is_error {
                            JavaProcessEvent::Stderr(
                                text,
                            )
                        } else {
                            JavaProcessEvent::Stdout(
                                text,
                            )
                        };

                    let _ =
                        channel.send(
                            event,
                        );
                }

                Err(_) =>
                    break,
            }
        }
    });
}

fn run_java_class_blocking(
    class_name: String,
    classpath: String,
    source_code: String,
    interactive: bool,
    on_event:
        Channel<JavaProcessEvent>,
    stdin_state:
        Arc<
            Mutex<
                Option<
                    Arc<
                        Mutex<
                            ChildStdin,
                        >,
                    >,
                >,
            >,
        >,
) -> Result<
    JavaCompileResult,
    String,
> {
    if source_code
        .trim()
        .is_empty()
    {
        return Ok(
            JavaCompileResult {
                compiled:
                    false,
                stdout:
                    String::new(),
                stderr:
                    "Java source code is empty."
                        .to_string(),
                exit_code:
                    Some(1),
            },
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

    if let Err(error) =
        fs::write(
            &source_path,
            source_code.as_bytes(),
        )
    {
        let _ =
            fs::remove_dir_all(
                &run_dir,
            );

        return Err(
            format!(
                "Could not write temporary Java source: {error}"
            ),
        );
    }

    let mut compile =
        Command::new(
            javac_binary(),
        );

    compile
        .arg("-encoding")
        .arg("UTF-8")
        .arg("-Xdiags:verbose")
        .arg("-d")
        .arg(&run_dir);

    if !classpath
        .trim()
        .is_empty()
    {
        compile
            .arg("-cp")
            .arg(&classpath);
    }

    compile.arg(
        &source_path,
    );

    let compile_output =
        match compile.output() {
            Ok(output) =>
                output,

            Err(error) => {
                let _ =
                    fs::remove_dir_all(
                        &run_dir,
                    );

                return Err(
                    format!(
                        "Could not start javac: {error}"
                    ),
                );
            }
        };

    let compile_stdout =
        String::from_utf8_lossy(
            &compile_output
                .stdout,
        )
        .to_string();

    let compile_stderr =
        String::from_utf8_lossy(
            &compile_output
                .stderr,
        )
        .to_string();

    if !compile_output
        .status
        .success()
    {
        let _ =
            fs::remove_dir_all(
                &run_dir,
            );

        return Ok(
            JavaCompileResult {
                compiled:
                    false,
                stdout:
                    compile_stdout,
                stderr:
                    compile_stderr,
                exit_code:
                    compile_output
                        .status
                        .code(),
            },
        );
    }

    let mut runtime_classpath =
        run_dir
            .to_string_lossy()
            .to_string();

    if !classpath
        .trim()
        .is_empty()
    {
        let separator =
            if cfg!(windows) {
                ";"
            } else {
                ":"
            };

        runtime_classpath
            .push_str(
                separator,
            );

        runtime_classpath
            .push_str(
                &classpath,
            );
    }

    let mut child =
        Command::new(
            java_binary(),
        );

    child
        .arg("-cp")
        .arg(
            runtime_classpath,
        )
        .arg(
            &class_name,
        )
        .stdin(
            Stdio::piped(),
        )
        .stdout(
            Stdio::piped(),
        )
        .stderr(
            Stdio::piped(),
        );

    let mut child =
        child
            .spawn()
            .map_err(|e| {
                format!(
                    "Could not start Java: {e}"
                )
            })?;

    let stdin =
        Arc::new(
            Mutex::new(
                child
                    .stdin
                    .take()
                    .ok_or_else(|| {
                        "Could not open Java stdin"
                            .to_string()
                    })?,
            ),
        );

    let stdout =
        child
            .stdout
            .take()
            .ok_or_else(|| {
                "Could not open Java stdout"
                    .to_string()
            })?;

    let stderr =
        child
            .stderr
            .take()
            .ok_or_else(|| {
                "Could not open Java stderr"
                    .to_string()
            })?;

    if let Ok(
        mut slot,
    ) =
        stdin_state.lock()
    {
        *slot =
            Some(
                stdin,
            );
    }

    read_stream(
        stdout,
        on_event.clone(),
        false,
    );

    read_stream(
        stderr,
        on_event.clone(),
        true,
    );

    /*
     * Existing Scanner/input freeze fix
     * preserved.
     */
    thread::spawn(move || {
        let start =
            Instant::now();

        let mut timed_out =
            false;

        let exit_code;

        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    exit_code =
                        status.code();

                    break;
                }

                Ok(None) => {
                    if !interactive
                        && start
                            .elapsed()
                            >= Duration::from_secs(
                                5,
                            )
                    {
                        timed_out =
                            true;

                        let _ =
                            child.kill();

                        exit_code =
                            child
                                .wait()
                                .ok()
                                .and_then(
                                    |status| {
                                        status
                                            .code()
                                    },
                                );

                        break;
                    }

                    thread::sleep(
                        Duration::from_millis(
                            40,
                        ),
                    );
                }

                Err(error) => {
                    let _ =
                        child.kill();

                    let _ =
                        child.wait();

                    if let Ok(
                        mut slot,
                    ) =
                        stdin_state.lock()
                    {
                        *slot =
                            None;
                    }

                    let _ =
                        fs::remove_dir_all(
                            &run_dir,
                        );

                    let _ =
                        on_event.send(
                            JavaProcessEvent::Stderr(
                                format!(
                                    "Could not monitor Java process: {error}"
                                ),
                            ),
                        );

                    let _ =
                        on_event.send(
                            JavaProcessEvent::Exit(
                                JavaExitEvent {
                                    success:
                                        false,
                                    exit_code:
                                        None,
                                    timed_out:
                                        false,
                                },
                            ),
                        );

                    return;
                }
            }
        }

        if let Ok(
            mut slot,
        ) =
            stdin_state.lock()
        {
            *slot =
                None;
        }

        let _ =
            on_event.send(
                JavaProcessEvent::Exit(
                    JavaExitEvent {
                        success:
                            !timed_out
                                && exit_code
                                    == Some(
                                        0,
                                    ),

                        exit_code,

                        timed_out,
                    },
                ),
            );

        let _ =
            fs::remove_dir_all(
                &run_dir,
            );
    });

    Ok(
        JavaCompileResult {
            compiled:
                true,
            stdout:
                String::new(),
            stderr:
                String::new(),
            exit_code:
                None,
        },
    )
}

#[tauri::command]
pub async fn run_java_class(
    class_name: String,
    classpath: String,
    source_code: String,
    interactive: bool,
    on_event:
        Channel<JavaProcessEvent>,
    state:
        State<'_, JavaProcessState>,
) -> Result<
    JavaCompileResult,
    String,
> {
    let stdin_state =
        state.stdin.clone();

    tauri::async_runtime::spawn_blocking(
        move || {
            run_java_class_blocking(
                class_name,
                classpath,
                source_code,
                interactive,
                on_event,
                stdin_state,
            )
        },
    )
    .await
    .map_err(|error| {
        format!(
            "Java execution worker failed: {error}"
        )
    })?
}

#[tauri::command]
pub async fn send_java_input(
    input: String,
    state:
        State<'_, JavaProcessState>,
) -> Result<(), String> {
    let stdin =
        state
            .stdin
            .lock()
            .map_err(|_| {
                "Java stdin lock was poisoned"
                    .to_string()
            })?
            .clone()
            .ok_or_else(|| {
                "No interactive Java program is currently waiting for input."
                    .to_string()
            })?;

    tauri::async_runtime::spawn_blocking(
        move || {
            let mut writer =
                stdin
                    .lock()
                    .map_err(|_| {
                        "Java stdin lock was poisoned"
                            .to_string()
                    })?;

            writer
                .write_all(
                    input.as_bytes(),
                )
                .and_then(
                    |_| {
                        writer.write_all(
                            b"\n",
                        )
                    },
                )
                .and_then(
                    |_| {
                        writer.flush()
                    },
                )
                .map_err(
                    |e| {
                        format!(
                            "Could not send Java input: {e}"
                        )
                    },
                )
        },
    )
    .await
    .map_err(|error| {
        format!(
            "Java input worker failed: {error}"
        )
    })?
}