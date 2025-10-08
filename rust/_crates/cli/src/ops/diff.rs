use std::env;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use leaky_common::prelude::*;

use super::utils;
use crate::change_log::{ChangeLog, ChangeType};

pub async fn diff(base: &mut ChangeLog) -> Result<ChangeLog, DiffError> {
    // Get current working directory for relative path handling
    let current_dir = env::current_dir().map_err(DiffError::Io)?;

    let mut update = base.clone();
    let next = utils::fs_tree()?;
    let default_hash = Cid::default();
    let ipfs = IpfsRpc::default();

    // When comparing paths, make them relative to the current directory
    // This ensures path comparisons work correctly regardless of where the command is run from
    let make_relative = |path: &Path| -> PathBuf {
        if path.is_absolute() {
            path.strip_prefix(&current_dir)
                .unwrap_or(path)
                .to_path_buf()
        } else {
            path.to_path_buf()
        }
    };

    // Insert the root directory hash into the regular changes for comparison
    base.insert(
        make_relative(&PathBuf::from("")),
        (default_hash, ChangeType::Base { last_check: None }),
    );

    // Iterate over the path-sorted change_log and the fs-tree in order to diff
    let mut base_iter = base
        .iter()
        .map(|(path, (hash, change))| (path.clone(), (hash, change)));
    let mut next_iter = next.iter();

    let mut next_next = next_iter.next();
    let mut base_next = base_iter.next();
    loop {
        match (next_next.clone(), base_next.clone()) {
            // If these are both something we got some work to do
            (Some((next_tree, next_path)), Some((base_path, (base_hash, base_type)))) => {
                // For each object, assuming we stay aligned on a sorted list of paths:
                // If the base comes before then this file was removed
                if make_relative(&base_path) < make_relative(&next_path) {
                    if !base_path.is_dir() {
                        match base_type {
                            ChangeType::Added { .. } => {
                                update.remove(&base_path);
                            }
                            ChangeType::Modified { .. } | ChangeType::Base { .. } => {
                                update.insert(
                                    base_path.clone(),
                                    (default_hash, ChangeType::Removed { processed: false }),
                                );
                            }
                            // not our responsibility to process removed
                            ChangeType::Removed { .. } => {}
                        }
                    }
                    base_next = base_iter.next();
                    continue;
                }

                // If next comes before base then the file was added
                if make_relative(&next_path) < make_relative(&base_path) {
                    if !next_path.is_dir() {
                        let hash = utils::hash_file(&next_path, &ipfs, None).await?;
                        update.insert(
                            next_path.clone(),
                            (
                                hash,
                                ChangeType::Added {
                                    modified: true,
                                    last_check: Some(SystemTime::now()),
                                },
                            ),
                        );
                    }
                    next_next = next_iter.next();
                    continue;
                }

                // If they are equal then we are good. Move on to the next objects
                if make_relative(&next_path) == make_relative(&base_path) {
                    // These are either both files or both directories
                    // If they are both files then we need to compare hashes
                    if !next_tree.is_dir() {
                        // Get the last check time from the existing change type
                        let last_check = match base_type {
                            ChangeType::Added { last_check, .. } => *last_check,
                            ChangeType::Modified { last_check, .. } => *last_check,
                            ChangeType::Base { last_check } => *last_check,
                            _ => None,
                        };

                        let next_hash =
                            utils::hash_file(&next_path, &ipfs, Some((base_hash, last_check)))
                                .await?;

                        if base_hash != &next_hash {
                            let change_type = match base_type {
                                ChangeType::Added { .. } => ChangeType::Added {
                                    modified: true,
                                    last_check: Some(SystemTime::now()),
                                },
                                _ => ChangeType::Modified {
                                    processed: false,
                                    last_check: Some(SystemTime::now()),
                                },
                            };

                            update.insert(base_path.clone(), (next_hash, change_type));
                        }
                    }

                    next_next = next_iter.next();
                    base_next = base_iter.next();
                    continue;
                }
            }

            // Theres more new files than old, so this file was added
            (Some((next_tree, next_path)), None) => {
                if !next_tree.is_dir() {
                    let hash = utils::hash_file(&next_path, &ipfs, None).await?;
                    update.insert(
                        next_path.clone(),
                        (
                            hash,
                            ChangeType::Added {
                                modified: true,
                                last_check: Some(SystemTime::now()),
                            },
                        ),
                    );
                }
                next_next = next_iter.next();
                continue;
            }

            // There's more old files than new, so this file was removed
            (None, Some((base_path, (_base_hash, base_type)))) => {
                if !base_path.is_dir() {
                    match base_type {
                        ChangeType::Added { .. } => {
                            update.remove(&base_path);
                        }
                        ChangeType::Modified { .. } | ChangeType::Base { .. } => {
                            update.insert(
                                base_path.clone(),
                                (default_hash, ChangeType::Removed { processed: false }),
                            );
                        }
                        // not our responsibility to process removed
                        ChangeType::Removed { .. } => {}
                    }
                }
                base_next = base_iter.next();
                continue;
            }
            (None, None) => {
                // We are done
                break;
            }
        }
    }

    Ok(update)
}

#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    #[error("default error: {0}")]
    Default(#[from] anyhow::Error),
    #[error("could not read change_log: {0}")]
    ReadChanges(#[from] serde_json::Error),
    #[error("fs-tree error: {0}")]
    FsTree(#[from] fs_tree::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("leaky error: {0}")]
    Mount(#[from] MountError),
    #[error("file does not exist")]
    PathDoesNotExist(PathBuf),
    #[error("path is a directory")]
    PathIsDirectory(PathBuf),
}
