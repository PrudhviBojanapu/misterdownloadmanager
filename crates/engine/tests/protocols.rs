//! Removed protocols fail before any connection or modification of download files.

use mdm_engine::state::{ResumeState, Validators, state_path};
use mdm_engine::{Destination, Download, Error, Options, Request, probe, supported};
use reqwest::header::HeaderMap;
use url::Url;

fn unsupported(error: Error, scheme: &str) {
    assert!(
        matches!(&error, Error::UnsupportedScheme(s) if s == scheme),
        "{error}"
    );
    assert!(error.to_string().contains("only HTTP and HTTPS"));
}

#[tokio::test]
async fn ftp_variants_are_rejected_without_creating_files() {
    let dir = tempfile::tempdir().unwrap();
    for scheme in ["ftp", "ftps", "ftpes"] {
        let url: Url = format!("{scheme}://user:secret@127.0.0.1:9/file.bin")
            .parse()
            .unwrap();
        assert!(!supported(&url));
        unsupported(
            probe(url.clone(), HeaderMap::new(), &Options::default())
                .await
                .unwrap_err(),
            scheme,
        );
        let error = Download::start(
            Request {
                url,
                headers: HeaderMap::new(),
                dest: Destination::Dir(dir.path().to_owned()),
            },
            Options::default(),
        )
        .wait()
        .await
        .unwrap_err();
        assert!(!error.to_string().contains("secret"));
        unsupported(error, scheme);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}

#[tokio::test]
async fn old_ftp_resume_state_is_preserved_and_explicitly_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("old.bin");
    let part = mdm_engine::naming::part_path(&dest);
    std::fs::write(&part, b"saved bytes").unwrap();
    for (original, target) in [
        ("ftp://127.0.0.1:9/old.bin", "ftp://127.0.0.1:9/old.bin"),
        (
            "https://example.invalid/old.bin",
            "ftp://127.0.0.1:9/old.bin",
        ),
    ] {
        let saved = ResumeState::new(
            original.parse().unwrap(),
            target.parse().unwrap(),
            dest.clone(),
            11,
            Validators::default(),
        );
        let path = state_path(&saved.dest);
        saved.write(&path).unwrap();
        let before = std::fs::read(&path).unwrap();
        let error = Download::resume(path.clone(), HeaderMap::new(), Options::default())
            .unwrap()
            .wait()
            .await
            .unwrap_err();
        unsupported(error, "ftp");
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::read(&part).unwrap(), b"saved bytes");
    }
}
