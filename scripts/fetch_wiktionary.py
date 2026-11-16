"""Download independently pinned English Wiktionary snapshots from Kaikki.org."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1] / "data" / "sources"
SOURCES = {
    "simple-wiktionary": {
        "url": "https://kaikki.org/dictionary/downloads/simple/simple-extract.jsonl.gz",
        "edition": "https://simple.wiktionary.org",
        "title": "Simple English Wiktionary",
    },
    "wiktionary": {
        "url": "https://kaikki.org/dictionary/English/kaikki.org-dictionary-English.jsonl.gz",
        "edition": "https://en.wiktionary.org",
        "title": "English Wiktionary",
    },
}
LICENSE_URL = "https://creativecommons.org/licenses/by-sa/4.0/legalcode.txt"


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def download(url, path):
    request = urllib.request.Request(url, headers={"User-Agent": "TinyDict-local-import"})
    temporary = None
    try:
        with urllib.request.urlopen(request, timeout=180) as response:
            modified = response.headers.get("Last-Modified")
            etag = response.headers.get("ETag")
            expected_size = response.headers.get("Content-Length")
            with tempfile.NamedTemporaryFile(dir=path.parent, suffix=".partial", delete=False) as output:
                temporary = Path(output.name)
                size = 0
                progress = 0
                while chunk := response.read(1024 * 1024):
                    output.write(chunk)
                    size += len(chunk)
                    if size - progress >= 64 * 1024 * 1024:
                        print(f"{path.parent.name}: {size / 1048576:.0f} MiB downloaded", flush=True)
                        progress = size
        if expected_size and size != int(expected_size):
            raise ValueError(f"Incomplete download: {url}")
        digest = sha256(temporary)
        # Refuse to replace an existing source file.
        os.link(temporary, path)
        return {"url": url, "sha256": digest, "bytes": size,
                "last_modified": modified, "etag": etag}
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def fetch(name):
    source = SOURCES[name]
    directory = ROOT / name
    directory.mkdir(parents=True, exist_ok=True)
    pin = directory / "source.json"
    if pin.exists():
        metadata = json.loads(pin.read_text(encoding="utf-8"))
        for filename, expected in metadata["sha256"].items():
            path = directory / filename
            if not path.exists():
                raise ValueError(f"Missing pinned file: {path}; restore it before reusing this snapshot")
            if sha256(path) != expected:
                raise ValueError(f"Checksum mismatch: {path}")
        print(f"{name}: verified {metadata['revision']}", flush=True)
        return
    data = directory / "dictionary.jsonl.gz"
    receipt = directory / "download.json"
    if data.exists():
        if not receipt.exists():
            raise ValueError(f"Unpinned input exists without a download receipt: {data}")
        evidence = json.loads(receipt.read_text(encoding="utf-8"))
        if evidence["url"] != source["url"] or sha256(data) != evidence["sha256"]:
            raise ValueError(f"Source file does not match its download receipt: {data}")
    else:
        evidence = download(source["url"], data)
        receipt.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
    legal = directory / "CC-BY-SA-4.0.txt"
    if not legal.exists():
        download(LICENSE_URL, legal)
    attribution = (
        f"{source['title']}\n\n"
        f"Copyright: {source['title']} contributors.\n"
        f"Source: {source['edition']}/\n"
        f"For each headword, the original page and contributor history are at\n"
        f"{source['edition']}/wiki/<URL-encoded headword> and its History tab.\n"
        "Extraction: Kaikki.org / Wiktextract, https://kaikki.org/\n"
        f"Snapshot: {evidence['last_modified']}\n"
        f"Download: {source['url']}\n"
        f"SHA-256: {evidence['sha256']}\n\n"
        "Wiktionary text is available under CC BY-SA 4.0 (and GFDL where applicable).\n"
        "This local adaptation uses CC BY-SA 4.0. Individual quotations retain their\n"
        "upstream references; additional terms can apply to quoted material.\n"
        "Changes: select English-language entries, retain definitions, examples and\n"
        "quotation references, omit translations/media and other unused fields,\n"
        "and build independent local lookup indexes. No cross-dictionary merging.\n\n"
        + legal.read_text(encoding="utf-8")
    )
    (directory / "LICENSE.txt").write_text(attribution, encoding="utf-8")
    metadata = {
        "source": source["edition"], "download": evidence,
        "revision": f"Kaikki {evidence['last_modified']} / {evidence['sha256'][:12]}",
        "retrieved_at": datetime.now(timezone.utc).isoformat(),
        "sha256": {filename: sha256(directory / filename) for filename in
                   ("dictionary.jsonl.gz", "download.json", "CC-BY-SA-4.0.txt", "LICENSE.txt")},
    }
    pin.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    print(f"{name}: pinned {evidence['bytes'] / 1048576:.1f} MiB -> {directory}", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sources", nargs="*", choices=list(SOURCES))
    args = parser.parse_args()
    for name in args.sources or SOURCES:
        fetch(name)
