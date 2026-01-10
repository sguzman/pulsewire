use std::path::{Path, PathBuf};

// Mimics Scala's "if path is under resources, base is CWD else config parent".
pub(crate) fn resolve_db_base_dir(config_path: &Path) -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let path_str = config_path.to_string_lossy();
    if path_str.contains("resources") {
        cwd
    } else {
        config_path.parent().unwrap_or(&cwd).to_path_buf()
    }
}

pub(crate) fn resolve_log_dir(config_path: &Path, log_dir: &str) -> PathBuf {
    let p = Path::new(log_dir);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(p)
}
