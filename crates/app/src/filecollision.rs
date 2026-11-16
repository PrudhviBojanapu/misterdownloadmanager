//! Filename conflicts are independent of link identity: signed links can change.

use std::path::{Path, PathBuf};
use std::time::Duration;

use mdm_engine::naming::{part_path, reserve};
use mdm_engine::state::{ResumeState, state_path};
use url::Url;

use crate::dialogs::{self, FileInfo, Outcome, Start};
use crate::launch::Asked;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Disk {
    pub taken: bool,
    pub regular: bool,
    pub partial: bool,
    pub resume: Option<ResumeState>,
}

fn metadata(path: &Path) -> Result<Option<std::fs::Metadata>, String> {
    match std::fs::metadata(path) {
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

pub(crate) fn inspect(dest: &Path) -> Result<Disk, String> {
    let file = metadata(dest)?;
    let part = metadata(&part_path(dest))?;
    let state = metadata(&state_path(dest))?;
    let resume = if state.is_some() && part.is_some() {
        // A broken/foreign sidecar is still a conflict, never permission to start over.
        ResumeState::load(&state_path(dest)).ok().filter(|s| {
            same_path(&s.dest, dest)
                && part
                    .as_ref()
                    .is_some_and(|p| p.is_file() && p.len() == s.size)
                && s.downloaded() > 0
                && s.downloaded() < s.size
        })
    } else {
        None
    };
    Ok(Disk {
        taken: file.is_some() || part.is_some() || state.is_some(),
        regular: file.as_ref().is_none_or(|f| f.is_file()),
        partial: part.is_some() || state.is_some(),
        resume,
    })
}

pub(crate) fn same_path(a: &Path, b: &Path) -> bool {
    fn normalized(path: &Path) -> PathBuf {
        let mut out = PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    out.pop();
                }
                _ => out.push(component.as_os_str()),
            }
        }
        out
    }
    let (a, b) = (normalized(a), normalized(b));
    #[cfg(windows)]
    return a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase();
    #[cfg(not(windows))]
    return a == b;
}

pub(crate) fn checked(
    asked: &Asked<PathBuf, Result<Disk, String>>,
    dest: &Path,
    ctx: &egui::Context,
) -> Option<Result<Disk, String>> {
    asked.get(
        &dest.to_owned(),
        Duration::MAX,
        Duration::ZERO,
        Some(ctx),
        |p| inspect(p),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Choice {
    Numbered,
    Overwrite,
    Resume,
}

pub(crate) struct Existing {
    pub id: u64,
    pub active: bool,
    pub plain: bool,
    pub url: Url,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Prepared {
    New,
    Numbered(PathBuf),
    Overwrite,
    Resume(ResumeState),
    Existing(u64),
}

fn numbered(dest: &Path, occupied: &[PathBuf]) -> Result<Prepared, String> {
    let dir = dest
        .parent()
        .ok_or_else(|| crate::i18n::t("Invalid destination").to_owned())?;
    let name = dest
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| crate::i18n::t("Invalid destination").to_owned())?;
    let mut taken = occupied.iter().cloned().collect();
    Ok(Prepared::Numbered(crate::app::place_in(
        dir, name, &mut taken,
    )))
}

fn prepare(
    choice: Choice,
    dest: &Path,
    url: &Url,
    size: Option<u64>,
    owner: Option<&Url>,
) -> Result<Prepared, String> {
    let _hold = reserve(dest).ok_or_else(|| {
        crate::i18n::t("This file is in use. Wait for its download to stop.").to_owned()
    })?;
    let disk = inspect(dest)?;
    match choice {
        Choice::Overwrite if disk.regular => {
            // Only confirmed, inactive partial data is discarded. A completed file stays
            // until the replacement finishes successfully (Options::replace_existing).
            for path in [part_path(dest), state_path(dest)] {
                match std::fs::remove_file(&path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(format!("{}: {e}", path.display())),
                }
            }
            Ok(Prepared::Overwrite)
        }
        Choice::Resume => {
            let mut saved = disk.resume.ok_or_else(|| {
                crate::i18n::t("No valid unfinished download was found.").to_owned()
            })?;
            if size.is_some_and(|n| n != saved.size) || owner.is_some_and(|u| *u != saved.url) {
                return Err(
                    crate::i18n::t("The partial download does not match this file.").into(),
                );
            }
            saved.url = url.clone();
            saved.final_url = url.clone();
            // Keep validators, segment boundaries and checksum. The engine verifies range
            // answers and overlap bytes before extending the partial download.
            saved.write(&state_path(dest)).map_err(|e| e.to_string())?;
            Ok(Prepared::Resume(saved))
        }
        _ => Err(crate::i18n::t("This file cannot be overwritten.").into()),
    }
}

pub(crate) struct PendingFile {
    pub fi: Box<FileInfo>,
    pub start: Start,
    disk: Asked<PathBuf, Result<Disk, String>>,
    preparing: Asked<Choice, Result<Prepared, String>>,
    action: Option<Choice>,
    dialog: dialogs::Duplicate,
    error: Option<String>,
}

impl PendingFile {
    pub fn new(fi: FileInfo, start: Start) -> Self {
        let dialog = dialogs::Duplicate::for_file(fi.dest().display().to_string());
        Self {
            fi: Box::new(fi),
            start,
            disk: Default::default(),
            preparing: Default::default(),
            action: None,
            dialog,
            error: None,
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        existing: Option<&Existing>,
        occupied: &[PathBuf],
    ) -> Outcome<Prepared> {
        let dest = self.fi.dest();
        let Some(disk) = checked(&self.disk, &dest, ctx) else {
            return Outcome::Open;
        };
        let disk = match disk {
            Ok(d) => d,
            Err(e) => {
                return match dialogs::message(ctx, "file-conflict-error", "Download error", &e) {
                    Outcome::Open => Outcome::Open,
                    _ => Outcome::Closed,
                };
            }
        };
        if !disk.taken && existing.is_none() {
            return Outcome::Done(Prepared::New);
        }
        let active = existing.is_some_and(|e| e.active);
        let plain = existing.is_none_or(|e| e.plain);
        let overwrite = !active && plain && disk.regular;
        let resume = plain
            && (active
                || disk.resume.as_ref().is_some_and(|s| {
                    self.fi.size().is_none_or(|n| n == s.size)
                        && existing.is_none_or(|e| e.url == s.url)
                }));
        if self.action.is_none() && self.fi.replaces().is_some() && overwrite {
            self.action = Some(Choice::Overwrite);
        }
        if let Some(action) = self.action {
            let url = self.fi.url.clone();
            let size = self.fi.size();
            let owner = existing.map(|e| e.url.clone());
            let occupied = occupied.to_vec();
            let answer = self.preparing.get(
                &action,
                Duration::MAX,
                Duration::ZERO,
                Some(ctx),
                move |choice| match choice {
                    Choice::Numbered => numbered(&dest, &occupied),
                    _ => prepare(*choice, &dest, &url, size, owner.as_ref()),
                },
            );
            match answer {
                Some(Ok(prepared)) => return Outcome::Done(prepared),
                Some(Err(e)) => {
                    self.error = Some(e);
                    self.action = None;
                    self.preparing = Default::default();
                }
                None => {}
            }
        }
        let waiting = self.action.is_some();
        let note = if waiting {
            crate::i18n::t("Preparing download...")
        } else if let Some(e) = self.error.as_deref() {
            e
        } else if active {
            crate::i18n::t("The existing download is running. It will not be overwritten.")
        } else if !resume && disk.partial {
            crate::i18n::t("Resume unavailable: missing, invalid or mismatched partial download.")
        } else {
            crate::i18n::t("Overwrite discards partial data; completed files stay until success.")
        };
        self.dialog.file_state(overwrite, resume, note, waiting);
        match self.dialog.show(ctx) {
            Outcome::Closed => Outcome::Closed,
            Outcome::Done(3) if active => Outcome::Done(Prepared::Existing(existing.unwrap().id)),
            Outcome::Done(choice @ 1..=3) => {
                self.action = Some(match choice {
                    1 => Choice::Numbered,
                    2 => Choice::Overwrite,
                    _ => Choice::Resume,
                });
                ctx.request_repaint();
                Outcome::Open
            }
            Outcome::Done(_) => Outcome::Closed,
            Outcome::Open => Outcome::Open,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(token: &str) -> Url {
        Url::parse(&format!("https://files.test/file.bin?token={token}")).unwrap()
    }

    fn partial(dest: &Path) -> ResumeState {
        let mut saved =
            ResumeState::new(url("old"), url("old"), dest.into(), 100, Default::default());
        saved.segments = vec![mdm_engine::planner::SavedSegment {
            start: 0,
            cursor: 40,
            end: 100,
        }];
        std::fs::write(part_path(dest), [7; 100]).unwrap();
        saved.write(&state_path(dest)).unwrap();
        saved
    }

    #[test]
    fn detects_completed_and_partial_files_without_numbering_or_mutation() {
        let d = tempfile::tempdir().unwrap();
        let dest = d.path().join("file.bin");
        assert!(!inspect(&dest).unwrap().taken);
        std::fs::write(&dest, b"finished").unwrap();
        let disk = inspect(&dest).unwrap();
        assert!(disk.taken && disk.resume.is_none());
        assert_eq!(std::fs::read(&dest).unwrap(), b"finished");
        let saved = partial(&dest);
        assert_eq!(inspect(&dest).unwrap().resume, Some(saved));
        assert!(!inspect(&d.path().join("other.bin")).unwrap().taken);
        assert!(
            !inspect(&d.path().join("other").join("file.bin"))
                .unwrap()
                .taken
        );
    }

    #[test]
    fn resume_refreshes_signed_link_but_preserves_data_and_validation() {
        let d = tempfile::tempdir().unwrap();
        let dest = d.path().join("file.bin");
        let mut old = partial(&dest);
        old.validators = mdm_engine::state::Validators::from_response(
            Some("\"version-one\""),
            None,
            None,
            Some("files.test"),
        );
        old.write(&state_path(&dest)).unwrap();
        let result = prepare(
            Choice::Resume,
            &dest,
            &url("new"),
            Some(100),
            Some(&url("old")),
        )
        .unwrap();
        let Prepared::Resume(saved) = result else {
            panic!("not resumed")
        };
        assert_eq!(saved.url, url("new"));
        assert_eq!(saved.final_url, url("new"));
        assert_eq!(saved.validators, old.validators);
        assert_eq!(saved.segments, old.segments);
        assert_eq!(std::fs::read(part_path(&dest)).unwrap(), vec![7; 100]);
    }

    #[test]
    fn rejects_wrong_size_foreign_state_and_active_download_without_touching_data() {
        let d = tempfile::tempdir().unwrap();
        let dest = d.path().join("file.bin");
        let old = partial(&dest);
        for (size, owner) in [(Some(99), url("old")), (Some(100), url("foreign"))] {
            assert!(prepare(Choice::Resume, &dest, &url("new"), size, Some(&owner)).is_err());
            assert_eq!(ResumeState::load(&state_path(&dest)).unwrap(), old);
        }
        let _hold = reserve(&dest).unwrap();
        assert!(prepare(Choice::Overwrite, &dest, &url("new"), None, None).is_err());
        assert_eq!(std::fs::read(part_path(&dest)).unwrap(), vec![7; 100]);
    }

    #[test]
    fn overwrite_only_discards_confirmed_partial_data_not_completed_file() {
        let d = tempfile::tempdir().unwrap();
        let dest = d.path().join("file.bin");
        std::fs::write(&dest, b"finished").unwrap();
        partial(&dest);
        assert_eq!(
            prepare(Choice::Overwrite, &dest, &url("new"), None, None).unwrap(),
            Prepared::Overwrite
        );
        assert!(!part_path(&dest).exists());
        assert!(!state_path(&dest).exists());
        assert_eq!(std::fs::read(&dest).unwrap(), b"finished");
    }

    #[test]
    fn numbered_copy_skips_disk_files_and_queued_names_without_changing_them() {
        let d = tempfile::tempdir().unwrap();
        let dest = d.path().join("file.bin");
        let queued = d.path().join("file (1).bin");
        std::fs::write(&dest, b"finished").unwrap();
        assert_eq!(
            numbered(&dest, &[queued.clone()]).unwrap(),
            Prepared::Numbered(d.path().join("file (2).bin"))
        );
        assert_eq!(std::fs::read(&dest).unwrap(), b"finished");
        assert!(!queued.exists());
    }
}
