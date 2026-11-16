//! Torrents through the service (#44): a magnet link goes to the user's account, the service
//! downloads the torrent on its own servers, and its files then come over HTTP through MDM's
//! engine (every connection, resume). Nothing connects to peers from this computer. A `.torrent`
//! file is sent as its magnet link (the app makes it: its hash, name and trackers).
//!
//! TorBox's documented API:
//! - TorBox: `torrents/createtorrent` (`magnet`), `torrents/mylist?id=`, `torrents/requestdl` for
//!   each file (a direct link), `torrents/controltorrent` (`delete`).

use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mdm_engine::download::{SegmentView, State};
use mdm_engine::{Destination, Download, ServiceFault, Snapshot};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::{Debrid, Fault, ServiceKind, Session, torbox, unsupported_service};

/// How the service is doing with a torrent.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// Still at it: how far (0 to 1, when it says), and its own word for it ("downloading").
    Working {
        progress: Option<f32>,
        what: String,
    },
    /// Done on its servers: the torrent's name and files.
    Ready {
        name: String,
        files: Vec<RemoteFile>,
    },
    Failed(ServiceFault),
}

/// One finished file at the service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteFile {
    /// Its path in the torrent, `/`-separated (empty: the service names it when it is unlocked).
    pub path: String,
    /// 0: unknown.
    pub size: u64,
    /// A direct link to the user's TorBox file. Never shown.
    pub link: String,
}

impl Session {
    /// Send `magnet` to the service: its id for the torrent there.
    async fn torrent_add(&self, magnet: &str) -> Result<String, Fault> {
        match self.account.service {
            ServiceKind::TorBox => {
                let key = self.token(None).await?;
                let id = torbox::create(&self.http, &key, torbox::Kind::Torrent, "magnet", magnet)
                    .await?;
                Ok(id.to_string())
            }
            service => Err(unsupported_service(service)),
        }
    }

    /// The files ticked in Add torrent are unused: TorBox fetches the whole torrent.
    async fn torrent_status(&self, id: &str, _chosen: Option<&[String]>) -> Result<Status, Fault> {
        match self.account.service {
            ServiceKind::TorBox => {
                let (kind, n) = (torbox::Kind::Torrent, torbox::torrent_id(id)?);
                let key = self.token(None).await?;
                let i = torbox::item(&self.http, &key, kind, n).await?;
                match torbox::state(&i) {
                    Some(s) => Ok(s),
                    None => Ok(Status::Ready {
                        files: torbox::files(&self.http, &key, kind, n, &i).await?,
                        name: i.name,
                    }),
                }
            }
            service => Err(unsupported_service(service)),
        }
    }

    async fn torrent_delete(&self, id: &str) -> Result<(), Fault> {
        match self.account.service {
            ServiceKind::TorBox => {
                let n = torbox::torrent_id(id)?;
                torbox::delete(
                    &self.http,
                    &self.token(None).await?,
                    torbox::Kind::Torrent,
                    n,
                )
                .await
            }
            service => Err(unsupported_service(service)),
        }
    }
}

impl Debrid {
    /// The account torrents go to: the first one whose service takes magnets.
    pub fn torrent_account(&self) -> Option<(u64, ServiceKind)> {
        self.sessions
            .read()
            .unwrap()
            .iter()
            .find(|s| s.account.service.takes_magnets())
            .map(|s| (s.account.id, s.account.service))
    }

    pub(crate) fn session(&self, id: u64) -> Option<Arc<Session>> {
        self.sessions
            .read()
            .unwrap()
            .iter()
            .find(|s| s.account.id == id)
            .cloned()
    }
}

/// A path from the service, made safe to write under the torrent's folder: each part a file
/// name MDM accepts, no `..`, no root, nothing empty. None: nothing usable is left.
pub fn safe_path(path: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for part in path.split(['/', '\\']) {
        let part = part.trim();
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return None;
        }
        let clean = mdm_engine::naming::sanitize(part);
        if clean.is_empty()
            || Path::new(&clean)
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return None;
        }
        out.push(clean);
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// What the app gives a torrent through the service.
#[derive(Clone, Debug)]
pub struct Order {
    /// The account it goes to.
    pub account: u64,
    /// The magnet link (also the list's address).
    pub magnet: Url,
    /// Its id at the service, when it was sent already (a resumed download).
    pub transfer: Option<String>,
    /// Its folder: the files go under it, in their paths.
    pub folder: PathBuf,
    /// Custom file or folder name chosen in Add torrent (#517), when renamed from the torrent's own.
    pub renamed: Option<String>,
    /// The files ticked in Add torrent, by their path in the torrent (`/`-separated, relative to
    /// its name, as `.torrent` files list them; never a padding file). None: all of them. Matched
    /// to the service's list by path (`chosen`), never by index: each service numbers files its
    /// own way.
    pub files: Option<Vec<String>>,
    /// The downloads' options (connections, proxy, DNS, adapter, speed limit); the unlocker is
    /// set here.
    pub net: mdm_engine::Options,
    /// How often the service is asked while it works.
    pub poll: Duration,
}

#[derive(Default)]
struct Shared {
    state: Option<State>,
    transfer: Option<String>,
    /// The event log: its own notes and the finished files' events, in order (stable, so the
    /// list copies each once); the running file's follow.
    events: Vec<(std::time::SystemTime, String)>,
    error: Option<String>,
    failure: Option<ServiceFault>,
    /// The service's name, for the failure.
    service: &'static str,
    /// The service at work on it: how far, when it says (`at_service`).
    working: Option<Option<f32>>,
    /// Bytes of the files done, and of all of them.
    done: u64,
    total: Option<u64>,
    current: Option<Download>,
    folder: Option<PathBuf>,
    saved_files: Vec<PathBuf>,
    preexisting_folder: bool,
}

/// A torrent going through the service, then its files downloading: what the list shows as one
/// download.
#[derive(Clone)]
pub struct RemoteTorrent {
    shared: Arc<Mutex<Shared>>,
    cancel: CancellationToken,
    url: Url,
}

impl std::fmt::Debug for RemoteTorrent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RemoteTorrent")
    }
}

impl RemoteTorrent {
    /// Start (or continue) `order` through `debrid`'s account. Runs on the current tokio runtime.
    pub fn start(debrid: &Debrid, order: Order) -> RemoteTorrent {
        let shared = Arc::new(Mutex::new(Shared {
            state: Some(State::Probing),
            transfer: order.transfer.clone(),
            ..Default::default()
        }));
        let me = RemoteTorrent {
            shared,
            cancel: CancellationToken::new(),
            url: order.magnet.clone(),
        };
        let session = debrid
            .session(order.account)
            .ok_or_else(|| debrid.account_fault(order.account));
        if let Ok(s) = &session {
            me.shared.lock().unwrap().service = s.account.service.name();
        }
        let run = me.clone();
        tokio::spawn(async move {
            let out = match session {
                Ok(s) => run.run(s, order).await,
                Err(f) => Err(f),
            };
            let mut sh = run.shared.lock().unwrap();
            sh.current = None;
            match out {
                Ok(()) => sh.state = Some(State::Done),
                Err(_) if run.cancel.is_cancelled() => sh.state = Some(State::Cancelled),
                Err(f) => {
                    let f = f.service();
                    sh.error = Some(f.to_string());
                    sh.failure = Some(f);
                    sh.state = Some(State::Failed);
                }
            }
        });
        me
    }

    fn note(&self, text: String) {
        self.shared
            .lock()
            .unwrap()
            .events
            .push((std::time::SystemTime::now(), text));
    }

    async fn run(&self, s: Arc<Session>, order: Order) -> Result<(), Fault> {
        let service = s.account.service.name();
        let cancelled = || Fault::Service(ServiceFault::Other("cancelled".into()));
        let known = self.shared.lock().unwrap().transfer.clone();
        let id = match known {
            Some(id) => id,
            None => {
                let id = s.torrent_add(order.magnet.as_str()).await?;
                self.note(format!("Sent to {service}"));
                self.shared.lock().unwrap().transfer = Some(id.clone());
                id
            }
        };
        // The service at work: asked every `poll`, the event log told when its word or the
        // percentage (by tens) changes.
        let mut said = String::new();
        let (name, files) = loop {
            match s.torrent_status(&id, order.files.as_deref()).await? {
                Status::Ready { name, files } => break (name, files),
                Status::Failed(f) => return Err(Fault::Service(f)),
                Status::Working { progress, what } => {
                    self.shared.lock().unwrap().working = Some(progress);
                    let line = match progress {
                        Some(p) => format!(
                            "{service} is downloading the torrent: {what}, {}%",
                            ((p.clamp(0.0, 1.0) * 10.0).floor() * 10.0) as u32
                        ),
                        None => format!("{service} is downloading the torrent: {what}"),
                    };
                    if line != said {
                        self.note(line.clone());
                        said = line;
                    }
                }
            }
            tokio::select! {
                _ = tokio::time::sleep(order.poll) => {}
                _ = self.cancel.cancelled() => return Err(cancelled()),
            }
        };
        self.shared.lock().unwrap().working = None;
        // Several files in the torrent: also when only one of them was ticked (it keeps its
        // place in the torrent's folder, as when downloaded from peers).
        let multi = files.len() > 1 || order.files.is_some();
        let files = match &order.files {
            // Real-Debrid was asked for those only (`selectFiles`): its list is theirs.
            Some(_) if s.account.service == ServiceKind::RealDebrid => {
                self.note(format!(
                    "{service} has the {} file(s) chosen in Add torrent, downloading them",
                    files.len()
                ));
                files
            }
            Some(wanted) => {
                let all = files.len();
                let files = chosen(files, &name, wanted)
                    .map_err(|p| Fault::Service(ServiceFault::ChosenFileMissing(p)))?;
                self.note(format!(
                    "{service} has the torrent: {all} file(s), downloading the {} chosen in Add torrent",
                    files.len()
                ));
                files
            }
            None => {
                self.note(format!(
                    "{service} has the torrent: {} file(s), downloading them",
                    files.len()
                ));
                files
            }
        };
        let total: u64 = files.iter().map(|f| f.size).sum();
        self.shared.lock().unwrap().total = (total > 0).then_some(total);
        // TorBox's file links are direct, not file-host links to unlock again.
        // As torrent programs do: one file goes into the folder itself, several into a folder
        // named after the torrent (a path that already starts with that name isn't doubled).
        let folder = if !multi {
            order.folder.clone()
        } else {
            let folder_name = order
                .renamed
                .as_deref()
                .and_then(safe_path)
                .or_else(|| safe_path(&name))
                .unwrap_or_else(|| PathBuf::from("torrent"));
            order.folder.join(folder_name)
        };
        let within = |path: &str| {
            let t = path.trim_start_matches(['/', '\\']);
            // The one file by its name, whatever folder the service keeps it in (TorBox's
            // `<torrent>/<file>`).
            if !multi {
                return t.rsplit(['/', '\\']).next().unwrap_or(t).to_owned();
            }
            match t.split_once(['/', '\\']) {
                Some((first, rest)) if first.trim() == name.trim() => rest.to_owned(),
                _ => t.to_owned(),
            }
        };
        if multi
            && order.transfer.is_none()
            && std::fs::read_dir(&folder).is_ok_and(|mut rd| rd.next().is_some())
        {
            self.shared.lock().unwrap().preexisting_folder = true;
        }
        std::fs::create_dir_all(&folder)
            .map_err(|e| Fault::Service(ServiceFault::Other(e.to_string())))?;
        for f in &files {
            let link = Url::parse(&f.link).map_err(|_| {
                Fault::Service(ServiceFault::Other(
                    "the service gave a link MDM can't read".into(),
                ))
            })?;
            let chosen_rel = if !multi {
                order
                    .renamed
                    .as_deref()
                    .and_then(safe_path)
                    .or_else(|| safe_path(&within(&f.path)))
            } else {
                safe_path(&within(&f.path))
            };
            let dest = match chosen_rel {
                Some(rel) => {
                    let path = folder.join(rel);
                    if let Some(dir) = path.parent() {
                        std::fs::create_dir_all(dir)
                            .map_err(|e| Fault::Service(ServiceFault::Other(e.to_string())))?;
                    }
                    path
                }
                // No usable path: the service's own name for it, in the folder.
                None if f.path.trim().is_empty() => PathBuf::new(),
                None => return Err(Fault::Service(ServiceFault::UnsafeName(f.path.clone()))),
            };
            // Done in an earlier run: there, whole (no resume state beside it), and of the
            // service's size when the service gives one. Another file of that name (another
            // site's download, an earlier attempt whose state is gone) stays as it is: this one
            // downloads beside it, under the engine's next numbered name.
            if !dest.as_os_str().is_empty()
                && dest.is_file()
                && !mdm_engine::state::state_path(&dest).exists()
                && (f.size == 0 || std::fs::metadata(&dest).is_ok_and(|m| m.len() == f.size))
            {
                let mut sh = self.shared.lock().unwrap();
                sh.done += f.size;
                sh.saved_files.push(dest.clone());
                // One file: the download is that file, as when it comes down below.
                if !multi {
                    sh.folder = Some(dest);
                }
                continue;
            }
            let opts = mdm_engine::Options {
                unlocker: None,
                ..order.net.clone()
            };
            let state = mdm_engine::state::state_path(&dest);
            let dl = if !dest.as_os_str().is_empty() && state.exists() {
                Download::resume(state, Default::default(), opts.clone())
                    .unwrap_or_else(|_| Download::start(request(&link, &dest, &folder), opts))
            } else {
                Download::start(request(&link, &dest, &folder), opts)
            };
            self.shared.lock().unwrap().current = Some(dl);
            // Until it ends (the snapshot reads it meanwhile); then its events join the log.
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(200)) => {}
                    _ = self.cancel.cancelled() => {
                        if let Some(d) = &self.shared.lock().unwrap().current {
                            d.cancel();
                        }
                    }
                }
                let sh = self.shared.lock().unwrap();
                let ended = sh.current.as_ref().is_none_or(|d| {
                    matches!(
                        d.snapshot().state,
                        State::Done | State::Failed | State::Cancelled
                    )
                });
                if ended {
                    break;
                }
            }
            let dl = {
                let mut sh = self.shared.lock().unwrap();
                let dl = sh.current.take().expect("the file's download");
                let ev = dl.snapshot().events;
                sh.events.extend(ev.into_iter().map(|e| (e.at, e.text)));
                dl
            };
            if self.cancel.is_cancelled() {
                let _ = dl.wait().await;
                return Err(cancelled());
            }
            let out = dl.wait().await.map_err(|e| match e {
                mdm_engine::Error::Refused(mdm_engine::Refusal::Service { fault, .. }) => {
                    Fault::Service(fault)
                }
                e => Fault::Service(ServiceFault::Other(e.to_string())),
            })?;
            let mut sh = self.shared.lock().unwrap();
            sh.done += out.size;
            sh.saved_files.push(out.path.clone());
            // One file: the download is that file.
            if !multi {
                sh.folder = Some(out.path);
            }
        }
        // Everything is here: the torrent goes from the account's list.
        match s.torrent_delete(&id).await {
            Ok(()) => self.note(format!("Removed from {service}'s list")),
            Err(e) => self.note(format!(
                "Not removed from {service}'s list: {}",
                e.service()
            )),
        }
        let mut sh = self.shared.lock().unwrap();
        if multi || sh.folder.is_none() {
            sh.folder = Some(folder);
        }
        Ok(())
    }

    /// The files written for this torrent, for checking whether its folder holds untracked files.
    pub fn saved_files(&self) -> Vec<PathBuf> {
        self.shared.lock().unwrap().saved_files.clone()
    }

    /// True when the multi-file folder already existed with files before this transfer started.
    pub fn preexisting_folder(&self) -> bool {
        self.shared.lock().unwrap().preexisting_folder
    }

    /// Its id at the service, once sent (the app keeps it, to continue after a restart).
    pub fn transfer(&self) -> Option<String> {
        self.shared.lock().unwrap().transfer.clone()
    }

    /// While the service downloads the torrent (before any file comes): its name, and how far
    /// it is (0 to 1) when it says.
    pub fn at_service(&self) -> Option<(&'static str, Option<f32>)> {
        let sh = self.shared.lock().unwrap();
        let running = matches!(sh.state, Some(State::Probing));
        sh.working.filter(|_| running).map(|p| (sh.service, p))
    }

    /// Why it stopped, in the service's terms.
    pub fn failure(&self) -> Option<ServiceFault> {
        self.shared.lock().unwrap().failure.clone()
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
        if let Some(d) = &self.shared.lock().unwrap().current {
            d.cancel();
        }
    }

    pub fn set_speed_limit(&self, rate: Option<u64>) {
        if let Some(d) = &self.shared.lock().unwrap().current {
            d.set_speed_limit(rate);
        }
    }

    /// The speed limit of the file downloading now (None: none, or no file yet).
    pub fn speed_limit(&self) -> Option<u64> {
        self.shared
            .lock()
            .unwrap()
            .current
            .as_ref()
            .and_then(Download::speed_limit)
    }

    /// As one download: the service's work first (no bytes yet), then its files' bytes in all.
    pub fn snapshot(&self) -> Snapshot {
        let sh = self.shared.lock().unwrap();
        let cur = sh.current.as_ref().map(Download::snapshot);
        let downloaded = sh.done + cur.as_ref().map_or(0, |c| c.downloaded);
        let state = sh.state.unwrap_or(State::Probing);
        let state = match (state, &cur) {
            (State::Probing, Some(_)) => State::Downloading,
            (s, _) => s,
        };
        // Progress reported while a file's link is being prepared.
        let at_service = cur.as_ref().and_then(|c| c.at_service.clone());
        let (connections, server, running) = match cur {
            Some(c) => (
                c.connections,
                c.server,
                c.events.into_iter().map(|e| (e.at, e.text)).collect(),
            ),
            None => (Vec::new(), None, Vec::new()),
        };
        let events = sh
            .events
            .iter()
            .cloned()
            .chain(running)
            .enumerate()
            .map(|(i, (at, text))| mdm_engine::download::Event {
                seq: i as u64 + 1,
                at,
                text,
            })
            .collect();
        Snapshot {
            state,
            url: self.url.clone(),
            file: sh.folder.clone(),
            size: sh.total,
            downloaded,
            resumable: Some(true),
            segments: sh
                .total
                .map(|t| {
                    vec![SegmentView {
                        start: 0,
                        cursor: downloaded.min(t),
                        end: t,
                    }]
                })
                .unwrap_or_default(),
            connections,
            error: sh.error.clone(),
            // Said in the interface language by the app, as a link's refusal is.
            failure: sh.failure.clone().map(|fault| {
                mdm_engine::Error::Refused(mdm_engine::Refusal::Service {
                    service: sh.service.to_owned(),
                    fault,
                })
            }),
            events,
            server,
            at_service,
            finishing: None,
            kept: None,
        }
    }
}

/// A path's parts (`/` or `\`), empty ones dropped.
fn parts(path: &str) -> Vec<&str> {
    path.split(['/', '\\'])
        .map(str::trim)
        .filter(|p| !p.is_empty() && *p != ".")
        .collect()
}

/// Whether the service's `path` is the torrent's `wanted` one: the same parts, or the same under
/// the torrent's `name` (the services keep a torrent's files in a folder of its name; the
/// torrent's own paths are relative to it).
pub(crate) fn same_file(path: &str, name: &str, wanted: &str) -> bool {
    let (have, want) = (parts(path), parts(wanted));
    !want.is_empty()
        && (have == want
            || (have.len() == want.len() + 1 && have[0] == name.trim() && have[1..] == want[..]))
}

/// The service's files ticked in Add torrent, matched by path. Err: a ticked path the service's
/// list doesn't hold (then nothing is picked, rather than every file).
pub(crate) fn chosen(
    files: Vec<RemoteFile>,
    name: &str,
    wanted: &[String],
) -> Result<Vec<RemoteFile>, String> {
    if let Some(w) = wanted
        .iter()
        .find(|w| !files.iter().any(|f| same_file(&f.path, name, w)))
    {
        return Err(w.clone());
    }
    Ok(files
        .into_iter()
        .filter(|f| wanted.iter().any(|w| same_file(&f.path, name, w)))
        .collect())
}

fn request(link: &Url, dest: &Path, folder: &Path) -> mdm_engine::Request {
    mdm_engine::Request {
        url: link.clone(),
        headers: Default::default(),
        dest: if dest.as_os_str().is_empty() {
            Destination::Dir(folder.to_owned())
        } else {
            Destination::File(dest.to_owned())
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_from_the_service_stay_in_the_torrents_folder() {
        assert_eq!(
            safe_path("Show/S01/e1.mkv"),
            Some(PathBuf::from("Show").join("S01").join("e1.mkv"))
        );
        assert_eq!(
            safe_path("/Show/e1.mkv"),
            Some(PathBuf::from("Show").join("e1.mkv"))
        );
        assert_eq!(
            safe_path("a\\b.txt"),
            Some(PathBuf::from("a").join("b.txt"))
        );
        assert_eq!(safe_path("../evil.exe"), None);
        assert_eq!(safe_path("ok/../../evil"), None);
        assert_eq!(safe_path(""), None);
        assert_eq!(safe_path("///"), None);
        // Each part a name MDM writes: no drive, no reserved characters.
        let p = safe_path("C:/x<y>.txt").unwrap();
        assert!(
            p.components().all(|c| matches!(c, Component::Normal(_))),
            "{p:?}"
        );
    }
}
