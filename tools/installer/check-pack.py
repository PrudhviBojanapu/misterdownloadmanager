"""Validate Windows release package contents without executing the programs.

    python tools/installer/check-pack.py target/dist
"""
import json
from pathlib import Path, PurePosixPath
import re
import struct
import sys
from zipfile import ZipFile


ROOT = Path(__file__).resolve().parents[2]
PROGRAMS = (
    "misterdownloadmanager.exe", "misterdownloadmanager-nmhost.exe",
    "misterdownloadmanager-cli.exe", "misterdownloadmanager-mcp.exe",
)


def check(dist):
    version = re.search(r'(?m)^version\s*=\s*"([^"]+)"',
                        (ROOT / "Cargo.toml").read_text(encoding="utf-8")).group(1)
    prefix = f"MisterDownloadManager-{version}"
    expected = {f"{prefix}-setup.exe", f"{prefix}-portable.zip",
                f"{prefix}-extension-chromium.zip"}
    actual = {p.name for p in dist.iterdir() if p.is_file()}
    assert actual - {"SHA256SUMS.txt", "SHA256SUMS.txt.sig", "SOURCE.txt"} == expected, actual
    assert (dist / f"{prefix}-setup.exe").read_bytes()[:2] == b"MZ", "setup is not an executable"

    with ZipFile(dist / f"{prefix}-extension-chromium.zip") as extension:
        assert extension.testzip() is None, "corrupt extension archive"
        manifest = extension.read("manifest.json")
        assert json.loads(manifest)["version"] == version
        assert {n.split("/")[1] for n in extension.namelist()
                if n.startswith("_locales/")} == {"en"}, "extension is not English-only"
        for license in ("LICENSE.txt", "ATTRIBUTION.txt"):
            assert license in extension.namelist(), f"extension lacks {license}"

    with ZipFile(dist / f"{prefix}-portable.zip") as portable:
        assert portable.testzip() is None, "corrupt portable archive"
        folder = f"{prefix}-portable/"
        names = {n.removeprefix(folder) for n in portable.namelist()}
        assert all(n.startswith(folder) and "\\" not in n
                   and ".." not in PurePosixPath(n).parts for n in portable.namelist())
        assert {n for n in names if "/" not in n} == set(PROGRAMS) | {
            "LICENSE.txt", "ATTRIBUTION.txt"}, "unexpected top-level portable payload"
        for name in PROGRAMS:
            data = portable.read(folder + name)
            assert data[:2] == b"MZ", f"{name}: not a PE program"
            offset = struct.unpack_from("<I", data, 0x3C)[0]
            assert data[offset:offset + 4] == b"PE\0\0", f"{name}: invalid PE header"
            assert struct.unpack_from("<H", data, offset + 4)[0] == 0x8664, f"{name}: not x64"
        assert "data/README.txt" in names, "portable mode marker missing"
        assert portable.read(folder + "extension/chromium/manifest.json") == manifest
        assert {n.split("/")[3] for n in names
                if n.startswith("extension/chromium/_locales/")} == {"en"}
        assert not any(n.endswith(".xpi") or any(p in {"firefox", "safari", "chrome-store"}
                                              for p in PurePosixPath(n).parts) for n in names)
    print(f"Windows x64 packages OK ({version}; setup, portable, English Edge extension)")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    check(Path(sys.argv[1]))
