use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;

use leaky_common::prelude::*;

use crate::change_log::ChangeLog;
use crate::change_log::ChangeType;
use crate::ops::EditableObject;
use crate::{AppState, Op};

use super::utils;

#[derive(Debug, clap::Args, Clone)]
pub struct Pull;

#[derive(Debug, thiserror::Error)]
pub enum PullError {
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
    #[error("mount error: {0}")]
    Mount(#[from] MountError),
    #[error("api error: {0}")]
    Api(#[from] leaky_common::error::ApiError),
    #[error("app state error: {0}")]
    AppState(#[from] crate::state::AppStateSetupError),
    #[error("path is a directory: {0}")]
    PathIsDirectory(PathBuf),
}

// TODO: known error that a file not being updated will not trigger a pull
//  of it's metadata

#[async_trait]
impl Op for Pull {
    type Error = PullError;
    type Output = Cid;

    async fn execute(&self, state: &AppState) -> Result<Self::Output, Self::Error> {
        let mut client = state.client()?;
        let pull_root_req = PullRoot {};
        let root_cid = client.call(pull_root_req).await?;
        let cid = root_cid.cid();
        let ipfs_rpc = Arc::new(client.ipfs_rpc()?);
        let local_ipfs_rpc = IpfsRpc::default();
        let mount = Mount::pull(cid, &ipfs_rpc).await?;

        let (ls, schemas) = mount.ls_deep(&PathBuf::from("/")).await?;

        let pulled_items = ls
            .iter()
            .map(|(path, cid)| (path.strip_prefix("/").unwrap().to_path_buf(), cid.clone()))
            .collect::<Vec<_>>();
        let pulled_schemas = schemas
            .iter()
            .map(|(path, schema)| {
                (
                    path.strip_prefix("/").unwrap().to_path_buf(),
                    schema.clone(),
                )
            })
            .collect::<Vec<_>>();
        // Insert everything in the change log
        let mut change_log = ChangeLog::new();

        // set base items for all pulled items
        for (path, link) in pulled_items.iter() {
            // set these to None because we don't know when they were last checked yet
            change_log.insert(
                path.clone(),
                (*link.cid(), ChangeType::Base { last_check: None }),
            );
        }

        // Handle regular files and their objects together
        let current_fs_tree = utils::fs_tree()?;

        let mut pi_iter = pulled_items.iter();
        let mut ci_iter = current_fs_tree.iter();

        // Pop off "" from the fs-tree
        ci_iter.next();

        let mut to_pull = Vec::new();

        let mut pi_next = pi_iter.next();
        let mut ci_next = ci_iter.next();

        loop {
            match (pi_next, ci_next.clone()) {
                (Some((pi_path, pi_link)), Some((ci_tree, ci_path))) => {
                    // Skip schema files
                    if ci_tree.is_dir() || ci_path.to_str().is_some_and(|p| p.ends_with(".schema"))
                    {
                        ci_next = ci_iter.next();
                        continue;
                    }

                    let normalized_ci_path =
                        if ci_path.to_str().is_some_and(|p| p.ends_with(".json")) {
                            // Skip object files in comparison
                            ci_next = ci_iter.next();
                            continue;
                        } else {
                            ci_path.clone()
                        };

                    // Skip schema files in pulled items comparison
                    if pi_path.to_str().is_some_and(|p| p.ends_with(".schema")) {
                        pi_next = pi_iter.next();
                        continue;
                    }

                    if pi_path < &normalized_ci_path {
                        to_pull.push((pi_path, pi_link.cid()));
                        pi_next = pi_iter.next();
                    } else if pi_path > &normalized_ci_path {
                        ci_next = ci_iter.next();
                    } else if file_needs_pull(&local_ipfs_rpc, &normalized_ci_path, pi_link.cid())
                        .await?
                        && *pi_link.cid() != Cid::default()
                    {
                        to_pull.push((pi_path, pi_link.cid()));
                        pi_next = pi_iter.next();
                        ci_next = ci_iter.next();
                    } else {
                        pi_next = pi_iter.next();
                        ci_next = ci_iter.next();
                    }
                }
                (Some(pi), None) => {
                    // Skip schema files in pulled items
                    if pi.0.to_str().is_some_and(|p| p.ends_with(".schema")) {
                        pi_next = pi_iter.next();
                        continue;
                    }
                    to_pull.push((&pi.0, pi.1.cid()));
                    pi_next = pi_iter.next();
                }
                (None, Some(_)) => {
                    ci_next = ci_iter.next();
                }
                (None, None) => break,
            }
        }

        // Handle regular files
        for (path, link) in to_pull {
            pull_file(&mount, path).await?;
            // now that it's on disk, set the last_check to Some(SystemTime::now())
            change_log.insert(
                path.clone(),
                (
                    *link,
                    ChangeType::Base {
                        last_check: Some(SystemTime::now()),
                    },
                ),
            );
        }

        // Handle all object files
        for (path, link) in pulled_items.iter() {
            if let NodeLink::Data(_, Some(object)) = link {
                // Get base path and extension
                let stem = path.file_stem().unwrap().to_str().unwrap();
                let ext = path.extension().unwrap_or_default().to_str().unwrap();
                let parent = path.parent().unwrap();

                // Create object file path: <name>.<ext>.obj.md
                let obj_path = parent.join(format!("{}.{}.obj.md", stem, ext));

                // Convert to EditableObject for human-friendly serialization
                let editable: EditableObject = object.clone().into();

                // Serialize with surrounding ```json
                let obj_str = format!("```json\n{}\n```", serde_json::to_string_pretty(&editable)?);
                std::fs::write(&obj_path, obj_str)?;

                // Write to change log
                let cid = utils::hash_file(&obj_path, &local_ipfs_rpc, None).await?;
                change_log.insert(
                    obj_path.clone(),
                    (
                        cid,
                        ChangeType::Base {
                            last_check: Some(SystemTime::now()),
                        },
                    ),
                );
            }
        }

        // Handle schemas last, after all files are processed
        for (path, schema) in pulled_schemas {
            let schema_path = path.join("schema.md");
            let schema_str = format!("```json\n{}\n```", serde_json::to_string_pretty(&schema)?);
            std::fs::create_dir_all(&path)?;
            std::fs::write(&schema_path, schema_str)?;

            // Write schema to change log
            let cid = utils::hash_file(&schema_path, &local_ipfs_rpc, None).await?;
            change_log.insert(
                schema_path.clone(),
                (
                    cid,
                    ChangeType::Base {
                        last_check: Some(SystemTime::now()),
                    },
                ),
            );
        }

        let cid = *mount.cid();
        state.save(&mount, Some(&change_log), Some(cid))?;
        Ok(cid)
    }
}

pub async fn file_needs_pull(
    ipfs_rpc: &IpfsRpc,
    path: &PathBuf,
    cid: &Cid,
) -> Result<bool, PullError> {
    if !path.exists() {
        return Ok(true);
    } else if path.is_dir() {
        return Err(PullError::PathIsDirectory(path.clone()));
    }

    let hash = utils::hash_file(path, ipfs_rpc, None).await?;
    if hash == *cid {
        Ok(false)
    } else {
        Ok(true)
    }
}

pub async fn pull_file(mount: &Mount, path: &PathBuf) -> Result<(), PullError> {
    let data_vec = mount.cat(&PathBuf::from("/").join(path)).await?;
    let mut object_path = path.clone();
    object_path.pop();
    std::fs::create_dir_all(object_path)?;
    let mut file = std::fs::File::create(path)?;
    file.write_all(data_vec.as_slice())?;
    Ok(())
}

fn rm_file(path: &PathBuf) -> Result<(), PullError> {
    std::fs::remove_file(path)?;
    Ok(())
}
