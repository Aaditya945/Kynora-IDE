use std::{
    fs,
    path::PathBuf,
    process::{
        Child,
        ChildStderr,
        ChildStdin,
        ChildStdout,
        Command,
        Stdio,
    },
    sync::{
        Arc,
        Mutex,
    },
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use tauri::{
    AppHandle,
    Manager,
};

use crate::jdtls_runtime::{
    create_data_directory,
    ensure_java21,
    find_jdtls_directory,
    find_launcher_jar,
    locate_java_binary,
};

use crate::jdtls_trace::{
    trace_enabled,
    trace_log,
};

pub(crate) struct JdtlsProcess {
    pub(crate) root:
        PathBuf,

    pub(crate) data_dir:
        PathBuf,

    pub(crate) java:
        PathBuf,

    pub(crate) child:
        Child,

    pub(crate) stdin:
        Arc<Mutex<ChildStdin>>,

    pub(crate) stdout:
        ChildStdout,

    pub(crate) stderr:
        ChildStderr,
}

pub(crate) fn start(
    app:
        &AppHandle,
) -> Result<JdtlsProcess, String> {
    let workspace_base =
        app.path()
            .app_local_data_dir()
            .map_err(|e| {
                format!(
                    "Could not locate Kynora local data directory: {e}"
                )
            })?
            .join(
                "workspace",
            );

    fs::create_dir_all(
        &workspace_base,
    )
    .map_err(|e| {
        format!(
            "Could not create Kynora workspace directory: {e}"
        )
    })?;

    let root =
        workspace_base
            .join(
                "default-java",
            )
            .canonicalize()
            .unwrap_or_else(
                |_| {
                    workspace_base
                        .join(
                            "default-java",
                        )
                },
            );

    fs::create_dir_all(
        &root,
    )
    .map_err(|e| {
        format!(
            "Could not create JDTLS project workspace: {e}"
        )
    })?;

    let main_file =
        root.join(
            "Main.java",
        );

    if !main_file.exists() {
        fs::write(
            &main_file,
            b"\n",
        )
        .map_err(|e| {
            format!(
                "Could not create JDTLS Main.java shadow file: {e}"
            )
        })?;
    }

    let project_file =
        root.join(
            ".project",
        );

    if !project_file.exists() {
        let project_xml =
r#"<?xml version="1.0" encoding="UTF-8"?>
<projectDescription>
    <name>KynoraDefaultJavaProject</name>
    <comment></comment>
    <projects>
    </projects>
    <buildSpec>
        <buildCommand>
            <name>org.eclipse.jdt.core.javabuilder</name>
            <arguments>
            </arguments>
        </buildCommand>
    </buildSpec>
    <natures>
        <nature>org.eclipse.jdt.core.javanature</nature>
    </natures>
</projectDescription>
"#;

        fs::write(
            &project_file,
            project_xml.as_bytes(),
        )
        .map_err(|e| {
            format!(
                "Could not create JDTLS .project file: {e}"
            )
        })?;
    }

    let classpath_file =
        root.join(
            ".classpath",
        );

    if !classpath_file.exists() {
        let classpath_xml =
r#"<?xml version="1.0" encoding="UTF-8"?>
<classpath>
    <classpathentry kind="src" path=""/>
    <classpathentry kind="con" path="org.eclipse.jdt.launching.JRE_CONTAINER/org.eclipse.jdt.internal.debug.ui.launcher.StandardVMType/JavaSE-21"/>
    <classpathentry kind="output" path="bin"/>
</classpath>
"#;

        fs::write(
            &classpath_file,
            classpath_xml.as_bytes(),
        )
        .map_err(|e| {
            format!(
                "Could not create JDTLS .classpath file: {e}"
            )
        })?;
    }

    let jdtls_dir =
        find_jdtls_directory(
            app,
        )?;

    let launcher =
        find_launcher_jar(
            &jdtls_dir,
        )?;

    let java =
        locate_java_binary()?;

    ensure_java21(
        &java,
    )?;

    let data_dir =
        create_data_directory(
            app,
            &root,
        )?;

    trace_log(format!(
        "JDTLS PATHS: root={}",
        root.display()
    ));

    trace_log(format!(
        "JDTLS PATHS: data={}",
        data_dir.display()
    ));

    trace_log(format!(
        "JDTLS PATHS: jdtls={}",
        jdtls_dir.display()
    ));

    trace_log(format!(
        "JDTLS PATHS: launcher={}",
        launcher.display()
    ));

    trace_log(format!(
        "JDTLS PATHS: java={}",
        java.display()
    ));

    trace_log(format!(
        "JDTLS PATHS: config={}",
        jdtls_dir
            .join(
                "config_win",
            )
            .display()
    ));

    let mut command =
        Command::new(
            &java,
        );

    command
        .current_dir(
            &jdtls_dir,
        )
        .arg(
            "-Declipse.application=org.eclipse.jdt.ls.core.id1",
        )
        .arg(
            "-Dosgi.bundles.defaultStartLevel=4",
        )
        .arg(
            "-Declipse.product=org.eclipse.jdt.ls.core.product",
        )
        .arg(
            "-Dlog.protocol=false",
        )
        .arg(
            "-Dlog.level=WARNING",
        )
        .arg(
            "-Xms256m",
        )
        .arg(
            "-Xmx1G",
        )
        .arg(
            "--add-modules=ALL-SYSTEM",
        )
        .arg(
            "--add-opens",
        )
        .arg(
            "java.base/java.util=ALL-UNNAMED",
        )
        .arg(
            "--add-opens",
        )
        .arg(
            "java.base/java.lang=ALL-UNNAMED",
        )
        .arg(
            "-jar",
        )
        .arg(
            &launcher,
        )
        .arg(
            "-configuration",
        )
        .arg(
            jdtls_dir.join(
                "config_win",
            ),
        )
        .arg(
            "-data",
        )
        .arg(
            &data_dir,
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

    trace_log(format!(
        "JDTLS PROCESS: cwd={}",
        jdtls_dir.display()
    ));

    if trace_enabled() {
        trace_log(format!(
            "JDTLS PROCESS: command={command:?}"
        ));
    }

    #[cfg(windows)]
    command.creation_flags(
        0x0800_0000,
    );

    let mut child =
        command
            .spawn()
            .map_err(|e| {
                format!(
                    "Failed to start JDTLS: {e}"
                )
            })?;

    crate::jdtls_trace::emit_log(
        app,
        format!(
            "JDTLS started (pid={})",
            child.id()
        ),
    );

    let stdin =
        Arc::new(
            Mutex::new(
                child
                    .stdin
                    .take()
                    .ok_or_else(|| {
                        "Failed to open JDTLS stdin"
                            .to_string()
                    })?,
            ),
        );

    let stdout =
        child
            .stdout
            .take()
            .ok_or_else(|| {
                "Failed to open JDTLS stdout"
                    .to_string()
            })?;

    let stderr =
        child
            .stderr
            .take()
            .ok_or_else(|| {
                "Failed to open JDTLS stderr"
                    .to_string()
            })?;

    Ok(
        JdtlsProcess {
            root,
            data_dir,
            java,
            child,
            stdin,
            stdout,
            stderr,
        },
    )
}