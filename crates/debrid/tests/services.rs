//! TorBox account, device sign-in, downloads, and legacy-account rejection against the lab.

use std::sync::Arc;
use std::time::Duration;

use mdm_debrid::{Account, Credential, Debrid, Http, ServiceKind, Services, check, signin};
use mdm_engine::{Destination, Download, Error, Options, Refusal, Request, ServiceFault};
use mdm_labserver::{Config, LabServer, content, debrid::seed};
use url::Url;

async fn lab() -> (LabServer, Arc<Http>) {
    let lab = LabServer::start(Config::default()).await.unwrap();
    let base = Url::parse(&lab.url("/")).unwrap();
    let http = Arc::new(Http::new(&Options::default(), Services::at(&base)).unwrap());
    (lab, http)
}

fn token(value: &str) -> Credential {
    Credential::Token {
        token: value.into(),
    }
}

async fn account(http: &Arc<Http>) -> Debrid {
    let d = Debrid::default();
    d.set_accounts(
        vec![Account {
            id: 3,
            service: ServiceKind::TorBox,
            credential: token("good"),
        }],
        http.clone(),
    );
    d.refresh_hosts().await.unwrap();
    d
}

#[tokio::test]
async fn torbox_device_auth_and_account_check() {
    let (_lab, http) = lab().await;
    let info = check(&http, ServiceKind::TorBox, token("good"))
        .await
        .unwrap();
    assert_eq!(
        (info.user.as_str(), info.premium),
        ("lab@example.invalid", Some(true))
    );
    assert!(info.premium_left.is_some());
    assert_eq!(
        check(&http, ServiceKind::TorBox, token("free"))
            .await
            .unwrap()
            .premium,
        Some(false)
    );
    assert!(matches!(
        check(&http, ServiceKind::TorBox, token("bad")).await,
        Err(mdm_debrid::Fault::Service(ServiceFault::SignIn))
    ));
    let code = signin::start(&http, ServiceKind::TorBox).await.unwrap();
    assert_eq!(code.shown, "123456");
    assert!(code.page.contains("torbox.app"));
    let mut credential = None;
    for _ in 0..10 {
        credential = signin::poll(&http, &code).await.unwrap();
        if credential.is_some() {
            break;
        }
    }
    assert_eq!(
        check(
            &http,
            ServiceKind::TorBox,
            credential.expect("device approved")
        )
        .await
        .unwrap()
        .premium,
        Some(true)
    );
}

#[tokio::test]
async fn torbox_downloads_and_releases_the_file_without_leaking_credentials() {
    let (lab, http) = lab().await;
    let d = account(&http).await;
    assert!(
        d.account_for(&"http://hoster2.test/x".parse().unwrap())
            .is_some()
    );
    assert!(
        d.account_for(&"http://elsewhere.test/x".parse().unwrap())
            .is_none()
    );
    let url: Url = "http://hoster.test/file/2M/b.bin".parse().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let dl = Download::start(
        Request {
            url: url.clone(),
            headers: Default::default(),
            dest: Destination::Dir(dir.path().to_owned()),
        },
        Options {
            unlocker: d.unlocker_for(&url),
            ..Options::default()
        },
    );
    let mut seen = None;
    for _ in 0..400 {
        let snap = dl.snapshot();
        seen = seen.or(snap.at_service);
        if matches!(
            snap.state,
            mdm_engine::download::State::Done | mdm_engine::download::State::Failed
        ) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(seen, Some(("TorBox".to_owned(), Some(0.5))));
    assert_eq!(dl.snapshot().at_service, None);
    let out = dl.wait().await.unwrap();
    let bytes = std::fs::read(out.path).unwrap();
    let mut want = vec![0; bytes.len()];
    content::fill(seed("hoster.test/file/2M/b.bin"), 0, &mut want);
    assert_eq!(bytes, want);
    let held = || async {
        let body = reqwest::get(lab.url("/_debrid/torrents"))
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        serde_json::from_slice::<Vec<String>>(&body).unwrap()
    };
    let before = held().await;
    d.release(&url).await.unwrap();
    let after = held().await;
    assert!(after.len() < before.len());
    d.release(&url).await.unwrap();
    assert_eq!(held().await, after);
    assert!(
        lab.log()
            .iter()
            .filter(|e| e.path.starts_with("/gen/"))
            .all(|e| e.authorization.is_none()
                && !e.query.as_deref().unwrap_or("").contains("token"))
    );
}

#[tokio::test]
async fn torbox_refusals_keep_the_service_reason() {
    let (_lab, http) = lab().await;
    let d = account(&http).await;
    let dir = tempfile::tempdir().unwrap();
    for (query, expected) in [
        ("gone=1", ServiceFault::FileUnavailable),
        ("down=1", ServiceFault::HostDown),
    ] {
        let url: Url = format!("http://hoster.test/file/1M/error.bin?{query}")
            .parse()
            .unwrap();
        let err = Download::start(
            Request {
                url: url.clone(),
                headers: Default::default(),
                dest: Destination::Dir(dir.path().to_owned()),
            },
            Options {
                unlocker: d.unlocker_for(&url),
                max_retries: 0,
                ..Options::default()
            },
        )
        .wait()
        .await
        .unwrap_err();
        assert!(
            matches!(err, Error::Refused(Refusal::Service { service, fault }) if service == "TorBox" && fault == expected)
        );
    }
}

#[tokio::test]
async fn legacy_services_remain_readable_but_never_contact_a_provider() {
    let (lab, http) = lab().await;
    for name in ["RealDebrid", "AllDebrid", "Premiumize", "MegaDebrid"] {
        let service: ServiceKind = serde_json::from_str(&format!("\"{name}\"")).unwrap();
        assert_ne!(service, ServiceKind::TorBox);
        assert!(!service.supported());
        assert!(!service.takes_magnets());
        let err = check(&http, service, token("legacy-secret"))
            .await
            .unwrap_err();
        assert!(
            matches!(err, mdm_debrid::Fault::Service(ServiceFault::Other(message)) if message.contains("no longer supported"))
        );
        assert!(signin::start(&http, service).await.is_err());
        let d = Debrid::default();
        d.set_accounts(
            vec![Account {
                id: 3,
                service,
                credential: token("legacy-secret"),
            }],
            http.clone(),
        );
        d.refresh_hosts().await.unwrap();
        assert!(d.torrent_account().is_none());
        assert!(
            d.account_for(&"http://hoster.test/file".parse().unwrap())
                .is_none()
        );
        assert!(matches!(
            d.account_info(3).await,
            Err(mdm_debrid::Fault::Service(ServiceFault::Other(message)))
                if message.contains("no longer supported")
        ));
        let dir = tempfile::tempdir().unwrap();
        let torrent = mdm_debrid::torrents::RemoteTorrent::start(
            &d,
            mdm_debrid::torrents::Order {
                account: 3,
                magnet: "magnet:?xt=urn:btih:1234abcd".parse().unwrap(),
                transfer: Some("old-transfer".into()),
                folder: dir.path().to_owned(),
                renamed: None,
                files: None,
                net: Options::default(),
                poll: Duration::from_millis(1),
            },
        );
        for _ in 0..100 {
            if torrent.snapshot().state == mdm_engine::download::State::Failed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            torrent
                .snapshot()
                .error
                .unwrap_or_default()
                .contains("no longer supported")
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
    assert!(
        lab.log().is_empty(),
        "legacy accounts must not send requests"
    );
}

#[tokio::test]
async fn legacy_credentials_cannot_be_used_as_torbox_keys() {
    let (lab, http) = lab().await;
    for credential in [
        Credential::Oauth {
            client_id: "id".into(),
            client_secret: "secret".into(),
            refresh_token: "refresh".into(),
        },
        Credential::Login {
            user: "user".into(),
            password: "password".into(),
        },
    ] {
        assert_eq!(
            check(&http, ServiceKind::TorBox, credential).await,
            Err(mdm_debrid::Fault::Service(ServiceFault::SignIn))
        );
    }
    assert!(lab.log().is_empty());
}
