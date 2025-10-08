use std::fmt::Display;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;

use leaky_common::prelude::*;

use crate::change_log::ChangeType;
use crate::ops::EditableObject;
use crate::{AppState, Op};

use super::diff::{diff, DiffError};

#[derive(Debug, clap::Args, Clone)]
pub struct Add {
    #[clap(short, long)]
    pub verbose: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum AddError {
    #[error("default error: {0}")]
    Default(#[from] anyhow::Error),
    #[error("encountered mismatched cid: {0} != {1}")]
    CidMismatch(Cid, Cid),
    #[error("fs-tree error: {0}")]
    FsTree(#[from] fs_tree::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not parse diff: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("could not strip prefix: {0}")]
    PathPrefix(#[from] std::path::StripPrefixError),
    #[error("diff error: {0}")]
    Diff(#[from] DiffError),
    #[error("mount error: {0}")]
    Mount(#[from] MountError),
    #[error("api error: {0}")]
    Api(#[from] leaky_common::error::ApiError),
    #[error("app state error: {0}")]
    AppState(#[from] crate::state::AppStateSetupError),
    #[error("invalid schema file: {0}")]
    InvalidSchema(String),
}

fn abs_path(path: &PathBuf) -> Result<PathBuf, DiffError> {
    let path = PathBuf::from("/").join(path);
    Ok(path)
}

#[derive(Debug)]
pub struct AddOutput {
    pub previous_cid: Cid,
    pub cid: Cid,
}

impl Display for AddOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.previous_cid == self.cid {
            write!(f, "No changes to add")
        } else {
            write!(f, "{} -> {}", self.previous_cid, self.cid)
        }
    }
}

#[async_trait]
impl Op for Add {
    type Error = AddError;
    type Output = AddOutput;

    async fn execute(&self, state: &AppState) -> Result<Self::Output, Self::Error> {
        let mut client = state.client()?;
        let cid = *state.cid();
        let mut change_log = state.change_log().clone();
        let ipfs_rpc = Arc::new(client.ipfs_rpc()?);
        let mut mount = Mount::pull(cid, &ipfs_rpc).await?;
        let mut updates = diff(&mut change_log).await?;
        let schema_updates = updates.schema().clone();
        let schema_change_log_iter = schema_updates.iter().map(|(path, (hash, change))| {
            let abs_path = abs_path(path).unwrap();
            (path.clone(), abs_path, (hash, change))
        });
        let object_updates = updates.object().clone();
        let object_change_log_iter = object_updates.iter().map(|(path, (hash, change))| {
            let abs_path = abs_path(path).unwrap();
            (path.clone(), abs_path, (hash, change))
        });
        let regular_updates = updates.regular().clone();
        let change_log_iter = regular_updates.iter().map(|(path, (hash, change))| {
            let abs_path = abs_path(path).unwrap();
            (path.clone(), abs_path, (hash, change))
        });

        // Track which files have been removed so we can handle object file cleanup gracefully
        let mut removed_files: std::collections::HashSet<PathBuf> =
            std::collections::HashSet::new();

        // First pass - handle schemas
        for (path, abs_path, (hash, diff_type)) in schema_change_log_iter {
            match diff_type {
                ChangeType::Added { modified: true, .. } => {
                    let contents = std::fs::read_to_string(path.clone())?;

                    let json = contents
                        .split("```json\n")
                        .nth(1)
                        .and_then(|s| s.split("\n```").next())
                        .ok_or_else(|| {
                            AddError::InvalidSchema("Invalid schema file format".to_string())
                        })?;

                    let schema: Schema = serde_json::from_str(json)?;
                    mount.set_schema(abs_path.parent().unwrap(), schema).await?;

                    if self.verbose {
                        println!(
                            " -> setting schema @ {}",
                            abs_path.parent().unwrap().display()
                        );
                    }

                    updates.insert(
                        path.clone(),
                        (
                            *hash,
                            ChangeType::Added {
                                modified: false,
                                last_check: Some(SystemTime::now()),
                            },
                        ),
                    );
                }
                ChangeType::Modified {
                    processed: false, ..
                } => {
                    let contents = std::fs::read_to_string(path.clone())?;

                    let json = contents
                        .split("```json\n")
                        .nth(1)
                        .and_then(|s| s.split("\n```").next())
                        .ok_or_else(|| {
                            AddError::InvalidSchema("Invalid schema file format".to_string())
                        })?;

                    let schema: Schema = serde_json::from_str(json)?;
                    mount.set_schema(abs_path.parent().unwrap(), schema).await?;

                    if self.verbose {
                        println!(
                            " -> updating schema @ {}",
                            abs_path.parent().unwrap().display()
                        );
                    }

                    updates.insert(
                        path.clone(),
                        (
                            *hash,
                            ChangeType::Modified {
                                processed: true,
                                last_check: Some(SystemTime::now()),
                            },
                        ),
                    );
                }
                ChangeType::Removed {
                    processed: false, ..
                } => {
                    mount.unset_schema(abs_path.parent().unwrap()).await?;

                    if self.verbose {
                        println!(
                            " -> removing schema @ {}",
                            abs_path.parent().unwrap().display()
                        );
                    }

                    updates.insert(
                        path.clone(),
                        (*hash, ChangeType::Removed { processed: true }),
                    );
                }
                _ => {}
            }
        }

        // Second pass - handle regular files
        for (path, abs_path, (hash, diff_type)) in change_log_iter {
            let path_clone = path.clone();
            match diff_type {
                ChangeType::Added { modified: true, .. } => {
                    // read the file and add it to the fucking mount

                    let file = File::open(path)?;
                    if self.verbose {
                        println!(" -> adding file @ {}", abs_path.display());
                    }
                    mount.add(&abs_path, (file, false)).await?;
                    updates.insert(
                        path_clone,
                        (
                            *hash,
                            ChangeType::Added {
                                modified: false,
                                last_check: Some(SystemTime::now()),
                            },
                        ),
                    );
                }
                ChangeType::Modified {
                    processed: false, ..
                } => {
                    // read the file and add it to the fucking mount
                    let file = File::open(path)?;
                    if self.verbose {
                        println!(" -> updating file @ {}", abs_path.display());
                    }
                    mount.add(&abs_path, (file, false)).await?;
                    updates.insert(
                        path_clone,
                        (
                            *hash,
                            ChangeType::Modified {
                                processed: true,
                                last_check: Some(SystemTime::now()),
                            },
                        ),
                    );
                }
                ChangeType::Removed {
                    processed: false, ..
                } => {
                    mount.rm(&abs_path).await?;
                    if self.verbose {
                        println!(" -> removing file @ {}", abs_path.display());
                    }

                    // Track that we removed this file
                    removed_files.insert(abs_path.clone());

                    // Remove the .obj file from the filesystem if it exists
                    let obj_path = path.with_extension("obj.md");
                    if obj_path.exists() {
                        std::fs::remove_file(&obj_path)?;
                        if self.verbose {
                            println!(" -> removing object file @ {}", obj_path.display());
                        }
                    }

                    updates.insert(path_clone, (*hash, ChangeType::Removed { processed: true }));
                }
                _ => {}
            }
        }

        // Third pass - handle objects
        for (path, abs_path, (hash, diff_type)) in object_change_log_iter {
            match diff_type {
                ChangeType::Added { modified: true, .. } => {
                    let contents = std::fs::read_to_string(path.clone())?;

                    let json = contents
                        .split("```json\n")
                        .nth(1)
                        .and_then(|s| s.split("\n```").next())
                        .ok_or_else(|| {
                            AddError::InvalidSchema("Invalid object file format".to_string())
                        })?;

                    let editable: EditableObject = serde_json::from_str(json)?;
                    let object: Object = editable.into();

                    let target_path = PathBuf::from("/").join(
                        abs_path.parent().unwrap().join(
                            abs_path
                                .file_stem()
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .strip_suffix(".obj")
                                .unwrap(),
                        ),
                    );

                    mount.tag(&target_path, object).await?;

                    if self.verbose {
                        println!(" -> adding tag @ {}", target_path.display());
                    }

                    updates.insert(
                        path.clone(),
                        (
                            *hash,
                            ChangeType::Added {
                                modified: false,
                                last_check: Some(SystemTime::now()),
                            },
                        ),
                    );
                }
                ChangeType::Modified {
                    processed: false, ..
                } => {
                    let contents = std::fs::read_to_string(path.clone())?;

                    let json = contents
                        .split("```json\n")
                        .nth(1)
                        .and_then(|s| s.split("\n```").next())
                        .ok_or_else(|| {
                            AddError::InvalidSchema("Invalid object file format".to_string())
                        })?;

                    let editable: EditableObject = serde_json::from_str(json)?;
                    let object: Object = editable.into();

                    let target_path = PathBuf::from("/").join(
                        abs_path.parent().unwrap().join(
                            abs_path
                                .file_stem()
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .strip_suffix(".obj")
                                .unwrap(),
                        ),
                    );

                    let created_at = object.created_at();
                    let updated_at = object.updated_at();
                    if self.verbose {
                        println!(
                            " -> updating tag @ {} (created: {}, updated: {})",
                            target_path.display(),
                            created_at,
                            updated_at
                        );
                    }

                    mount.tag(&target_path, object).await?;

                    updates.insert(
                        path.clone(),
                        (
                            *hash,
                            ChangeType::Modified {
                                processed: true,
                                last_check: Some(SystemTime::now()),
                            },
                        ),
                    );
                }
                ChangeType::Removed {
                    processed: false, ..
                } => {
                    let target_path = PathBuf::from("/").join(
                        abs_path.parent().unwrap().join(
                            abs_path
                                .file_stem()
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .strip_suffix(".obj")
                                .unwrap(),
                        ),
                    );

                    // If the target file was already removed, we can skip removing the tag
                    // since it would have been removed along with the file
                    if removed_files.contains(&target_path) {
                        if self.verbose {
                            println!(
                                " -> skipping tag removal for {} (target file already removed)",
                                target_path.display()
                            );
                        }
                    } else {
                        mount.rm_tag(&target_path).await?;
                        if self.verbose {
                            println!(" -> removing tag @ {}", target_path.display());
                        }
                    }

                    updates.insert(
                        path.clone(),
                        (*hash, ChangeType::Removed { processed: true }),
                    );
                }
                _ => {}
            }
        }

        // TODO: we really shouldn't need to push here
        //  I think the reason we are is so that we can persist
        //  the changes to the mount soooomewhere
        //  Ideally we should be able to write the current state of the mount
        //  locally and only push when we want to
        mount.push().await?;
        let new_cid = *mount.cid();

        state.save(&mount, Some(&updates), None)?;

        if new_cid == cid {
            return Ok(AddOutput {
                previous_cid: cid,
                cid: new_cid,
            });
        }

        Ok(AddOutput {
            previous_cid: cid,
            cid: new_cid,
        })
    }
}
