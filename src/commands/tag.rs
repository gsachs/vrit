// Creates, lists, or deletes lightweight and annotated tags
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;
use crate::object::{Object, TagData};
use crate::repo;

pub fn execute(
    name: Option<&str>,
    commit: Option<&str>,
    annotated: bool,
    message: Option<&str>,
    delete: Option<&str>,
) -> Result<(), String> {
    let vrit_dir = repo::find_vrit_dir()?;

    if let Some(tag_name) = delete {
        return delete_tag(&vrit_dir, tag_name);
    }

    match name {
        None => list_tags(&vrit_dir),
        Some(tag_name) => {
            let target = resolve_target(&vrit_dir, commit)?;
            if annotated {
                let msg = message.ok_or("-m <message> is required for annotated tags (-a)")?;
                create_annotated(&vrit_dir, tag_name, &target, msg)
            } else {
                create_lightweight(&vrit_dir, tag_name, &target)
            }
        }
    }
}

fn list_tags(vrit_dir: &std::path::Path) -> Result<(), String> {
    let tags_dir = vrit_dir.join("refs/tags");
    if !tags_dir.exists() {
        return Ok(());
    }

    let mut tags: Vec<String> = Vec::new();
    repo::collect_refs(&tags_dir, "", &mut tags)?;
    tags.sort();

    for tag in &tags {
        println!("{tag}");
    }
    Ok(())
}

fn create_lightweight(
    vrit_dir: &std::path::Path,
    name: &str,
    target_sha: &str,
) -> Result<(), String> {
    repo::validate_ref_name(name)?;
    let ref_path = vrit_dir.join("refs/tags").join(name);
    if ref_path.exists() {
        return Err(format!("tag '{name}' already exists"));
    }

    if let Some(parent) = ref_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create tag directory: {e}"))?;
    }

    fs::write(&ref_path, format!("{target_sha}\n"))
        .map_err(|e| format!("cannot write tag ref: {e}"))?;

    Ok(())
}

fn create_annotated(
    vrit_dir: &std::path::Path,
    name: &str,
    target_sha: &str,
    message: &str,
) -> Result<(), String> {
    repo::validate_ref_name(name)?;
    let ref_path = vrit_dir.join("refs/tags").join(name);
    if ref_path.exists() {
        return Err(format!("tag '{name}' already exists"));
    }

    let config = Config::load(vrit_dir)?;
    let user_name = config.require("user.name")?;
    let email = config.require("user.email")?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("time error: {e}"))?
        .as_secs();
    let tagger_line = format!("{user_name} <{email}> {timestamp} +0000");

    let msg = if message.ends_with('\n') {
        message.to_string()
    } else {
        format!("{message}\n")
    };

    let tag_obj = Object::Tag(TagData {
        object: target_sha.to_string(),
        object_type: "commit".to_string(),
        tag_name: name.to_string(),
        tagger: tagger_line,
        message: msg,
    });
    // Annotated tags point ref → tag object → commit (unlike lightweight: ref → commit directly).
    // This indirection stores the tagger, message, and timestamp alongside the ref.
    let tag_sha = tag_obj.write_to_store(vrit_dir)?;

    if let Some(parent) = ref_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create tag directory: {e}"))?;
    }

    fs::write(&ref_path, format!("{tag_sha}\n"))
        .map_err(|e| format!("cannot write tag ref: {e}"))?;

    Ok(())
}

fn delete_tag(vrit_dir: &std::path::Path, name: &str) -> Result<(), String> {
    repo::validate_ref_name(name)?;
    let ref_path = vrit_dir.join("refs/tags").join(name);
    if !ref_path.exists() {
        return Err(format!("tag '{name}' not found"));
    }

    fs::remove_file(&ref_path)
        .map_err(|e| format!("cannot delete tag: {e}"))?;

    println!("Deleted tag '{name}'");
    Ok(())
}

fn resolve_target(
    vrit_dir: &std::path::Path,
    commit: Option<&str>,
) -> Result<String, String> {
    match commit {
        Some(sha) => {
            // Verify it exists
            Object::read_from_store(vrit_dir, sha)
                .map_err(|_| format!("commit '{sha}' not found"))?;
            Ok(sha.to_string())
        }
        None => repo::resolve_head(vrit_dir)?.ok_or("no commits yet".into()),
    }
}
