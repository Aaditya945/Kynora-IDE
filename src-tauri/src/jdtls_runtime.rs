use std::{
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
};

use tauri::{
    AppHandle,
    Manager,
};

pub(crate) fn find_jdtls_directory(
    app:
        &AppHandle,
) -> Result<PathBuf, String> {
    let cwd =
        std::env::current_dir()
            .map_err(|e| {
                format!(
                    "Could not read Kynora working directory: {e}"
                )
            })?;

    let mut current =
        Some(cwd.clone());

    while let Some(dir) =
        current
    {
        let candidate =
            dir.join("jdtls");

        if candidate
            .join("plugins")
            .is_dir()
        {
            return Ok(candidate);
        }

        current =
            dir.parent()
                .map(
                    PathBuf::from,
                );
    }

    let resource_path =
        app.path()
            .resource_dir()
            .map_err(|e| {
                format!(
                    "Could not locate Kynora resource directory: {e}"
                )
            })?
            .join("jdtls");

    if resource_path
        .join("plugins")
        .is_dir()
    {
        return Ok(
            resource_path,
        );
    }

    Err(format!(
        "JDTLS folder was not found. Searched from {} and packaged resources.",
        cwd.display()
    ))
}

pub(crate) fn find_launcher_jar(
    jdtls_dir:
        &Path,
) -> Result<PathBuf, String> {
    let plugins =
        jdtls_dir.join(
            "plugins",
        );

    let entries =
        fs::read_dir(
            &plugins,
        )
        .map_err(|e| {
            format!(
                "Could not read JDTLS plugins directory: {e}"
            )
        })?;

    let mut matches =
        entries
            .filter_map(
                Result::ok,
            )
            .map(
                |entry| {
                    entry.path()
                },
            )
            .filter(
                |path| {
                    path.file_name()
                        .and_then(
                            |name|
                                name.to_str(),
                        )
                        .map(
                            |name| {
                                name.starts_with(
                                    "org.eclipse.equinox.launcher_",
                                )
                                && name.ends_with(
                                    ".jar",
                                )
                            },
                        )
                        .unwrap_or(
                            false,
                        )
                },
            )
            .collect::<Vec<_>>();

    matches.sort();

    matches
        .into_iter()
        .next()
        .ok_or_else(|| {
            "Could not find org.eclipse.equinox.launcher_*.jar in JDTLS/plugins"
                .to_string()
        })
}

pub(crate) fn locate_java_binary()
    -> Result<PathBuf, String>
{
    if let Ok(java_home) =
        std::env::var(
            "JAVA_HOME",
        )
    {
        let candidate =
            PathBuf::from(
                java_home,
            )
            .join("bin")
            .join(
                if cfg!(windows) {
                    "java.exe"
                } else {
                    "java"
                },
            );

        if candidate.exists() {
            return Ok(
                candidate,
            );
        }
    }

    Ok(
        PathBuf::from(
            if cfg!(windows) {
                "java.exe"
            } else {
                "java"
            },
        ),
    )
}

pub(crate) fn ensure_java21(
    java:
        &Path,
) -> Result<(), String> {
    let output =
        Command::new(
            java,
        )
        .arg("-version")
        .output()
        .map_err(|e| {
            format!(
                "Could not run Java to check its version: {e}"
            )
        })?;

    let combined =
        String::from_utf8_lossy(
            &output.stderr,
        )
        .to_string()
        + &String::from_utf8_lossy(
            &output.stdout,
        );

    let major =
        combined
            .split_whitespace()
            .find_map(
                |token| {
                    if token.starts_with(
                        '"',
                    ) {
                        let version =
                            token
                                .trim_matches(
                                    '"',
                                );

                        let first =
                            version
                                .split('.')
                                .next()?;

                        first
                            .parse::<u32>()
                            .ok()
                    } else {
                        None
                    }
                },
            )
            .unwrap_or(
                0,
            );

    if major < 21 {
        return Err(format!(
            "JDTLS needs Java 21 or newer, but Kynora found Java {major}."
        ));
    }

    Ok(())
}

pub(crate) fn workspace_hash(
    root:
        &Path,
) -> String {
    let mut hasher =
        std::collections::hash_map::DefaultHasher::new();

    root.to_string_lossy()
        .hash(
            &mut hasher,
        );

    format!(
        "{:016x}",
        hasher.finish()
    )
}

pub(crate) fn create_data_directory(
    app:
        &AppHandle,
    root:
        &Path,
) -> Result<PathBuf, String> {
    let base =
        app.path()
            .app_local_data_dir()
            .map_err(|e| {
                format!(
                    "Could not locate Kynora local data directory: {e}"
                )
            })?
            .join(
                "jdtls",
            )
            .join(
                "workspaces",
            );

    let data_dir =
        base.join(
            workspace_hash(
                root,
            ),
        );

    fs::create_dir_all(
        &data_dir,
    )
    .map_err(|e| {
        format!(
            "Could not create JDTLS workspace data directory: {e}"
        )
    })?;

    Ok(data_dir)
}