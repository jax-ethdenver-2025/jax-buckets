//! Common error modules for the LeakyCli

/// A common catch all minimal error type for the Blossom library.
#[derive(Debug, thiserror::Error)]
pub enum LeakyCliError {}

/// Convenience type for any fallible method that can produce a [`BlossomError`].
pub type LeakyCliResult<T> = Result<T, LeakyCliError>;
