use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{Duration, Instant, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    db::Database,
    domain::{MirrorRun, MirrorTarget},
};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static NEVER_CANCELLED: AtomicBool = AtomicBool::new(false);
pub const MIRROR_CANCELLED: &str = "Mirroring cancelled.";

fn check_cancel(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Acquire) {
        Err(MIRROR_CANCELLED.to_owned())
    } else {
        Ok(())
    }
}

fn destination_identity(path: &Path) -> Result<String, String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(format!("unix:{}:{}", metadata.dev(), metadata.ino()))
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        match (metadata.volume_serial_number(), metadata.file_index()) {
            (Some(volume), Some(index)) => Ok(format!("windows:{volume}:{index}")),
            _ => Err(invalid(
                "This destination filesystem does not expose a reliable identity.",
            )),
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(format!("path:{}", path.display()))
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct MirrorEntry {
    relative_path: String,
    source_key: String,
    file_size: u64,
    content_hash: String,
    // Optional for backward compatibility with manifests written before the
    // incremental metadata cache existed. Unknown fingerprints are rehashed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_modified_ns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    destination_modified_ns: Option<u64>,
}

fn modified_ns(metadata: &fs::Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos()
        .try_into()
        .ok()
        .filter(|value| *value != 0)
}

fn file_id(metadata: &fs::Metadata) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some((metadata.dev(), metadata.ino()))
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        Some((
            metadata.volume_serial_number()? as u64,
            metadata.file_index()?,
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        None
    }
}

fn same_entry_contents(left: &[MirrorEntry], right: &[MirrorEntry]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(a, b)| {
            a.relative_path == b.relative_path
                && a.source_key == b.source_key
                && a.file_size == b.file_size
                && a.content_hash == b.content_hash
        })
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MirrorManifest {
    format_version: u8,
    refrain_version: String,
    target_id: i64,
    generated_at: i64,
    entries: Vec<MirrorEntry>,
}

// A run journal is published before changing managed files. If the process
// stops before the manifest is committed, the next run can distinguish its
// own completed replacements from an unrelated modification.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MirrorJournal {
    format_version: u8,
    target_id: i64,
    previous_manifest_hash: Option<String>,
    planned_entries: Vec<MirrorEntry>,
    // Earlier interrupted copies can still be on disk when a newer source
    // snapshot replaces the plan. Carry their hashes until manifest commit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    recovery_entries: Vec<MirrorEntry>,
}

enum MirrorSource {
    Audio(PathBuf),
    Playlist(Vec<u8>),
}

struct DesiredFile {
    entry: MirrorEntry,
    source: MirrorSource,
}

#[derive(Clone)]
struct VerifiedDestination {
    hash: String,
    size: u64,
    modified_ns: Option<u64>,
    file_id: Option<(u64, u64)>,
}

// A destination is content-checked once during planning. Execution consumes
// this plan and only checks metadata again to guard destructive operations.
enum MirrorAction {
    Keep(VerifiedDestination),
    Copy(Option<VerifiedDestination>),
    Remove(VerifiedDestination),
    SkipMissing,
}

fn planned_action(
    desired: Option<&MirrorEntry>,
    verified: Option<VerifiedDestination>,
) -> MirrorAction {
    match (desired, verified) {
        (Some(entry), Some(actual)) if entry.content_hash == actual.hash => {
            MirrorAction::Keep(actual)
        }
        (Some(_), existing) => MirrorAction::Copy(existing),
        (None, Some(existing)) => MirrorAction::Remove(existing),
        (None, None) => MirrorAction::SkipMissing,
    }
}

#[derive(Default)]
struct Progress {
    copied: i64,
    unchanged: i64,
    removed: i64,
}

pub const MIRROR_PROGRESS_EVENT: &str = "library-mirror-progress";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorProgress {
    pub target_id: i64,
    pub completed: usize,
    pub total: usize,
    pub phase: &'static str,
    pub operation: &'static str,
    pub current_file: Option<String>,
    pub copied: i64,
    pub unchanged: i64,
    pub removed: i64,
    pub file_bytes_done: u64,
    pub file_bytes_total: u64,
}

fn mirror_event(
    target_id: i64,
    stage: (&'static str, &'static str),
    file: Option<&Path>,
    completed: usize,
    total: usize,
    progress: &Progress,
    bytes: (u64, u64),
) -> MirrorProgress {
    MirrorProgress {
        target_id,
        completed,
        total,
        phase: stage.0,
        operation: stage.1,
        current_file: file.map(|path| path.to_string_lossy().replace('\\', "/")),
        copied: progress.copied,
        unchanged: progress.unchanged,
        removed: progress.removed,
        file_bytes_done: bytes.0,
        file_bytes_total: bytes.1,
    }
}

fn invalid(message: impl Into<String>) -> String {
    message.into()
}

// The manifest stays on the device when macOS assigns a new filesystem inode
// after an Android/FUSE remount. It is a more durable binding than dev/ino.
fn destination_manifest_id(destination: &Path) -> Result<Option<i64>, String> {
    let relative = Path::new(".refrain/manifest.json");
    ensure_safe_destination(destination, relative)?;
    let path = destination.join(relative);
    if !path.exists() {
        return Ok(None);
    }
    let manifest: MirrorManifest =
        serde_json::from_slice(&fs::read(path).map_err(|error| error.to_string())?)
            .map_err(|_| invalid("Existing Refrain manifest is invalid. No files were changed."))?;
    if manifest.format_version != 1 {
        return Err(invalid("Unsupported Refrain mirror manifest version."));
    }
    Ok(Some(manifest.target_id))
}

pub fn target_available(target: &MirrorTarget) -> bool {
    let path = Path::new(&target.path);
    let Ok(canonical) = fs::canonicalize(path) else {
        return false;
    };
    canonical == path
        && canonical.is_dir()
        && (destination_identity(&canonical).ok().as_deref() == Some(&target.identity)
            || destination_manifest_id(&canonical).ok().flatten() == Some(target.id))
}

// Removable volumes can be mounted under a different name after reconnecting.
// Only rebind a saved target when its own on-device manifest identifies it.
// Keep the original target row and transfer history when this happens.
#[cfg(any(target_os = "macos", test))]
fn find_remounted_volume(
    target: &MirrorTarget,
    volume_root: &Path,
) -> Result<Option<(PathBuf, String)>, String> {
    let saved = Path::new(&target.path);
    let Ok(relative) = saved.strip_prefix(volume_root) else {
        return Ok(None);
    };
    let mut parts = relative.components();
    if parts.next().is_none() {
        return Ok(None);
    }
    let subfolder = parts.as_path();
    if let Ok(volumes) = fs::read_dir(volume_root) {
        for volume in volumes.flatten() {
            let candidate = if subfolder.as_os_str().is_empty() {
                volume.path()
            } else {
                volume.path().join(subfolder)
            };
            let Ok(path) = fs::canonicalize(&candidate) else {
                continue;
            };
            if !path.is_dir() || path != candidate {
                continue;
            }
            if destination_manifest_id(&path).ok().flatten() == Some(target.id) {
                return Ok(Some((path.clone(), destination_identity(&path)?)));
            }
        }
    }
    Ok(None)
}

pub fn refresh_target(database: &Database, target: &mut MirrorTarget) -> Result<(), String> {
    if target_available(target) {
        target.available = true;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    if let Some((path, identity)) = find_remounted_volume(target, Path::new("/Volumes"))? {
        *target = database
            .restore_mirror_target(target.id, &path.to_string_lossy(), &identity)
            .map_err(|error| error.to_string())?;
        target.available = true;
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    let _ = database;
    target.available = false;
    Ok(())
}

pub fn save_target(database: &Database, path: &str) -> Result<MirrorTarget, String> {
    let raw = Path::new(path);
    if !raw.is_absolute() {
        return Err(invalid("Choose an absolute mirror destination folder."));
    }
    let target = fs::canonicalize(raw).map_err(|error| error.to_string())?;
    if !target.is_dir() {
        return Err(invalid("Mirror destination must be an existing directory."));
    }
    let settings = database.get_settings().map_err(|error| error.to_string())?;
    let library = settings
        .library_root
        .ok_or("Configure the Local library root first.")?;
    let library = fs::canonicalize(library).map_err(|error| error.to_string())?;
    if target.starts_with(&library) || library.starts_with(&target) {
        return Err(invalid(
            "Mirror destination cannot contain or be inside the Local library.",
        ));
    }
    let path = target.to_string_lossy();
    let identity = destination_identity(&target)?;
    let on_device_id = destination_manifest_id(&target)?;
    let all = database
        .all_mirror_targets()
        .map_err(|error| error.to_string())?;
    let at_path = all.iter().find(|item| item.path == path);
    let on_device = on_device_id.and_then(|id| all.iter().find(|item| item.id == id));
    if let Some(existing) = at_path {
        if existing.identity != identity && on_device_id != Some(existing.id) {
            return Err(invalid(
                "Mirror destination identity changed. Another drive is mounted at this path. Reconnect the original drive.",
            ));
        }
        if on_device_id.is_some_and(|id| id != existing.id) {
            return Err(invalid(
                "This folder contains a different mirror's manifest.",
            ));
        }
        return database
            .restore_mirror_target(existing.id, &path, &identity)
            .map_err(|error| error.to_string());
    }
    if let Some(existing) = on_device {
        return database
            .restore_mirror_target(existing.id, &path, &identity)
            .map_err(|error| error.to_string());
    }
    // An orphaned on-device manifest may belong to a target removed by an
    // older Refrain version. Its recorded hashes are checked before adoption.
    database
        .save_mirror_target(&path, &identity)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
pub fn run_mirror(database: &Database, target_id: i64) -> Result<MirrorRun, String> {
    run_mirror_with_progress(database, target_id, |_| {})
}

#[cfg(test)]
pub fn run_mirror_with_progress(
    database: &Database,
    target_id: i64,
    notify: impl Fn(MirrorProgress),
) -> Result<MirrorRun, String> {
    run_mirror_with_cancellation(database, target_id, &NEVER_CANCELLED, notify)
}

pub fn run_mirror_with_cancellation(
    database: &Database,
    target_id: i64,
    cancelled: &AtomicBool,
    notify: impl Fn(MirrorProgress),
) -> Result<MirrorRun, String> {
    check_cancel(cancelled)?;
    let mut target = database
        .list_mirror_targets()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|target| target.id == target_id)
        .ok_or("Mirror target no longer exists.")?;
    // Validate availability before recording a running operation. Never create the
    // configured mount point if a removable destination is disconnected.
    let destination = fs::canonicalize(&target.path)
        .map_err(|_| "Mirror destination is unavailable. Connect the drive before retrying.")?;
    if !destination.is_dir() || destination.to_string_lossy() != target.path {
        return Err(invalid(
            "Mirror destination identity changed. Reconnect the original drive.",
        ));
    }
    let current_identity = destination_identity(&destination)?;
    if current_identity != target.identity {
        if destination_manifest_id(&destination)? != Some(target.id) {
            return Err(invalid(
                "Mirror destination identity changed. Reconnect the original drive.",
            ));
        }
        target = database
            .restore_mirror_target(target.id, &target.path, &current_identity)
            .map_err(|error| error.to_string())?;
    }
    let root = database
        .get_settings()
        .map_err(|error| error.to_string())?
        .library_root
        .ok_or("Configure the Local library root first.")?;
    let source_root = fs::canonicalize(&root).map_err(|error| error.to_string())?;
    if source_root.starts_with(&destination) || destination.starts_with(&source_root) {
        return Err(invalid(
            "Mirror destination and Local library must not overlap.",
        ));
    }

    let run_id = database
        .start_mirror_run(target_id)
        .map_err(|error| error.to_string())?;
    let mut progress = Progress::default();
    let result = run_files(
        database,
        &source_root,
        &destination,
        &target,
        &mut progress,
        &notify,
        cancelled,
    );
    let error = result.as_ref().err().map(String::as_str);
    database
        .finish_mirror_run(
            run_id,
            progress.copied,
            progress.unchanged,
            progress.removed,
            error,
            result
                .as_ref()
                .err()
                .is_some_and(|error| error == MIRROR_CANCELLED),
        )
        .map_err(|error| error.to_string())?;
    result?;
    database
        .list_mirror_runs(target_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|run| run.id == run_id)
        .ok_or_else(|| invalid("Mirror result could not be loaded."))
}

fn run_files(
    database: &Database,
    source_root: &Path,
    destination: &Path,
    target: &MirrorTarget,
    progress: &mut Progress,
    notify: &impl Fn(MirrorProgress),
    cancelled: &AtomicBool,
) -> Result<(), String> {
    let target_id = target.id;
    let target_identity = target.identity.as_str();
    check_cancel(cancelled)?;
    notify(mirror_event(
        target_id,
        ("preparing", "Reading mirror state"),
        None,
        0,
        0,
        progress,
        (0, 0),
    ));
    let manifest_path = destination.join(".refrain/manifest.json");
    let journal_path = destination.join(".refrain/inflight.json");
    ensure_safe_destination(destination, Path::new(".refrain/manifest.json"))?;
    ensure_safe_destination(destination, Path::new(".refrain/inflight.json"))?;
    let previous_bytes = if manifest_path.exists() {
        Some(fs::read(&manifest_path).map_err(|error| error.to_string())?)
    } else {
        None
    };
    let previous_hash = previous_bytes
        .as_ref()
        .map(|bytes| blake3::hash(bytes).to_hex().to_string());
    let previous = if let Some(bytes) = &previous_bytes {
        let manifest: MirrorManifest = serde_json::from_slice(bytes)
            .map_err(|_| invalid("Existing Refrain manifest is invalid. No files were changed."))?;
        if manifest.format_version != 1
            || (manifest.target_id != target_id
                && database
                    .all_mirror_targets()
                    .map_err(|error| error.to_string())?
                    .iter()
                    .any(|target| target.id == manifest.target_id))
        {
            return Err(invalid(
                "Destination belongs to another mirror target or manifest version.",
            ));
        }
        (manifest.target_id, manifest.entries)
    } else {
        (target_id, Vec::new())
    };
    let previous_target_id = previous.0;
    let mut owned = BTreeMap::new();
    for entry in previous.1 {
        let relative = validate_relative_path(&entry.relative_path)?;
        if owned.insert(relative, entry).is_some() {
            return Err(invalid("Mirror manifest has duplicate managed paths."));
        }
    }
    reject_case_insensitive_collisions(owned.keys())?;
    let desired = desired_files_with_progress(
        database,
        source_root,
        target_id,
        progress,
        notify,
        cancelled,
        &owned,
    )?;
    let planned_entries = desired
        .values()
        .map(|file| file.entry.clone())
        .collect::<Vec<_>>();
    let journal = if journal_path.exists() {
        let bytes = fs::read(&journal_path).map_err(|error| error.to_string())?;
        let journal: MirrorJournal = serde_json::from_slice(&bytes).map_err(|_| {
            invalid("Existing mirror run journal is invalid. No files were changed.")
        })?;
        if journal.format_version != 1 || journal.target_id != previous_target_id {
            return Err(invalid("Mirror run journal belongs to another target."));
        }
        // The manifest was committed before a crash that prevented journal
        // removal only if its hash changed since the journal was created.
        // A run can also rebase its plan back to the committed entries while
        // retaining recovery hashes for copies made by an earlier attempt.
        if same_entry_contents(
            &journal.planned_entries,
            &owned.values().cloned().collect::<Vec<_>>(),
        ) && journal.previous_manifest_hash != previous_hash
        {
            check_destination_identity(destination, target_identity)?;
            fs::remove_file(&journal_path).map_err(|error| error.to_string())?;
            None
        } else {
            if journal.previous_manifest_hash != previous_hash {
                return Err(invalid(
                    "An interrupted mirror run no longer matches the committed manifest.",
                ));
            }
            Some(journal)
        }
    } else {
        None
    };
    let mut interrupted_entries: BTreeMap<PathBuf, Vec<MirrorEntry>> = BTreeMap::new();
    if let Some(journal) = &journal {
        for entry in &journal.planned_entries {
            let relative = validate_relative_path(&entry.relative_path)?;
            if interrupted_entries.contains_key(&relative) {
                return Err(invalid(
                    "Interrupted mirror journal has duplicate managed paths.",
                ));
            }
            interrupted_entries.insert(relative, vec![entry.clone()]);
        }
        for entry in &journal.recovery_entries {
            let relative = validate_relative_path(&entry.relative_path)?;
            let existing = interrupted_entries.entry(relative).or_default();
            if !existing
                .iter()
                .any(|old| old.content_hash == entry.content_hash)
            {
                existing.push(entry.clone());
            }
        }
    }
    reject_case_insensitive_collisions(interrupted_entries.keys())?;

    // Plan every output before touching anything. Each destination is compared
    // only here, so unrelated destination data is never overwritten and the
    // execute phase does not need to rehash already inspected Android files.
    let stale = owned
        .keys()
        .chain(interrupted_entries.keys())
        .filter(|path| !desired.contains_key(*path))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let preflight_total = desired.len() + stale.len();
    let mut preflight_completed = 0;
    let mut actions = BTreeMap::new();
    for relative in desired.keys() {
        check_cancel(cancelled)?;
        notify(mirror_event(
            target_id,
            ("verifying", "Checking destination"),
            Some(relative),
            preflight_completed,
            preflight_total,
            progress,
            (0, 0),
        ));
        let verified = check_owned_file_with_cancel(
            destination,
            relative,
            owned.get(relative),
            interrupted_entries.get(relative).map(Vec::as_slice),
            cancelled,
            |done, total| {
                notify(mirror_event(
                    target_id,
                    ("verifying", "Checking destination"),
                    Some(relative),
                    preflight_completed,
                    preflight_total,
                    progress,
                    (done, total),
                ));
            },
        )?;
        let action = planned_action(Some(&desired[relative].entry), verified);
        actions.insert(relative.clone(), action);
        preflight_completed += 1;
    }
    // Include outputs introduced by an interrupted run, even when the source
    // changed and those outputs are absent from the committed manifest.
    for relative in &stale {
        check_cancel(cancelled)?;
        notify(mirror_event(
            target_id,
            ("verifying", "Checking obsolete file"),
            Some(relative),
            preflight_completed,
            preflight_total,
            progress,
            (0, 0),
        ));
        let verified = check_owned_file_with_cancel(
            destination,
            relative,
            owned.get(relative),
            interrupted_entries.get(relative).map(Vec::as_slice),
            cancelled,
            |done, total| {
                notify(mirror_event(
                    target_id,
                    ("verifying", "Checking obsolete file"),
                    Some(relative),
                    preflight_completed,
                    preflight_total,
                    progress,
                    (done, total),
                ));
            },
        )?;
        actions.insert(relative.clone(), planned_action(None, verified));
        preflight_completed += 1;
    }

    let total = desired.len() + stale.len();
    let mut completed = 0;
    notify(mirror_event(
        target_id,
        ("copying", "Starting file updates"),
        None,
        completed,
        total,
        progress,
        (0, 0),
    ));

    // Remove obsolete interrupted outputs while the old journal still proves
    // ownership. If interrupted again, the same journal can safely retry.
    for relative in &stale {
        check_cancel(cancelled)?;
        notify(mirror_event(
            target_id,
            ("working", "Removing obsolete file"),
            Some(relative),
            completed,
            total,
            progress,
            (0, 0),
        ));
        check_destination_identity(destination, target_identity)?;
        let path = destination.join(relative);
        match &actions[relative] {
            MirrorAction::Remove(verified) => {
                ensure_planned_destination(destination, relative, Some(verified))?;
                fs::remove_file(&path).map_err(|error| error.to_string())?;
                progress.removed += 1;
            }
            MirrorAction::SkipMissing => {
                ensure_planned_destination(destination, relative, None)?;
            }
            _ => return Err(invalid("Invalid obsolete file action in mirror plan.")),
        }
        completed += 1;
        notify(mirror_event(
            target_id,
            ("copying", "Removed obsolete file"),
            Some(relative),
            completed,
            total,
            progress,
            (0, 0),
        ));
    }

    if journal
        .as_ref()
        .is_none_or(|existing| existing.planned_entries != planned_entries)
    {
        check_cancel(cancelled)?;
        notify(mirror_event(
            target_id,
            ("working", "Saving recovery journal"),
            None,
            completed,
            total,
            progress,
            (0, 0),
        ));
        let mut recovery_entries = journal
            .as_ref()
            .into_iter()
            .flat_map(|old| {
                old.planned_entries
                    .iter()
                    .chain(old.recovery_entries.iter())
            })
            .filter(|entry| desired.contains_key(&PathBuf::from(&entry.relative_path)))
            .cloned()
            .collect::<Vec<_>>();
        recovery_entries.sort_by(|a, b| {
            (&a.relative_path, &a.content_hash).cmp(&(&b.relative_path, &b.content_hash))
        });
        recovery_entries.dedup_by(|a, b| {
            a.relative_path == b.relative_path && a.content_hash == b.content_hash
        });
        let journal = MirrorJournal {
            format_version: 1,
            target_id,
            previous_manifest_hash: previous_hash,
            planned_entries: planned_entries.clone(),
            recovery_entries,
        };
        let bytes = serde_json::to_vec_pretty(&journal).map_err(|error| error.to_string())?;
        check_destination_identity(destination, target_identity)?;
        replace_verified_with_cancel(
            &journal_path,
            blake3::hash(&bytes).to_hex().as_ref(),
            |output| output.write_all(&bytes),
            cancelled,
        )?;
    }

    let mut committed_entries = Vec::new();
    // Populate audio before publishing playlists that reference those files.
    // Manifest contents are sorted again before publication below.
    for (relative, file) in desired
        .iter()
        .filter(|(_, file)| matches!(file.source, MirrorSource::Audio(_)))
        .chain(
            desired
                .iter()
                .filter(|(_, file)| matches!(file.source, MirrorSource::Playlist(_))),
        )
    {
        check_cancel(cancelled)?;
        notify(mirror_event(
            target_id,
            ("working", "Applying planned action"),
            Some(relative),
            completed,
            total,
            progress,
            (0, 0),
        ));
        let path = destination.join(relative);
        let action = &actions[relative];
        if let MirrorAction::Keep(verified) = action {
            ensure_planned_destination(destination, relative, Some(verified))?;
            let mut entry = file.entry.clone();
            entry.destination_modified_ns = verified.modified_ns;
            committed_entries.push(entry);
            progress.unchanged += 1;
            completed += 1;
            notify(mirror_event(
                target_id,
                ("copying", "Unchanged"),
                Some(relative),
                completed,
                total,
                progress,
                (0, 0),
            ));
            continue;
        }
        let MirrorAction::Copy(existing) = action else {
            return Err(invalid("Invalid file copy action in mirror plan."));
        };
        check_destination_identity(destination, target_identity)?;
        // Validate the path before creating a same-directory temporary file.
        // The content fingerprint is checked once more immediately before the
        // final rename, after the temporary copy has been verified.
        ensure_safe_destination(destination, relative)?;
        let playlist_export = matches!(file.source, MirrorSource::Playlist(_));
        notify(mirror_event(
            target_id,
            (
                "working",
                if playlist_export {
                    "Exporting playlist"
                } else {
                    "Copying file"
                },
            ),
            Some(relative),
            completed,
            total,
            progress,
            (0, file.entry.file_size),
        ));
        match &file.source {
            MirrorSource::Audio(source) => {
                replace_verified_with_progress_guarded(
                    &path,
                    file.entry.content_hash.as_str(),
                    |output| {
                        let mut input = File::open(source)?;
                        let mut buffer = [0_u8; 256 * 1024];
                        let mut bytes_done = 0;
                        let mut last_update = Instant::now();
                        loop {
                            if cancelled.load(Ordering::Acquire) {
                                return Err(io::Error::other(MIRROR_CANCELLED));
                            }
                            let read = input.read(&mut buffer)?;
                            if read == 0 {
                                break;
                            }
                            output.write_all(&buffer[..read])?;
                            bytes_done += read as u64;
                            if last_update.elapsed() >= Duration::from_millis(200) {
                                notify(mirror_event(
                                    target_id,
                                    ("working", "Copying file"),
                                    Some(relative),
                                    completed,
                                    total,
                                    progress,
                                    (bytes_done, file.entry.file_size),
                                ));
                                last_update = Instant::now();
                            }
                        }
                        notify(mirror_event(
                            target_id,
                            ("working", "Copying file"),
                            Some(relative),
                            completed,
                            total,
                            progress,
                            (bytes_done, file.entry.file_size),
                        ));
                        Ok(())
                    },
                    cancelled,
                    |done, file_total| {
                        notify(mirror_event(
                            target_id,
                            ("working", "Verifying copied file"),
                            Some(relative),
                            completed,
                            total,
                            progress,
                            (done, file_total),
                        ));
                    },
                    || {
                        check_destination_identity(destination, target_identity)?;
                        ensure_planned_destination(destination, relative, existing.as_ref())
                    },
                )?;
            }
            MirrorSource::Playlist(bytes) => {
                replace_verified_with_progress_guarded(
                    &path,
                    file.entry.content_hash.as_str(),
                    |output| output.write_all(bytes),
                    cancelled,
                    |_, _| {},
                    || {
                        check_destination_identity(destination, target_identity)?;
                        ensure_planned_destination(destination, relative, existing.as_ref())
                    },
                )?;
                notify(mirror_event(
                    target_id,
                    ("working", "Exporting playlist"),
                    Some(relative),
                    completed,
                    total,
                    progress,
                    (bytes.len() as u64, bytes.len() as u64),
                ));
            }
        }
        let mut entry = file.entry.clone();
        entry.destination_modified_ns =
            modified_ns(&fs::metadata(&path).map_err(|error| error.to_string())?);
        committed_entries.push(entry);
        progress.copied += 1;
        completed += 1;
        notify(mirror_event(
            target_id,
            (
                "copying",
                if playlist_export {
                    "Exported playlist"
                } else {
                    "Copied file"
                },
            ),
            Some(relative),
            completed,
            total,
            progress,
            (0, 0),
        ));
    }

    // The manifest is the portable record of ownership. Publish it only after
    // all copies, replacements, and safe removals have completed.
    check_cancel(cancelled)?;
    notify(mirror_event(
        target_id,
        ("finalizing", "Saving mirror manifest"),
        None,
        completed,
        total,
        progress,
        (0, 0),
    ));
    committed_entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let manifest = MirrorManifest {
        format_version: 1,
        refrain_version: env!("CARGO_PKG_VERSION").into(),
        target_id,
        generated_at: crate::db::now_ms(),
        entries: committed_entries,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    check_destination_identity(destination, target_identity)?;
    replace_verified_with_cancel(
        &manifest_path,
        blake3::hash(&manifest_bytes).to_hex().as_ref(),
        |output| output.write_all(&manifest_bytes),
        cancelled,
    )?;
    let entries = manifest
        .entries
        .into_iter()
        .map(|entry| {
            (
                entry.relative_path,
                entry.source_key,
                entry.file_size as i64,
                entry.content_hash,
            )
        })
        .collect::<Vec<_>>();
    database
        .replace_mirror_entries(target_id, &entries)
        .map_err(|error| error.to_string())?;
    check_destination_identity(destination, target_identity)?;
    fs::remove_file(&journal_path).map_err(|error| error.to_string())?;
    notify(mirror_event(
        target_id,
        ("complete", "Mirror saved"),
        None,
        total,
        total,
        progress,
        (0, 0),
    ));
    Ok(())
}

#[cfg(test)]
fn desired_files(
    database: &Database,
    library: &Path,
) -> Result<BTreeMap<PathBuf, DesiredFile>, String> {
    desired_files_with_progress(
        database,
        library,
        0,
        &Progress::default(),
        &|_| {},
        &NEVER_CANCELLED,
        &BTreeMap::new(),
    )
}

fn desired_files_with_progress(
    database: &Database,
    library: &Path,
    target_id: i64,
    progress: &Progress,
    notify: &impl Fn(MirrorProgress),
    cancelled: &AtomicBool,
    previous: &BTreeMap<PathBuf, MirrorEntry>,
) -> Result<BTreeMap<PathBuf, DesiredFile>, String> {
    let mut desired = BTreeMap::new();
    let mut source_paths = HashMap::new();
    // A playlist may prefer an external copy even when the same logical track
    // has a present, indexed copy inside the library being mirrored.
    let mut mirrored_tracks: HashMap<i64, (bool, PathBuf)> = HashMap::new();
    let audio_files = database
        .local_files_snapshot()
        .map_err(|error| error.to_string())?;
    let audio_total = audio_files
        .iter()
        .filter(|audio| audio.state == "present" && Path::new(&audio.path).starts_with(library))
        .count();
    let mut scanned = 0;
    for audio in audio_files {
        check_cancel(cancelled)?;
        if audio.state != "present" {
            continue;
        }
        // External indexed copies are not mirror inputs. They may be offline,
        // even when another copy of the same track exists under this root.
        if !Path::new(&audio.path).starts_with(library) {
            continue;
        }
        scanned += 1;
        let source = fs::canonicalize(&audio.path).map_err(|error| error.to_string())?;
        let Ok(relative) = source.strip_prefix(library) else {
            continue;
        };
        if !source.is_file() {
            return Err(invalid(format!(
                "Local audio is unavailable: {}",
                source.display()
            )));
        }
        let dest = Path::new("Music").join(relative);
        let metadata = fs::metadata(&source).map_err(|error| error.to_string())?;
        notify(mirror_event(
            target_id,
            ("scanning", "Hashing source file"),
            Some(&dest),
            scanned - 1,
            audio_total,
            progress,
            (0, metadata.len()),
        ));
        let timestamp = modified_ns(&metadata);
        let cached = previous.get(&dest).filter(|entry| {
            entry.source_key == format!("local-file:{}", audio.id)
                && entry.file_size == metadata.len()
                && timestamp.is_some()
                && entry.source_modified_ns == timestamp
        });
        let hash = if let Some(entry) = cached {
            notify(mirror_event(
                target_id,
                ("scanning", "Source unchanged"),
                Some(&dest),
                scanned - 1,
                audio_total,
                progress,
                (metadata.len(), metadata.len()),
            ));
            entry.content_hash.clone()
        } else {
            let hash = hash_file_with_progress(&source, cancelled, |done, total| {
                notify(mirror_event(
                    target_id,
                    ("scanning", "Hashing source file"),
                    Some(&dest),
                    scanned - 1,
                    audio_total,
                    progress,
                    (done, total),
                ));
            })?;
            if fs::metadata(&source)
                .map_err(|error| error.to_string())?
                .len()
                != metadata.len()
                || modified_ns(&fs::metadata(&source).map_err(|error| error.to_string())?)
                    != timestamp
            {
                return Err(invalid(format!(
                    "Source changed while scanning: {}",
                    source.display()
                )));
            }
            hash
        };
        let entry = MirrorEntry {
            relative_path: relative_string(&dest)?,
            source_key: format!("local-file:{}", audio.id),
            file_size: metadata.len(),
            content_hash: hash,
            source_modified_ns: timestamp,
            destination_modified_ns: None,
        };
        if desired
            .insert(
                dest.clone(),
                DesiredFile {
                    entry,
                    source: MirrorSource::Audio(source.clone()),
                },
            )
            .is_some()
        {
            return Err(invalid(
                "Two local files resolve to the same mirror location.",
            ));
        }
        if let Some(track_id) = audio.library_track_id {
            mirrored_tracks
                .entry(track_id)
                .and_modify(|(preferred, path)| {
                    if audio.is_preferred && !*preferred {
                        *preferred = true;
                        *path = dest.clone();
                    }
                })
                .or_insert_with(|| (audio.is_preferred, dest.clone()));
        }
        source_paths.insert(source, dest);
    }

    let playlists = database
        .list_local_playlists()
        .map_err(|error| error.to_string())?;
    let playlist_total = playlists.len();
    for (index, playlist) in playlists.into_iter().enumerate() {
        check_cancel(cancelled)?;
        notify(mirror_event(
            target_id,
            ("playlists", "Preparing playlist"),
            Some(Path::new(&playlist.name)),
            index,
            playlist_total,
            progress,
            (0, 0),
        ));
        let detail = database
            .get_local_playlist(playlist.id)
            .map_err(|error| error.to_string())?;
        let mut entries_output = String::new();
        let mut unavailable = 0;
        for entry in detail.entries {
            check_cancel(cancelled)?;
            let audio_path = entry
                .file_path
                .as_deref()
                .and_then(|path| fs::canonicalize(path).ok())
                .and_then(|path| source_paths.get(&path))
                .or_else(|| {
                    mirrored_tracks
                        .get(&entry.library_track_id)
                        .map(|(_, path)| path)
                });
            let title = entry.title.replace(['\r', '\n'], " ");
            let artists = entry.artists.join(", ").replace(['\r', '\n'], " ");
            let Some(audio_path) = audio_path else {
                // M3U comments preserve the missing position for inspection
                // without making a media player open a broken absolute path.
                unavailable += 1;
                entries_output.push_str(&format!("#REFRAIN-UNAVAILABLE:{} - {}\n", artists, title));
                continue;
            };
            entries_output.push_str(&format!(
                "#EXTINF:{},{} - {}\n../{}\n",
                entry.duration_ms.unwrap_or(0) / 1000,
                artists,
                title,
                audio_path
                    .strip_prefix("Music")
                    .map_err(|_| invalid("Mirror playlist track is outside Music."))?
                    .to_string_lossy()
                    .replace('\\', "/"),
            ));
        }
        // Plain M3U is recognized by more Android media scanners than M3U8.
        // Ownership is tracked in the mirror manifest, so no custom header is needed.
        let mut output = String::from("#EXTM3U\n");
        if unavailable > 0 {
            output.push_str(&format!("#REFRAIN-MIRROR-SKIPPED:{unavailable}\n"));
            notify(mirror_event(
                target_id,
                ("playlists", "Playlist contains unavailable tracks"),
                Some(Path::new(&playlist.name)),
                index + 1,
                playlist_total,
                progress,
                (0, 0),
            ));
        }
        output.push_str(&entries_output);
        let cleaned = playlist
            .name
            .chars()
            .map(|character| {
                if character.is_control() || "/\\:*?\"<>|".contains(character) {
                    '_'
                } else {
                    character
                }
            })
            .collect::<String>();
        let cleaned = cleaned.trim().trim_matches('.');
        let basename = if cleaned.is_empty() {
            "Playlist"
        } else {
            cleaned
        };
        let relative =
            Path::new("Music/00 Playlists").join(format!("{basename}-{}.m3u", playlist.id));
        let bytes = output.into_bytes();
        let entry = MirrorEntry {
            relative_path: relative_string(&relative)?,
            source_key: format!("local-playlist:{}", playlist.id),
            file_size: bytes.len() as u64,
            content_hash: blake3::hash(&bytes).to_hex().to_string(),
            source_modified_ns: None,
            destination_modified_ns: None,
        };
        desired.insert(
            relative,
            DesiredFile {
                entry,
                source: MirrorSource::Playlist(bytes),
            },
        );
    }
    reject_case_insensitive_collisions(desired.keys())?;
    Ok(desired)
}

fn reject_case_insensitive_collisions<'a>(
    paths: impl IntoIterator<Item = &'a PathBuf>,
) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for path in paths {
        if !seen.insert(path.to_string_lossy().to_lowercase()) {
            return Err(invalid(
                "Mirror destination contains filenames that conflict on case-insensitive filesystems.",
            ));
        }
    }
    Ok(())
}

fn relative_string(path: &Path) -> Result<String, String> {
    validate_relative_path(&path.to_string_lossy())?;
    Ok(path.to_string_lossy().replace('\\', "/"))
}

fn validate_relative_path(path: &str) -> Result<PathBuf, String> {
    let relative = PathBuf::from(path);
    if path.is_empty()
        || !matches!(relative.components().next(), Some(Component::Normal(_)))
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || !(path.starts_with("Music/") || path.starts_with("Playlists/"))
    {
        return Err(invalid("Mirror manifest contains an unsafe managed path."));
    }
    Ok(relative)
}

fn ensure_safe_destination(root: &Path, relative: &Path) -> Result<(), String> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(invalid("Mirror destination path contains traversal."));
        }
        current.push(component);
        if let Ok(metadata) = fs::symlink_metadata(&current)
            && metadata.file_type().is_symlink()
        {
            return Err(invalid(format!(
                "Mirror destination contains a symbolic link: {}",
                current.display()
            )));
        }
    }
    Ok(())
}

fn check_destination_identity(root: &Path, expected_identity: &str) -> Result<(), String> {
    let current = fs::canonicalize(root).map_err(|_| {
        invalid("Mirror destination is unavailable. Connect the original drive before retrying.")
    })?;
    if current != root || destination_identity(&current)? != expected_identity {
        return Err(invalid(
            "Mirror destination identity changed during the run. Reconnect the original drive before retrying.",
        ));
    }
    Ok(())
}

// A metadata-only guard against edits, replacements, or new collisions since
// planning. In particular, this never performs a second Android content hash.
fn ensure_planned_destination(
    root: &Path,
    relative: &Path,
    planned: Option<&VerifiedDestination>,
) -> Result<(), String> {
    ensure_safe_destination(root, relative)?;
    let path = root.join(relative);
    let current = match fs::metadata(&path) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.to_string()),
    };
    let same = match (planned, current.as_ref()) {
        (None, None) => true,
        (Some(expected), Some(metadata)) => {
            metadata.is_file()
                && metadata.len() == expected.size
                && modified_ns(metadata) == expected.modified_ns
                && file_id(metadata) == expected.file_id
        }
        _ => false,
    };
    if !same {
        return Err(invalid(format!(
            "Mirror destination has an unrelated or externally changed file: {}",
            path.display()
        )));
    }
    Ok(())
}

fn check_owned_file_with_cancel(
    root: &Path,
    relative: &Path,
    previous: Option<&MirrorEntry>,
    interrupted: Option<&[MirrorEntry]>,
    cancelled: &AtomicBool,
    on_progress: impl FnMut(u64, u64),
) -> Result<Option<VerifiedDestination>, String> {
    check_cancel(cancelled)?;
    ensure_safe_destination(root, relative)?;
    let path = root.join(relative);
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.is_file() {
        return Err(invalid(format!(
            "Mirror destination has an unrelated or externally changed file: {}",
            path.display()
        )));
    }
    let timestamp = modified_ns(&metadata);
    let cached = previous
        .into_iter()
        .chain(interrupted.into_iter().flatten())
        .find(|entry| {
            entry.file_size == metadata.len()
                && timestamp.is_some()
                && entry.destination_modified_ns == timestamp
        });
    let actual_hash = if let Some(entry) = cached {
        entry.content_hash.clone()
    } else {
        hash_file_with_progress(&path, cancelled, on_progress)?
    };
    if !(previous.is_some_and(|entry| entry.content_hash == actual_hash)
        || interrupted.is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| entry.content_hash == actual_hash)
        }))
    {
        return Err(invalid(format!(
            "Mirror destination has an unrelated or externally changed file: {}",
            path.display()
        )));
    }
    Ok(Some(VerifiedDestination {
        hash: actual_hash,
        size: metadata.len(),
        modified_ns: timestamp,
        file_id: file_id(&metadata),
    }))
}

fn hash_file_with_progress(
    path: &Path,
    cancelled: &AtomicBool,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<String, String> {
    let mut input = File::open(path).map_err(|error| error.to_string())?;
    let total = input.metadata().map_err(|error| error.to_string())?.len();
    let mut done = 0;
    let mut last_update = Instant::now();
    on_progress(0, total);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_cancel(cancelled)?;
        let read = input.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        done += read as u64;
        if last_update.elapsed() >= Duration::from_millis(200) {
            on_progress(done, total);
            last_update = Instant::now();
        }
    }
    if done != total {
        return Err(invalid(format!(
            "File size changed during hashing: {}",
            path.display()
        )));
    }
    on_progress(done, total);
    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
fn replace_verified(
    path: &Path,
    hash: &str,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> Result<(), String> {
    replace_verified_with_cancel(path, hash, write, &NEVER_CANCELLED)
}

fn replace_verified_with_cancel(
    path: &Path,
    hash: &str,
    write: impl FnOnce(&mut File) -> io::Result<()>,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    replace_verified_with_progress(path, hash, write, cancelled, |_, _| {})
}

fn replace_verified_with_progress(
    path: &Path,
    hash: &str,
    write: impl FnOnce(&mut File) -> io::Result<()>,
    cancelled: &AtomicBool,
    on_verification_progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    replace_verified_with_progress_guarded(
        path,
        hash,
        write,
        cancelled,
        on_verification_progress,
        || Ok(()),
    )
}

fn replace_verified_with_progress_guarded(
    path: &Path,
    hash: &str,
    write: impl FnOnce(&mut File) -> io::Result<()>,
    cancelled: &AtomicBool,
    on_verification_progress: impl FnMut(u64, u64),
    before_replace: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    check_cancel(cancelled)?;
    let parent = path
        .parent()
        .ok_or("Mirror file has no parent directory.")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let unique = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp = parent.join(format!(".refrain-{}-{unique}.tmp", std::process::id()));
    let result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| error.to_string())?;
        write(&mut file).map_err(|error| error.to_string())?;
        check_cancel(cancelled)?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        if hash_file_with_progress(&temp, cancelled, on_verification_progress)? != hash {
            return Err(invalid(
                "Mirror copy changed while being written. Original output preserved.",
            ));
        }
        check_cancel(cancelled)?;
        before_replace()?;
        replace_file(&temp, path).map_err(|error| error.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(not(windows))]
fn replace_file(temp: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(temp, destination)
}

// Windows rename does not consistently replace an existing destination.
// MoveFileExW with REPLACE_EXISTING keeps the old file until the new verified
// temporary file can be renamed over it on the same filesystem.
#[cfg(windows)]
fn replace_file(temp: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new_name: *const u16, flags: u32) -> i32;
    }

    let from = temp
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let to = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    const MOVEFILE_REPLACE_EXISTING: u32 = 1;
    const MOVEFILE_WRITE_THROUGH: u32 = 8;
    // SAFETY: Both pointers refer to null-terminated UTF-16 buffers that
    // remain alive for the duration of the synchronous Windows API call.
    if unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_library::scan_library;

    #[test]
    fn rejects_case_insensitive_destination_collisions() {
        let paths = [
            PathBuf::from("Music/Track.flac"),
            PathBuf::from("Music/track.flac"),
        ];
        assert!(reject_case_insensitive_collisions(paths.iter()).is_err());
    }

    #[test]
    fn plans_keep_copy_remove_and_missing_actions() {
        let entry = MirrorEntry {
            relative_path: "Music/song.wav".into(),
            source_key: "local-file:1".into(),
            file_size: 4,
            content_hash: "new".into(),
            source_modified_ns: None,
            destination_modified_ns: None,
        };
        let verified = || VerifiedDestination {
            hash: "new".into(),
            size: 4,
            modified_ns: Some(12),
            file_id: Some((1, 2)),
        };
        assert!(matches!(
            planned_action(Some(&entry), Some(verified())),
            MirrorAction::Keep(_)
        ));
        assert!(matches!(
            planned_action(Some(&entry), None),
            MirrorAction::Copy(None)
        ));
        let mut changed = verified();
        changed.hash = "old".into();
        assert!(matches!(
            planned_action(Some(&entry), Some(changed)),
            MirrorAction::Copy(Some(_))
        ));
        assert!(matches!(
            planned_action(None, Some(verified())),
            MirrorAction::Remove(_)
        ));
        assert!(matches!(
            planned_action(None, None),
            MirrorAction::SkipMissing
        ));
    }

    struct Fixture {
        folder: PathBuf,
        source: PathBuf,
        target: PathBuf,
        database: Database,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            let number = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let folder = std::env::temp_dir().join(format!(
                "refrain-mirror-{label}-{}-{number}",
                std::process::id()
            ));
            let source = folder.join("library");
            let target = folder.join("target");
            fs::create_dir_all(&source).unwrap();
            fs::create_dir_all(&target).unwrap();
            let database = Database::open(folder.join("test.sqlite3")).unwrap();
            let mut settings = database.get_settings().unwrap();
            settings.library_root = Some(source.to_string_lossy().into_owned());
            database.update_settings(&settings).unwrap();
            Self {
                folder,
                source,
                target,
                database,
            }
        }

        fn wav(&self, bytes: usize) {
            let sample_rate = 8_000u32;
            let mut wav = Vec::new();
            wav.extend_from_slice(b"RIFF");
            wav.extend_from_slice(&(36 + bytes as u32).to_le_bytes());
            wav.extend_from_slice(b"WAVEfmt ");
            wav.extend_from_slice(&16u32.to_le_bytes());
            wav.extend_from_slice(&1u16.to_le_bytes());
            wav.extend_from_slice(&1u16.to_le_bytes());
            wav.extend_from_slice(&sample_rate.to_le_bytes());
            wav.extend_from_slice(&sample_rate.to_le_bytes());
            wav.extend_from_slice(&1u16.to_le_bytes());
            wav.extend_from_slice(&8u16.to_le_bytes());
            wav.extend_from_slice(b"data");
            wav.extend_from_slice(&(bytes as u32).to_le_bytes());
            wav.extend(std::iter::repeat_n(128, bytes));
            fs::write(self.source.join("song.wav"), wav).unwrap();
        }

        fn scan(&self) {
            scan_library(&self.database, &self.source, |_| {}).unwrap();
        }

        fn target_id(&self) -> i64 {
            save_target(&self.database, self.target.to_str().unwrap())
                .unwrap()
                .id
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.folder);
        }
    }

    #[test]
    fn rejects_manifest_traversal() {
        for path in [
            "../Music/song.flac",
            "Music/../private.txt",
            "/Music/song.flac",
            "Other/song.flac",
        ] {
            assert!(validate_relative_path(path).is_err(), "{path}");
        }
        assert!(validate_relative_path("Music/Artist/recording.flac").is_ok());
    }

    #[test]
    fn hashing_rejects_a_source_that_grows_during_the_read() {
        let fixture = Fixture::new("changing-hash-size");
        let path = fixture.source.join("changing.bin");
        fs::write(&path, b"original").unwrap();
        let result = hash_file_with_progress(&path, &NEVER_CANCELLED, |done, _| {
            if done == 0 {
                let mut file = OpenOptions::new().append(true).open(&path).unwrap();
                file.write_all(b"additional bytes").unwrap();
            }
        });
        assert!(
            result
                .unwrap_err()
                .contains("File size changed during hashing")
        );
    }

    #[test]
    fn reports_file_names_stages_bytes_and_final_counts() {
        let fixture = Fixture::new("observable-progress");
        fixture.wav(800);
        fixture.scan();
        let target_id = fixture.target_id();
        let events = std::cell::RefCell::new(Vec::new());
        run_mirror_with_progress(&fixture.database, target_id, |progress| {
            events.borrow_mut().push(progress);
        })
        .unwrap();
        let events = events.into_inner();
        assert!(events.iter().any(|event| {
            event.phase == "scanning"
                && event.operation == "Hashing source file"
                && event.current_file.as_deref() == Some("Music/song.wav")
                && event.file_bytes_total > 0
                && event.file_bytes_done == event.file_bytes_total
        }));
        assert!(events.iter().any(|event| {
            event.phase == "verifying" && event.current_file.as_deref() == Some("Music/song.wav")
        }));
        assert!(events.iter().any(|event| {
            event.operation == "Verifying copied file"
                && event.file_bytes_done == 0
                && event.file_bytes_total > 0
        }));
        assert!(events.iter().any(|event| {
            event.operation == "Verifying copied file"
                && event.file_bytes_done == event.file_bytes_total
                && event.file_bytes_total > 0
        }));
        assert!(events.iter().any(|event| {
            event.phase == "copying" && event.operation == "Copied file" && event.copied > 0
        }));
        assert_eq!(events.last().unwrap().phase, "complete");

        let repeated = std::cell::RefCell::new(Vec::new());
        run_mirror_with_progress(&fixture.database, target_id, |progress| {
            repeated.borrow_mut().push(progress);
        })
        .unwrap();
        assert!(
            repeated
                .borrow()
                .iter()
                .any(|event| { event.operation == "Unchanged" && event.unchanged > 0 })
        );
        assert!(
            repeated.borrow().iter().any(|event| {
                event.operation == "Source unchanged" && event.file_bytes_done > 0
            })
        );
        assert!(!repeated.borrow().iter().any(|event| {
            matches!(
                event.operation,
                "Hashing source file" | "Checking destination" | "Comparing file"
            ) && event.file_bytes_done > 0
        }));
    }

    #[test]
    fn cancelling_during_source_hash_records_cancelled_without_writing_files() {
        let fixture = Fixture::new("cancel-source-hash");
        fixture.wav(800);
        fixture.scan();
        let id = fixture.target_id();
        let cancelled = AtomicBool::new(false);
        let result = run_mirror_with_cancellation(&fixture.database, id, &cancelled, |event| {
            if event.operation == "Hashing source file" {
                cancelled.store(true, Ordering::Release);
            }
        });

        assert_eq!(result.unwrap_err(), MIRROR_CANCELLED);
        assert_eq!(
            fixture.database.list_mirror_runs(id).unwrap()[0].status,
            "cancelled"
        );
        assert!(!fixture.target.join(".refrain/manifest.json").exists());
        assert!(!fixture.target.join("Music/song.wav").exists());
        assert_eq!(
            run_mirror(&fixture.database, id).unwrap().status,
            "succeeded"
        );
    }

    #[test]
    fn cancelling_after_copy_preserves_previous_output_and_recovers_journal() {
        let fixture = Fixture::new("cancel-after-copy");
        fixture.wav(800);
        fixture.scan();
        let id = fixture.target_id();
        run_mirror(&fixture.database, id).unwrap();
        let original_audio = fs::read(fixture.target.join("Music/song.wav")).unwrap();
        let original_manifest = fs::read(fixture.target.join(".refrain/manifest.json")).unwrap();

        fixture.wav(4000);
        fixture.scan();
        let cancelled = AtomicBool::new(false);
        let result = run_mirror_with_cancellation(&fixture.database, id, &cancelled, |event| {
            if event.operation == "Verifying copied file" {
                cancelled.store(true, Ordering::Release);
            }
        });

        assert_eq!(result.unwrap_err(), MIRROR_CANCELLED);
        assert_eq!(
            fs::read(fixture.target.join("Music/song.wav")).unwrap(),
            original_audio
        );
        assert_eq!(
            fs::read(fixture.target.join(".refrain/manifest.json")).unwrap(),
            original_manifest
        );
        assert!(fixture.target.join(".refrain/inflight.json").exists());
        assert!(
            fs::read_dir(fixture.target.join("Music"))
                .unwrap()
                .all(|entry| entry.unwrap().file_name() == "song.wav")
        );

        let reopened = Database::open(fixture.folder.join("test.sqlite3")).unwrap();
        assert_eq!(
            reopened.list_mirror_runs(id).unwrap()[0].status,
            "cancelled"
        );
        let resumed = run_mirror(&reopened, id).unwrap();
        assert_eq!(resumed.status, "succeeded");
        assert_eq!(resumed.copied, 1);
        assert_eq!(
            fs::read(fixture.target.join("Music/song.wav")).unwrap(),
            fs::read(fixture.source.join("song.wav")).unwrap()
        );
        assert!(!fixture.target.join(".refrain/inflight.json").exists());
    }

    #[test]
    fn incremental_mirror_preserves_unrelated_files_and_relative_playlist_paths() {
        let fixture = Fixture::new("incremental");
        fixture.wav(800);
        fs::write(
            fixture.source.join("Mix.m3u8"),
            "#EXTM3U\nsong.wav\nsong.wav\n",
        )
        .unwrap();
        fixture.scan();
        fs::write(fixture.target.join("Keep.txt"), "unrelated").unwrap();
        let id = fixture.target_id();

        let events = std::cell::RefCell::new(Vec::new());
        let first = run_mirror_with_progress(&fixture.database, id, |event| {
            events.borrow_mut().push(event.operation);
        })
        .unwrap();
        assert_eq!(first.status, "succeeded");
        assert_eq!(first.copied, 2);
        let events = events.into_inner();
        let audio_at = events
            .iter()
            .position(|event| *event == "Copied file")
            .unwrap();
        let playlist_at = events
            .iter()
            .position(|event| *event == "Exported playlist")
            .unwrap();
        let manifest_at = events
            .iter()
            .position(|event| *event == "Saving mirror manifest")
            .unwrap();
        assert!(audio_at < playlist_at && playlist_at < manifest_at);
        let playlist = fs::read_to_string(fixture.target.join("Music/00 Playlists/Mix-1.m3u"))
            .unwrap_or_else(|_| {
                let file = fs::read_dir(fixture.target.join("Music/00 Playlists"))
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .path();
                fs::read_to_string(file).unwrap()
            });
        assert_eq!(playlist.matches("../song.wav").count(), 2);
        for line in playlist
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let referenced = fixture.target.join("Music/00 Playlists").join(line);
            assert!(referenced.is_file(), "Missing mirrored audio: {line}");
        }
        assert!(fixture.target.join(".refrain/manifest.json").exists());
        assert!(!fixture.target.join(".refrain/inflight.json").exists());

        let second = run_mirror(&fixture.database, id).unwrap();
        assert_eq!((second.copied, second.unchanged), (0, 2));

        fixture.wav(1600);
        fixture.scan();
        let third = run_mirror(&fixture.database, id).unwrap();
        assert_eq!(third.copied, 1);

        fs::remove_file(fixture.source.join("song.wav")).unwrap();
        fixture.scan();
        let removed = run_mirror(&fixture.database, id).unwrap();
        assert!(removed.removed >= 1);
        assert!(!fixture.target.join("Music/song.wav").exists());
        assert_eq!(
            fs::read_to_string(fixture.target.join("Keep.txt")).unwrap(),
            "unrelated"
        );
    }

    #[test]
    fn exports_partially_available_playlists_and_restores_missing_entries_on_retry() {
        let fixture = Fixture::new("playlist-unavailable");
        fixture.wav(800);
        fs::copy(
            fixture.source.join("song.wav"),
            fixture.source.join("second.wav"),
        )
        .unwrap();
        fs::write(
            fixture.source.join("Mix.m3u"),
            "#EXTM3U\nsong.wav\nsecond.wav\nsong.wav\n",
        )
        .unwrap();
        fixture.scan();
        let id = fixture.target_id();
        let first = run_mirror(&fixture.database, id).unwrap();
        assert_eq!(first.copied, 3);

        fs::remove_file(fixture.source.join("second.wav")).unwrap();
        fixture.scan();
        let second = run_mirror(&fixture.database, id).unwrap();
        assert_eq!(second.status, "succeeded");
        let playlist_path = fs::read_dir(fixture.target.join("Music/00 Playlists"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let playlist = fs::read_to_string(&playlist_path).unwrap();
        assert!(playlist.contains("#REFRAIN-MIRROR-SKIPPED:1"));
        assert!(playlist.contains("#REFRAIN-UNAVAILABLE:"));
        assert_eq!(playlist.matches("../song.wav").count(), 2);
        assert!(!playlist.contains("../second.wav"));
        assert!(!fixture.target.join("Music/second.wav").exists());

        fs::copy(
            fixture.source.join("song.wav"),
            fixture.source.join("second.wav"),
        )
        .unwrap();
        fixture.scan();
        let restored = run_mirror(&fixture.database, id).unwrap();
        assert_eq!(restored.status, "succeeded");
        let playlist = fs::read_to_string(playlist_path).unwrap();
        assert!(!playlist.contains("#REFRAIN-MIRROR-SKIPPED"));
        assert!(!playlist.contains("#REFRAIN-UNAVAILABLE:"));
        assert_eq!(playlist.matches("../song.wav").count(), 2);
        assert_eq!(playlist.matches("../second.wav").count(), 1);
    }

    #[test]
    fn playlist_uses_mirrored_copy_when_preferred_audio_is_external() {
        let fixture = Fixture::new("playlist-external-preferred");
        fixture.wav(800);
        fs::write(fixture.source.join("Mix.m3u8"), "#EXTM3U\nsong.wav\n").unwrap();
        fixture.scan();
        let external = fixture.folder.join("external.wav");
        fs::copy(fixture.source.join("song.wav"), &external).unwrap();
        let connection = rusqlite::Connection::open(fixture.database.path()).unwrap();
        let source_path = fs::canonicalize(fixture.source.join("song.wav")).unwrap();
        connection
            .execute(
                "INSERT INTO local_files (
                    library_track_id, path, ownership, is_preferred, state,
                    format, file_size, modified_at, created_at, updated_at
                 ) SELECT library_track_id, ?1, 'external', 1, state,
                          format, file_size, modified_at, created_at, updated_at
                   FROM local_files WHERE path = ?2",
                rusqlite::params![
                    external.to_string_lossy().as_ref(),
                    source_path.to_string_lossy().as_ref()
                ],
            )
            .unwrap();
        assert_eq!(
            connection
                .execute(
                    "UPDATE local_files SET is_preferred = 0 WHERE path = ?1",
                    [source_path.to_string_lossy().as_ref()],
                )
                .unwrap(),
            1
        );
        let playlist_id = fixture.database.list_local_playlists().unwrap()[0].id;
        let detail = fixture.database.get_local_playlist(playlist_id).unwrap();
        assert_eq!(detail.entries[0].file_path.as_deref(), external.to_str());

        let run = run_mirror(&fixture.database, fixture.target_id()).unwrap();
        assert_eq!(run.copied, 2);
        let playlist = fs::read_to_string(
            fixture
                .target
                .join(format!("Music/00 Playlists/Mix-{playlist_id}.m3u")),
        )
        .unwrap();
        assert!(playlist.contains("../song.wav"));
        assert!(!playlist.contains("#REFRAIN-MIRROR-SKIPPED"));
        assert!(fixture.target.join("Music/song.wav").exists());

        // The preferred external copy can disappear between library scans.
        // The indexed file under the mirror root must remain usable.
        fs::remove_file(external).unwrap();
        let second = run_mirror(&fixture.database, fixture.target_id()).unwrap();
        assert_eq!(second.status, "succeeded");
        assert_eq!(second.unchanged, 2);
    }

    #[test]
    fn rejects_unrelated_collisions_and_modified_managed_files() {
        let fixture = Fixture::new("collisions");
        fixture.wav(800);
        fixture.scan();
        let id = fixture.target_id();
        fs::create_dir_all(fixture.target.join("Music")).unwrap();
        fs::write(fixture.target.join("Music/song.wav"), "unrelated").unwrap();
        assert!(
            run_mirror(&fixture.database, id)
                .unwrap_err()
                .contains("unrelated")
        );
        assert!(!fixture.target.join(".refrain/manifest.json").exists());
        fs::remove_file(fixture.target.join("Music/song.wav")).unwrap();
        run_mirror(&fixture.database, id).unwrap();
        fs::write(fixture.target.join("Music/song.wav"), "manually edited").unwrap();
        assert!(
            run_mirror(&fixture.database, id)
                .unwrap_err()
                .contains("externally changed")
        );
        assert_eq!(
            fs::read(fixture.target.join("Music/song.wav")).unwrap(),
            b"manually edited"
        );
    }

    #[test]
    fn interrupted_temporary_write_preserves_original() {
        let fixture = Fixture::new("partial");
        let file = fixture.target.join("result.txt");
        fs::write(&file, "original").unwrap();
        let result = replace_verified(&file, "any-hash", |output| {
            output.write_all(b"partial")?;
            Err(io::Error::new(io::ErrorKind::Interrupted, "interrupted"))
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&file).unwrap(), b"original");
    }

    #[test]
    fn retries_a_run_interrupted_after_replacing_content_but_before_manifest_commit() {
        let fixture = Fixture::new("resume");
        fixture.wav(800);
        fixture.scan();
        let id = fixture.target_id();
        run_mirror(&fixture.database, id).unwrap();
        let manifest_path = fixture.target.join(".refrain/manifest.json");
        let previous = fs::read(&manifest_path).unwrap();
        fixture.wav(1600);
        fixture.scan();
        let plan = desired_files(
            &fixture.database,
            &fs::canonicalize(&fixture.source).unwrap(),
        )
        .unwrap();
        let journal = MirrorJournal {
            format_version: 1,
            target_id: id,
            previous_manifest_hash: Some(blake3::hash(&previous).to_hex().to_string()),
            planned_entries: plan.values().map(|file| file.entry.clone()).collect(),
            recovery_entries: Vec::new(),
        };
        fs::write(
            fixture.target.join(".refrain/inflight.json"),
            serde_json::to_vec(&journal).unwrap(),
        )
        .unwrap();
        fs::copy(
            fixture.source.join("song.wav"),
            fixture.target.join("Music/song.wav"),
        )
        .unwrap();
        assert_eq!(fs::read(&manifest_path).unwrap(), previous);

        let resumed = run_mirror(&fixture.database, id).unwrap();
        assert_eq!(resumed.status, "succeeded");
        assert!(!fixture.target.join(".refrain/inflight.json").exists());
        assert_eq!(
            fs::read(fixture.source.join("song.wav")).unwrap(),
            fs::read(fixture.target.join("Music/song.wav")).unwrap()
        );
    }

    #[test]
    fn retains_recovery_hashes_when_rebased_plan_matches_old_manifest() {
        let fixture = Fixture::new("rebase-original-plan");
        fixture.wav(800);
        fixture.scan();
        let id = fixture.target_id();
        run_mirror(&fixture.database, id).unwrap();

        let manifest = fs::read(fixture.target.join(".refrain/manifest.json")).unwrap();
        let original: MirrorManifest = serde_json::from_slice(&manifest).unwrap();
        let original_audio = fs::read(fixture.source.join("song.wav")).unwrap();

        fixture.wav(1600);
        fixture.scan();
        let changed = desired_files(
            &fixture.database,
            &fs::canonicalize(&fixture.source).unwrap(),
        )
        .unwrap();
        let changed_entry = changed
            .get(Path::new("Music/song.wav"))
            .unwrap()
            .entry
            .clone();
        fs::copy(
            fixture.source.join("song.wav"),
            fixture.target.join("Music/song.wav"),
        )
        .unwrap();
        fs::write(fixture.source.join("song.wav"), original_audio).unwrap();
        fixture.scan();

        let journal = MirrorJournal {
            format_version: 1,
            target_id: id,
            previous_manifest_hash: Some(blake3::hash(&manifest).to_hex().to_string()),
            planned_entries: original.entries,
            recovery_entries: vec![changed_entry],
        };
        fs::write(
            fixture.target.join(".refrain/inflight.json"),
            serde_json::to_vec(&journal).unwrap(),
        )
        .unwrap();

        let resumed = run_mirror(&fixture.database, id).unwrap();
        assert_eq!(resumed.status, "succeeded");
        assert_eq!(resumed.copied, 1);
        assert!(!fixture.target.join(".refrain/inflight.json").exists());
        assert_eq!(
            fs::read(fixture.target.join("Music/song.wav")).unwrap(),
            fs::read(fixture.source.join("song.wav")).unwrap()
        );
    }

    #[test]
    fn source_changes_after_interruption_are_reconciled() {
        let fixture = Fixture::new("changed-after-interruption");
        fixture.wav(800);
        fixture.scan();
        let id = fixture.target_id();
        run_mirror(&fixture.database, id).unwrap();
        let previous_manifest = fs::read(fixture.target.join(".refrain/manifest.json")).unwrap();

        fixture.wav(1600);
        fixture.scan();
        let interrupted = desired_files(
            &fixture.database,
            &fs::canonicalize(&fixture.source).unwrap(),
        )
        .unwrap();
        let orphan = MirrorEntry {
            relative_path: "Playlists/obsolete.m3u8".into(),
            source_key: "interrupted-playlist".into(),
            file_size: 4,
            content_hash: blake3::hash(b"test").to_hex().to_string(),
            source_modified_ns: None,
            destination_modified_ns: None,
        };
        let journal = MirrorJournal {
            format_version: 1,
            target_id: id,
            previous_manifest_hash: Some(blake3::hash(&previous_manifest).to_hex().to_string()),
            planned_entries: interrupted
                .values()
                .map(|file| file.entry.clone())
                .chain(Some(orphan))
                .collect(),
            recovery_entries: Vec::new(),
        };
        fs::write(
            fixture.target.join(".refrain/inflight.json"),
            serde_json::to_vec(&journal).unwrap(),
        )
        .unwrap();
        fs::create_dir_all(fixture.target.join("Playlists")).unwrap();
        fs::write(fixture.target.join("Playlists/obsolete.m3u8"), b"test").unwrap();
        fs::copy(
            fixture.source.join("song.wav"),
            fixture.target.join("Music/song.wav"),
        )
        .unwrap();

        // The interrupted 1600-byte copy is no longer the desired snapshot.
        fixture.wav(2400);
        fixture.scan();
        let resumed = run_mirror(&fixture.database, id).unwrap();
        assert_eq!(resumed.status, "succeeded");
        assert_eq!(resumed.removed, 1);
        assert!(!fixture.target.join("Playlists/obsolete.m3u8").exists());
        assert!(!fixture.target.join(".refrain/inflight.json").exists());
        assert_eq!(
            fs::read(fixture.source.join("song.wav")).unwrap(),
            fs::read(fixture.target.join("Music/song.wav")).unwrap()
        );
    }

    #[test]
    fn rebased_journal_preserves_earlier_interrupted_copies_on_another_failure() {
        let fixture = Fixture::new("rebase-twice");
        fixture.wav(800);
        fs::copy(
            fixture.source.join("song.wav"),
            fixture.source.join("other.wav"),
        )
        .unwrap();
        fixture.scan();
        let id = fixture.target_id();
        run_mirror(&fixture.database, id).unwrap();
        let previous = fs::read(fixture.target.join(".refrain/manifest.json")).unwrap();

        fixture.wav(1600);
        fs::copy(
            fixture.source.join("song.wav"),
            fixture.source.join("other.wav"),
        )
        .unwrap();
        fixture.scan();
        let interrupted = desired_files(
            &fixture.database,
            &fs::canonicalize(&fixture.source).unwrap(),
        )
        .unwrap();
        let intermediate_hash = interrupted
            .get(Path::new("Music/song.wav"))
            .unwrap()
            .entry
            .content_hash
            .clone();
        fs::write(
            fixture.target.join(".refrain/inflight.json"),
            serde_json::to_vec(&MirrorJournal {
                format_version: 1,
                target_id: id,
                previous_manifest_hash: Some(blake3::hash(&previous).to_hex().to_string()),
                planned_entries: interrupted
                    .values()
                    .map(|file| file.entry.clone())
                    .collect(),
                recovery_entries: Vec::new(),
            })
            .unwrap(),
        )
        .unwrap();
        for name in ["song.wav", "other.wav"] {
            fs::copy(
                fixture.source.join(name),
                fixture.target.join("Music").join(name),
            )
            .unwrap();
        }

        fixture.wav(2400);
        fs::copy(
            fixture.source.join("song.wav"),
            fixture.source.join("other.wav"),
        )
        .unwrap();
        fixture.scan();
        let newest_song = fs::read(fixture.source.join("song.wav")).unwrap();
        let failed = run_mirror_with_progress(&fixture.database, id, |progress| {
            if progress.phase == "copying" && progress.completed == 1 {
                fs::remove_file(fixture.source.join("song.wav")).unwrap();
            }
        });
        assert!(failed.is_err());
        let rebased: MirrorJournal = serde_json::from_slice(
            &fs::read(fixture.target.join(".refrain/inflight.json")).unwrap(),
        )
        .unwrap();
        assert!(rebased.recovery_entries.iter().any(|entry| {
            entry.relative_path == "Music/song.wav" && entry.content_hash == intermediate_hash
        }));
        fs::write(fixture.source.join("song.wav"), newest_song).unwrap();
        let resumed = run_mirror(&fixture.database, id).unwrap();
        assert_eq!(resumed.status, "succeeded");
        assert!(!fixture.target.join(".refrain/inflight.json").exists());
        for name in ["song.wav", "other.wav"] {
            assert_eq!(
                fs::read(fixture.source.join(name)).unwrap(),
                fs::read(fixture.target.join("Music").join(name)).unwrap()
            );
        }
    }

    #[test]
    fn rejects_a_replacement_directory_at_the_same_destination_path() {
        let fixture = Fixture::new("replaced-target");
        fixture.wav(800);
        fixture.scan();
        let id = fixture.target_id();
        let old_target = fixture.folder.join("disconnected-drive");
        fs::rename(&fixture.target, &old_target).unwrap();
        fs::create_dir(&fixture.target).unwrap();
        assert!(
            run_mirror(&fixture.database, id)
                .unwrap_err()
                .contains("identity changed")
        );
        assert!(
            save_target(&fixture.database, fixture.target.to_str().unwrap())
                .unwrap_err()
                .contains("identity changed")
        );
        assert!(!fixture.target.join("Music/song.wav").exists());
    }

    #[test]
    fn removed_and_restored_target_keeps_its_history_and_unchanged_files() {
        let fixture = Fixture::new("restore-target");
        fixture.wav(800);
        fixture.scan();
        let id = fixture.target_id();
        assert_eq!(run_mirror(&fixture.database, id).unwrap().copied, 1);
        fixture.database.remove_mirror_target(id).unwrap();
        assert!(fixture.database.list_mirror_targets().unwrap().is_empty());
        let restored = save_target(&fixture.database, fixture.target.to_str().unwrap()).unwrap();
        assert_eq!(restored.id, id);
        assert_eq!(restored.last_run.unwrap().status, "succeeded");
        let repeated = run_mirror(&fixture.database, id).unwrap();
        assert_eq!((repeated.copied, repeated.unchanged), (0, 1));
        assert_eq!(fixture.database.list_mirror_runs(id).unwrap().len(), 2);
    }

    #[test]
    fn finds_reconnected_volume_under_a_different_mount_name() {
        let fixture = Fixture::new("volume-rename");
        fixture.wav(800);
        fixture.scan();
        let id = fixture.target_id();
        run_mirror(&fixture.database, id).unwrap();

        let renamed = fixture.folder.join("reconnected");
        fs::rename(&fixture.target, &renamed).unwrap();
        let saved = fixture.database.list_mirror_targets().unwrap().remove(0);
        assert!(!target_available(&saved));
        let (found_path, _) =
            find_remounted_volume(&saved, &fs::canonicalize(&fixture.folder).unwrap())
                .unwrap()
                .expect("manifest should identify the remounted folder");
        assert_eq!(found_path, fs::canonicalize(&renamed).unwrap());

        let restored = save_target(&fixture.database, found_path.to_str().unwrap()).unwrap();
        assert_eq!(restored.id, id);
        assert_eq!(run_mirror(&fixture.database, id).unwrap().unchanged, 1);
    }

    #[test]
    fn remount_with_new_filesystem_identity_reuses_on_device_manifest() {
        let fixture = Fixture::new("remounted-target");
        fixture.wav(800);
        fixture.scan();
        let id = fixture.target_id();
        run_mirror(&fixture.database, id).unwrap();
        let old_identity = destination_identity(&fixture.target).unwrap();

        let moved = fixture.folder.join("old-mount");
        fs::rename(&fixture.target, &moved).unwrap();
        fs::create_dir(&fixture.target).unwrap();
        fs::create_dir_all(fixture.target.join(".refrain")).unwrap();
        fs::create_dir_all(fixture.target.join("Music")).unwrap();
        fs::copy(
            moved.join(".refrain/manifest.json"),
            fixture.target.join(".refrain/manifest.json"),
        )
        .unwrap();
        fs::copy(
            moved.join("Music/song.wav"),
            fixture.target.join("Music/song.wav"),
        )
        .unwrap();
        assert_ne!(destination_identity(&fixture.target).unwrap(), old_identity);

        let target = fixture.database.list_mirror_targets().unwrap().remove(0);
        assert!(target_available(&target));
        let result = run_mirror(&fixture.database, id).unwrap();
        assert_eq!((result.copied, result.unchanged), (0, 1));
        assert_eq!(
            fixture.database.list_mirror_targets().unwrap()[0].identity,
            destination_identity(&fixture.target).unwrap()
        );
    }

    #[test]
    fn aborts_if_destination_is_replaced_during_a_run() {
        let fixture = Fixture::new("swapped-mid-run");
        fixture.wav(800);
        fixture.scan();
        let id = fixture.target_id();
        let displaced = fixture.folder.join("original-drive");
        let result = run_mirror_with_progress(&fixture.database, id, |progress| {
            if progress.phase == "copying" && progress.completed == 0 {
                fs::rename(&fixture.target, &displaced).unwrap();
                fs::create_dir(&fixture.target).unwrap();
            }
        });
        assert!(result.unwrap_err().contains("identity changed"));
        assert!(!fixture.target.join("Music/song.wav").exists());
        assert!(!fixture.target.join(".refrain/inflight.json").exists());
    }

    #[test]
    fn rejects_a_managed_file_edited_after_preflight() {
        let fixture = Fixture::new("edited-mid-run");
        fixture.wav(800);
        fixture.scan();
        let id = fixture.target_id();
        run_mirror(&fixture.database, id).unwrap();
        fixture.wav(1600);
        fixture.scan();
        let result = run_mirror_with_progress(&fixture.database, id, |progress| {
            if progress.phase == "copying" && progress.completed == 0 {
                fs::write(fixture.target.join("Music/song.wav"), b"external edit").unwrap();
            }
        });
        assert!(result.unwrap_err().contains("externally changed"));
        assert_eq!(
            fs::read(fixture.target.join("Music/song.wav")).unwrap(),
            b"external edit"
        );
    }

    #[test]
    fn destination_content_is_checked_only_during_planning() {
        let fixture = Fixture::new("planned-once");
        fixture.wav(800);
        fixture.scan();
        let id = fixture.target_id();
        run_mirror(&fixture.database, id).unwrap();

        // Invalidate the destination metadata cache without changing content.
        // Planning must hash it to establish ownership of the old contents.
        let output = fixture.target.join("Music/song.wav");
        File::options()
            .write(true)
            .open(&output)
            .unwrap()
            .set_times(
                fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(1_500_000_000)),
            )
            .unwrap();
        fixture.wav(1600);
        fixture.scan();
        let events = std::cell::RefCell::new(Vec::new());
        let result = run_mirror_with_progress(&fixture.database, id, |event| {
            events.borrow_mut().push(event);
        })
        .unwrap();
        assert_eq!(result.copied, 1);
        let events = events.into_inner();
        assert!(events.iter().any(|event| {
            event.operation == "Checking destination"
                && event.current_file.as_deref() == Some("Music/song.wav")
                && event.file_bytes_total > 0
                && event.file_bytes_done == event.file_bytes_total
        }));
        let execution_started = events
            .iter()
            .position(|event| event.operation == "Starting file updates")
            .unwrap();
        assert!(
            !events[execution_started..]
                .iter()
                .any(|event| event.operation == "Checking destination")
        );
    }

    #[test]
    fn refuses_a_destination_changed_while_copying_without_overwriting_it() {
        let fixture = Fixture::new("edited-during-copy");
        fixture.wav(800);
        fixture.scan();
        let id = fixture.target_id();
        run_mirror(&fixture.database, id).unwrap();
        let old_manifest = fs::read(fixture.target.join(".refrain/manifest.json")).unwrap();
        fixture.wav(1600);
        fixture.scan();

        let result = run_mirror_with_progress(&fixture.database, id, |event| {
            if event.operation == "Verifying copied file" && event.file_bytes_done == 0 {
                fs::write(fixture.target.join("Music/song.wav"), b"external edit").unwrap();
            }
        });
        assert!(result.unwrap_err().contains("externally changed"));
        assert_eq!(
            fs::read(fixture.target.join("Music/song.wav")).unwrap(),
            b"external edit"
        );
        assert_eq!(
            fs::read(fixture.target.join(".refrain/manifest.json")).unwrap(),
            old_manifest
        );
    }

    #[test]
    fn rejects_a_stale_managed_file_edited_after_preflight() {
        let fixture = Fixture::new("edited-stale-mid-run");
        fixture.wav(800);
        fixture.scan();
        let id = fixture.target_id();
        run_mirror(&fixture.database, id).unwrap();
        fs::remove_file(fixture.source.join("song.wav")).unwrap();
        fixture.scan();
        let result = run_mirror_with_progress(&fixture.database, id, |progress| {
            if progress.phase == "copying" && progress.completed == 0 {
                fs::write(fixture.target.join("Music/song.wav"), b"external edit").unwrap();
            }
        });
        assert!(result.unwrap_err().contains("externally changed"));
        assert_eq!(
            fs::read(fixture.target.join("Music/song.wav")).unwrap(),
            b"external edit"
        );
    }

    #[test]
    fn interrupted_run_is_marked_failed_on_database_restart() {
        let fixture = Fixture::new("interrupted-record");
        let id = fixture.target_id();
        fixture.database.start_mirror_run(id).unwrap();
        let reopened = Database::open(fixture.folder.join("test.sqlite3")).unwrap();
        let runs = reopened.list_mirror_runs(id).unwrap();
        assert_eq!(runs[0].status, "failed");
        assert!(runs[0].error.as_ref().unwrap().contains("Retry"));
    }
}
