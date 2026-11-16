//! Filename-only archive grouping and safe rename guards; no archive extraction.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeKind {
    RarPart,
    RarOld,
    Split,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    pub base: String,
    pub kind: VolumeKind,
    pub index: u32,
}

impl Volume {
    pub fn is_first(&self) -> bool {
        self.index == 1
    }
}

pub fn volume_key(name: &str) -> Option<Volume> {
    let lower = name.to_lowercase();
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    // Measure the ASCII suffix from the end: Unicode lowercasing can change the stem's length.
    let base = |at| {
        name.get(..name.len() - (lower.len() - at))
            .unwrap_or(name)
            .to_owned()
    };
    if let Some(stem) = lower.strip_suffix(".rar")
        && let Some(i) = stem.rfind(".part")
        && digits(&stem[i + 5..])
    {
        return Some(Volume {
            base: base(i),
            kind: VolumeKind::RarPart,
            index: stem[i + 5..].parse().ok()?,
        });
    }
    if lower.ends_with(".rar") {
        return Some(Volume {
            base: base(lower.len() - 4),
            kind: VolumeKind::RarOld,
            index: 1,
        });
    }
    if let Some(i) = lower.rfind(".r")
        && lower.len() - i == 4
        && digits(&lower[i + 2..])
    {
        return Some(Volume {
            base: base(i),
            kind: VolumeKind::RarOld,
            index: lower[i + 2..].parse::<u32>().ok()? + 2,
        });
    }
    for ext in [".7z.", ".zip."] {
        if let Some(i) = lower.rfind(ext) {
            let num = &lower[i + ext.len()..];
            if num.len() == 3 && digits(num) {
                return Some(Volume {
                    base: base(i),
                    kind: VolumeKind::Split,
                    index: num.parse().ok()?,
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouping_preserves_unicode_stems_and_volume_suffixes() {
        for (name, stem, index) in [
            ("İstanbul.part2.rar", "İstanbul", 2),
            ("Archive.7z.001", "Archive", 1),
            ("movie.r00", "movie", 2),
        ] {
            let volume = volume_key(name).unwrap();
            assert_eq!(volume.base, stem);
            assert_eq!(volume.index, index);
        }
        assert!(volume_key("photo.jpg").is_none());
    }
}
