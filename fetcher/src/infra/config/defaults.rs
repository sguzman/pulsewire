use super::ConfigError;

pub(crate) fn default_pg_user() -> String {
    "admin".to_string()
}

pub(crate) fn default_pg_password() -> String {
    "admin".to_string()
}

pub(crate) fn default_pg_host() -> String {
    "localhost".to_string()
}

pub(crate) fn default_pg_port() -> u16 {
    5432
}

pub(crate) fn default_pg_database() -> String {
    "data".to_string()
}

pub(crate) fn default_sqlite_path() -> String {
    "rss.db".to_string()
}

pub(crate) fn default_log_file_directory() -> String {
    "logs".to_string()
}

pub(crate) fn default_log_file_name() -> String {
    "feedrv3".to_string()
}

pub(crate) fn default_log_file_rotation() -> String {
    "hourly".to_string()
}

pub(crate) fn default_log_file_level() -> String {
    "info".to_string()
}

pub(crate) fn default_log_file_enabled() -> bool {
    false
}

pub(crate) fn default_metrics_enabled() -> bool {
    false
}

pub(crate) fn default_metrics_bind() -> String {
    "0.0.0.0:9898".to_string()
}
pub(crate) fn default_log_tick_warn_seconds() -> u64 {
    600
}

pub(crate) fn default_log_feed_timing_warn_ms() -> u64 {
    30_000
}

pub(crate) fn default_max_consecutive_errors() -> u32 {
    5
}

pub(crate) fn default_immediate_error_statuses() -> Vec<u16> {
    vec![404]
}

pub(crate) fn normalize_log_level(level: &str) -> Result<String, ConfigError> {
    let l = level.trim().to_ascii_lowercase();
    if l.is_empty() {
        return Err(ConfigError::Invalid("logging.file_level cannot be empty".into()));
    }
    match l.as_str() {
        "error" | "warn" | "info" | "debug" | "trace" | "off" => Ok(l),
        _ => Err(ConfigError::Invalid(format!(
            "invalid logging.file_level '{level}', expected error|warn|info|debug|trace|off"
        ))),
    }
}

pub(crate) fn normalize_log_rotation(rotation: &str) -> Result<String, ConfigError> {
    let r = rotation.trim().to_ascii_lowercase();
    if r.is_empty() {
        return Err(ConfigError::Invalid(
            "logging.file_rotation cannot be empty".into(),
        ));
    }
    match r.as_str() {
        "hourly" => Ok(r),
        _ => Err(ConfigError::Invalid(format!(
            "invalid logging.file_rotation '{rotation}', expected 'hourly'"
        ))),
    }
}

pub(crate) fn normalize_domains(domains: &[String]) -> Result<Vec<String>, ConfigError> {
    let mut normalized = Vec::with_capacity(domains.len());
    for d in domains {
        let domain = d.trim().to_ascii_lowercase();
        if domain.is_empty() {
            return Err(ConfigError::Invalid(
                "logging.feed_timing_domains contains empty domain".into(),
            ));
        }
        normalized.push(domain);
    }
    Ok(normalized)
}

pub(crate) fn normalize_status_codes(codes: &[u16]) -> Result<Vec<u16>, ConfigError> {
    let mut normalized = Vec::with_capacity(codes.len());
    for &code in codes {
        if !(100..=599).contains(&code) {
            return Err(ConfigError::Invalid(format!(
            "invalid backoff.immediate_error_statuses code '{code}', expected 100-599"
        )));
        }
        normalized.push(code);
    }
    Ok(normalized)
}
