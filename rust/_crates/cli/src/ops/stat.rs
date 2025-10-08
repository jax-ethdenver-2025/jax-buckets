use std::fmt::Display;

use async_trait::async_trait;

use crate::change_log::{ChangeLog, ChangeType};
use crate::{AppState, Op};

#[derive(Debug, clap::Args, Clone)]
pub struct Stat {
    #[clap(short, long)]
    pub verbose: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum StatError {
    #[error("default error: {0}")]
    Default(#[from] anyhow::Error),
    #[error("app state error: {0}")]
    AppState(#[from] crate::state::AppStateSetupError),
}

#[derive(Debug)]
pub struct StatOutput {
    pub change_log: ChangeLog,
    pub verbose: bool,
}

impl Display for StatOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        let mut changes = false;
        for (path, (_hash, diff_type)) in self.change_log.iter() {
            if !self.verbose && matches!(diff_type, ChangeType::Base { .. }) {
                continue;
            }
            changes = true;
            // don't prefix the first line with a newline
            if s.is_empty() {
                s.push_str(&format!("{}: {}", path.to_str().unwrap(), diff_type));
            } else {
                s.push_str(&format!("\n{}: {}", path.to_str().unwrap(), diff_type));
            }
        }
        if !changes && !self.verbose {
            s.push_str("No changes");
        }
        write!(f, "{}", s)
    }
}

#[async_trait]
impl Op for Stat {
    type Error = StatError;
    type Output = StatOutput;

    async fn execute(&self, state: &AppState) -> Result<Self::Output, Self::Error> {
        Ok(StatOutput {
            change_log: state.change_log().clone(),
            verbose: self.verbose,
        })
    }
}
