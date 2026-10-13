use std::path::PathBuf;

pub fn python_executable() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../compute-worker");
    let executable = std::env::var_os("SUNA_COMPUTE_WORKER_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            root.join(if cfg!(windows) {
                ".venv/Scripts/python.exe"
            } else {
                ".venv/bin/python"
            })
        });
    assert!(
        executable.is_file(),
        "Install the locked compute-worker test environment or set SUNA_COMPUTE_WORKER_PYTHON"
    );
    executable
}
