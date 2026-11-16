//! Connection information measured by the download itself. No location database or lookup.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

/// Kept compatible with connection information already saved in the download list.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seen {
    #[serde(default)]
    pub ip: Option<IpAddr>,
    #[serde(default)]
    pub rtt_ms: Option<u32>,
    #[serde(default)]
    pub proxied: bool,
    #[serde(default)]
    pub cdn: Option<String>,
}

impl From<&mdm_engine::Server> for Seen {
    fn from(s: &mdm_engine::Server) -> Self {
        Seen {
            ip: s.ip,
            rtt_ms: s.rtt.map(|d| d.as_millis().min(u32::MAX as u128) as u32),
            proxied: s.proxied,
            cdn: s.cdn.map(str::to_owned),
        }
    }
}

pub fn line(ui: &egui::Ui, x: f32, baseline: f32, right: f32, seen: Option<&Seen>, id: &str) {
    let words = seen.map_or_else(
        || "...".to_owned(),
        |s| {
            let mut words =
                s.ip.map_or_else(|| crate::i18n::t("unknown").to_owned(), |ip| ip.to_string());
            if let Some(ms) = s.rtt_ms {
                words.push_str(&format!(" · {} ms", ms.max(1)));
            }
            words
        },
    );
    crate::winui::fit_middle(
        ui,
        ui.painter(),
        x,
        baseline,
        &words,
        crate::winui::TEXT(),
        right,
        ("server-line", id),
    );
}
