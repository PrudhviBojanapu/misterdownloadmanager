//! Typed, non-interactive operations shared by the app, CLI and MCP.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Only retained functional options belong here. Credentials have separate write-only fields.
#[macro_export]
macro_rules! automation_settings {
    ($consumer:ident) => {
        $consumer! {
            speed_limit_on: bool, speed_limit_kb: u64, limiter_on_startup: bool,
            global_limit_on: bool, global_limit_kb: u64, global_only_active: bool,
            max_connections: u32, many_connections: bool,
            show_complete: bool, double_click_open: bool, confirm_delete: bool,
            launch_on_startup: bool, minimize_to_tray: bool,
            close_to_tray: bool, advanced_integration: bool, browsers: Vec<(String, bool)>,
            auto_types: String, excluded_sites: String, excluded_addresses: Vec<String>,
            video_panel: bool, video_panel_sites: String, capture_opened: bool, capture_min_kb: u64,
            change_folder_on_last: bool, set_creation_date: bool, mark_downloads: bool,
            show_start_dialog: bool, show_progress: bool, only_add_to_queue: bool,
            start_immediately: bool, queue_panel_later: bool, queue_panel_batch: bool,
            ignore_mtime: bool, duplicate_action: u8, user_agent: String,
            conn_exceptions: Vec<(String, u32)>,
            proxy_from_browser: bool, proxy_mode: u8, proxy_script: String,
            proxy_host: String, proxy_port: String, proxy_user: String,
            proxy_http: bool, proxy_https: bool,
            dns_mode: u8, dns_provider: String, dns_servers: String, dns_tls_name: String,
            dns_doh_url: String, dns_nextdns_id: String, dns_fallback: bool,
            net_adapter: String, debrid_on: bool, debrid_torrents: bool,
            remote_on: bool, remote_port: u16, remote_listen: String,
            torrent_keep_sharing: bool, torrent_incoming: bool, torrent_upnp: bool,
            torrent_port: u16, torrent_dht: bool, torrent_lsd: bool, torrent_proxy: bool,
            torrent_blocklist: String, torrent_stop_idle: bool,
            torrent_upload_on: bool, torrent_upload_kb: u64,
            torrent_stop_ratio_on: bool, torrent_stop_ratio: u32,
            torrent_stop_minutes_on: bool, torrent_stop_minutes: u32,
            update_check: bool, video_chapters: bool, video_cookies: String,
            video_cookies_profile: String, grabber_check: bool, grabber_hide_pages: bool
        }
    };
}

macro_rules! settings_patch {
    ($($name:ident: $ty:ty),* $(,)?) => {
        #[derive(Clone, Default, Serialize, Deserialize, JsonSchema)]
        #[serde(default, deny_unknown_fields)]
        pub struct SettingsPatch {
            $(#[serde(skip_serializing_if = "Option::is_none")] pub $name: Option<$ty>,)*
            /// Write only. Omitted on reads; an empty string clears the password.
            #[serde(skip_serializing_if = "Option::is_none")]
            pub proxy_password: Option<String>,
            /// Write only. Replaces site credentials; [] clears them.
            #[serde(skip_serializing_if = "Option::is_none")]
            pub site_logins: Option<Vec<(String, String, String)>>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub video_subtitles: Option<Subtitles>,
            /// Register/unregister torrent handlers without opening OS default-app dialogs.
            #[serde(skip_serializing_if = "Option::is_none")]
            pub torrent_associations: Option<bool>,
        }
        impl std::fmt::Debug for SettingsPatch {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("SettingsPatch(credential-redacted)")
            }
        }
    };
}
automation_settings!(settings_patch);

impl SettingsPatch {
    pub fn validate(&self) -> Result<(), String> {
        if self
            .max_connections
            .is_some_and(|n| !(1..=128).contains(&n))
            || self
                .conn_exceptions
                .as_ref()
                .is_some_and(|v| v.iter().any(|(_, n)| !(1..=128).contains(n)))
        {
            return Err("connections must be between 1 and 128".into());
        }
        if self.duplicate_action.is_some_and(|n| n > 3)
            || self.proxy_mode.is_some_and(|n| n > 2)
            || self.dns_mode.is_some_and(|n| n > 3)
        {
            return Err("invalid duplicate, proxy or DNS mode".into());
        }
        if self.remote_port == Some(0) || self.torrent_port == Some(0) {
            return Err("port must be between 1 and 65535".into());
        }
        if self
            .remote_listen
            .as_ref()
            .is_some_and(|s| !s.is_empty() && s.parse::<std::net::IpAddr>().is_err())
        {
            return Err("remote_listen must be an IP address".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DuplicatePolicy {
    /// Return the existing id, without changing or restarting it.
    #[default]
    Skip,
    /// Make a second entry with a numbered destination.
    Add,
    /// Stage an HTTP-file replacement; preserve the original until the replacement succeeds.
    Replace,
    /// Resume the existing entry if it can be started.
    Resume,
}

#[derive(Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddDownload {
    pub url: String,
    pub folder: Option<String>,
    pub file_name: Option<String>,
    #[serde(default)]
    pub later: bool,
    pub category: Option<String>,
    pub queue: Option<String>,
    pub checksum: Option<String>,
    pub referer: Option<String>,
    pub user_agent: Option<String>,
    /// A Cookie header for the download's requests (`name=value; other=value`).
    #[serde(default, alias = "cookies")]
    pub cookie: Option<String>,
    /// Keep it out of the saved list (as a private browser window's download).
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub duplicate: DuplicatePolicy,
    /// Never open Download complete for this download; other downloads follow Options.
    #[serde(default)]
    pub no_complete_dialog: bool,
}
impl std::fmt::Debug for AddDownload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AddDownload")
            .field("url", &self.url)
            .field("folder", &self.folder)
            .field("file_name", &self.file_name)
            .field("later", &self.later)
            .field("cookie", &self.cookie.as_ref().map(|_| "<redacted>"))
            .field("private", &self.private)
            .field("duplicate", &self.duplicate)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PropertyPatch {
    pub description: Option<String>,
    /// Empty clears it.
    pub referer: Option<String>,
    /// Empty username clears the login. Never returned on reads.
    pub login: Option<(String, String)>,
    pub address: Option<String>,
    pub direct: Option<bool>,
    pub category: Option<String>,
    /// Remembered bytes/second limit; zero clears it.
    pub speed_limit: Option<u64>,
    /// Hex MD5/SHA-1/SHA-256/SHA-512/CRC32; empty clears. Applied to the next start.
    pub checksum: Option<String>,
}
impl std::fmt::Debug for PropertyPatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PropertyPatch(credential-redacted)")
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Video,
    Gallery,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Helper {
    Ytdlp,
    Ffmpeg,
    GalleryDl,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Subtitles {
    pub langs: Vec<String>,
    pub automatic: bool,
    pub embed: bool,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Add {
        download: AddDownload,
    },
    Properties {
        id: u64,
    },
    SetProperties {
        id: u64,
        patch: PropertyPatch,
    },
    Retry {
        ids: Vec<u64>,
    },
    Move {
        ids: Vec<u64>,
        folder: String,
    },
    Rename {
        id: u64,
        name: String,
    },
    Settings {},
    SetSettings {
        patch: Box<SettingsPatch>,
    },
    Categories {},
    /// None creates a category; an existing key updates it. Empty folder restores its default.
    SetCategory {
        key: Option<String>,
        name: String,
        types: String,
        folder: Option<String>,
    },
    DeleteCategory {
        key: String,
    },
    Queues {},
    /// None creates a basic queue; an existing name updates it.
    SetQueue {
        queue: Option<String>,
        name: String,
        files_at_once: u32,
    },
    DeleteQueue {
        queue: String,
    },
    /// Replace the membership/order atomically; downloads must already exist.
    QueueItems {
        queue: String,
        ids: Vec<u64>,
    },
    StartQueue {
        queue: String,
    },
    StopQueue {
        queue: String,
    },
    /// Write-only TorBox API key. None creates an account; empty key removes the account.
    Torbox {
        id: Option<u64>,
        api_key: String,
        enabled: bool,
    },
    Accounts {},
    RemoteStatus {},
    Pair {},
    StopPairing {},
    RevokeDevice {
        id: String,
    },
    /// Async video quality discovery; poll with video_poll.
    VideoLookup {
        url: String,
    },
    VideoPoll {
        token: String,
    },
    /// Add one quality from video_lookup's menu without requiring its internal selector.
    VideoPick {
        token: String,
        choice: usize,
        download: AddDownload,
    },
    /// Starts a GUI-list video/gallery download; selector is yt-dlp syntax.
    MediaAdd {
        kind: MediaKind,
        download: AddDownload,
        #[serde(default)]
        selector: String,
        #[serde(default)]
        subtitles: Option<Subtitles>,
        limit: Option<u64>,
    },
    /// Async bounded listing; poll job, then add selected entries with playlist_add.
    PlaylistLookup {
        url: String,
        #[serde(default = "first")]
        from: u64,
        count: Option<u64>,
    },
    TorrentLookup {
        source: String,
    },
    /// Inspect a lookup's progress, files/entries or error. Jobs expire after ten minutes.
    Job {
        job: u64,
    },
    PlaylistAdd {
        job: u64,
        indices: Option<Vec<u64>>,
        folder: String,
        selector: String,
        #[serde(default)]
        later: bool,
    },
    TorrentAdd {
        job: u64,
        folder: String,
        files: Option<Vec<usize>>,
        #[serde(default)]
        later: bool,
        queue: Option<String>,
        #[serde(default)]
        use_existing: bool,
        #[serde(default)]
        in_order: bool,
    },
    TorrentRecheck {
        id: u64,
    },
    TorrentReannounce {
        id: u64,
    },
    TorrentTrackers {
        id: u64,
        trackers: Vec<String>,
    },
    TorrentOrder {
        id: u64,
        in_order: bool,
    },
    TorrentFiles {
        id: u64,
    },
    TorrentChooseFiles {
        id: u64,
        files: Vec<usize>,
    },
    Helpers {},
    InstallHelper {
        helper: Helper,
    },
    /// Validate and select a custom executable; null restores MDM's default helper.
    ChooseHelper {
        helper: Helper,
        path: Option<String>,
    },
    /// Hash a completed local file in the background. Omitted expected uses its saved checksum.
    VerifyChecksum {
        id: u64,
        expected: Option<String>,
    },
    /// Create a torrent asynchronously, without overwriting an existing output.
    TorrentCreate {
        path: String,
        output: String,
    },
}
fn first() -> u64 {
    1
}
impl std::fmt::Debug for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Operation")
            .field(&std::mem::discriminant(self))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn operations_request_results_and_debug_never_leaks_credentials() {
        let command = crate::Command {
            operation: Some(Operation::SetSettings {
                patch: Box::new(SettingsPatch {
                    proxy_password: Some("private-proxy-password".into()),
                    ..Default::default()
                }),
            }),
            ..Default::default()
        };
        assert!(command.wants_reply());
        assert!(!format!("{command:?}").contains("private-proxy-password"));
        let account = Operation::Torbox {
            id: None,
            api_key: "private-token".into(),
            enabled: true,
        };
        assert!(!format!("{account:?}").contains("private-token"));
    }
    #[test]
    fn rejects_removed_options_and_invalid_patches() {
        assert!(serde_json::from_str::<SettingsPatch>(r#"{"clipboard_autostart":true}"#).is_err());
        assert!(
            serde_json::from_str::<Operation>(r#"{"op":"settings","password":"oops"}"#).is_err()
        );
        let patch = SettingsPatch {
            max_connections: Some(0),
            ..Default::default()
        };
        assert!(patch.validate().is_err());
        for op in [
            "settings",
            "categories",
            "queues",
            "accounts",
            "remote_status",
            "pair",
            "stop_pairing",
            "helpers",
        ] {
            assert!(
                serde_json::from_value::<Operation>(
                    serde_json::json!({"op": op, "password": "oops"})
                )
                .is_err(),
                "{op}"
            );
        }
    }
    #[test]
    fn safe_duplicate_default_and_round_trip() {
        let add: Operation =
            serde_json::from_str(r#"{"op":"add","download":{"url":"https://example.com/a"}}"#)
                .unwrap();
        let Operation::Add { download } = &add else {
            panic!()
        };
        assert!(matches!(download.duplicate, DuplicatePolicy::Skip));
        assert!(serde_json::from_str::<Operation>(&serde_json::to_string(&add).unwrap()).is_ok());
    }
}
