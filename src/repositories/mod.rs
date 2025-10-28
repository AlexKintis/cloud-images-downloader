pub mod debian;
mod models;
pub mod ubuntu;

use std::{fs, path::Path, sync::OnceLock};

pub use models::Repository; // Re-export the model type to callers.

/// Single, module-private cache (set exactly once).
static CACHE: OnceLock<Vec<Repository>> = OnceLock::new();

// ---- Public API (serde hidden from callers) ----

/// Initialize from a JSON file path.
#[allow(unused)]
pub fn init_from_file(path: impl AsRef<Path>) -> Result<(), ReposError> {
    let data = fs::read_to_string(path).map_err(ReposError::Io)?;
    init_from_json_str(&data)
}

/// Initialize from a JSON string.
#[allow(unused)]
pub fn init_from_json_str(json: &str) -> Result<(), ReposError> {
    let parsed: Vec<Repository> = serde_json::from_str(json).map_err(ReposError::Json)?;
    CACHE
        .set(parsed)
        .map_err(|_| ReposError::AlreadyInitialized)?;
    Ok(())
}

/// Initialize from an env var containing JSON.
#[allow(unused)]
pub fn init_from_env(var: &str) -> Result<(), ReposError> {
    let s = std::env::var(var).map_err(|_| ReposError::MissingEnv(var.to_string()))?;
    init_from_json_str(&s)
}

/// Return an owned `Vec<Repository>` (as requested).
///
/// # Example
/// ```ignore
/// let repos_vec = repos::all_owned()?;
/// ```
#[allow(unused)]
pub fn all_owned() -> Result<Vec<Repository>, ReposError> {
    Ok(CACHE.get().ok_or(ReposError::NotInitialized)?.clone())
}

/// Borrowing alternative to avoid cloning.
#[allow(unused)]
pub fn all() -> Result<&'static [Repository], ReposError> {
    CACHE
        .get()
        .map(|v| v.as_slice())
        .ok_or(ReposError::NotInitialized)
}

/// Optional: find by name without cloning.
#[allow(unused)]
pub fn by_name(name: &str) -> Result<Option<&'static Repository>, ReposError> {
    let repos = CACHE.get().ok_or(ReposError::NotInitialized)?;
    Ok(repos.iter().find(|r| r.name() == name))
}

/// ---- Errors ----
#[derive(thiserror::Error, Debug)]
pub enum ReposError {
    #[error("repositories are not initialized")]
    NotInitialized,
    #[error("repositories already initialized")]
    AlreadyInitialized,
    #[error("missing env var: {0}")]
    MissingEnv(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
