"""Download pinned upstream data and licenses without extracting arbitrary archive paths."""
import hashlib
import io
import json
from pathlib import Path
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1] / "data" / "sources"


def download(url):
    request = urllib.request.Request(url, headers={"User-Agent": "TinyDict-local-import"})
    with urllib.request.urlopen(request, timeout=180) as response:
        return response.read()


def fetch(name, repo, branch):
    destination = ROOT / name
    destination.mkdir(parents=True, exist_ok=True)
    pin = destination / "source.json"
    revision = (json.loads(pin.read_text(encoding="utf-8"))["revision"] if pin.exists()
                else json.loads(download(f"https://api.github.com/repos/{repo}/commits/{branch}"))["sha"])
    base = f"https://raw.githubusercontent.com/{repo}/{revision}"
    files = {}
    if name == "cloudbytes":
        for filename in ["dictionary.jsonl.gz", "LICENSE-DATA.md", "SHA256SUMS"]:
            path = destination / filename
            if not path.exists():
                path.write_bytes(download(f"{base}/v2/{filename}"))
            files[filename] = hashlib.sha256(path.read_bytes()).hexdigest()
        sums = (destination / "SHA256SUMS").read_text().splitlines()
        expected = next(line.split()[0] for line in sums if line.split()[-1].endswith("dictionary.jsonl.gz"))
        if files["dictionary.jsonl.gz"] != expected:
            raise ValueError("CloudBytes checksum mismatch; remove the downloaded file and retry")
    else:
        data_dir = destination / "data"
        data_dir.mkdir(exist_ok=True)
        if not pin.exists():
            archive = zipfile.ZipFile(io.BytesIO(download(f"https://codeload.github.com/{repo}/zip/{revision}")))
            for item in archive.infolist():
                parts = item.filename.split("/")
                if len(parts) == 3 and parts[1] == "data" and parts[2].endswith(".json"):
                    (data_dir / parts[2]).write_bytes(archive.read(item))
            for filename in ["LICENSE", "Readme.md"]:
                (destination / filename).write_bytes(download(f"{base}/{filename}"))
        for path in sorted(data_dir.glob("*.json")):
            files[f"data/{path.name}"] = hashlib.sha256(path.read_bytes()).hexdigest()
        if not files:
            raise ValueError("Wordset archive contained no data files")
        for filename in ["LICENSE", "Readme.md"]:
            files[filename] = hashlib.sha256((destination / filename).read_bytes()).hexdigest()
    if pin.exists() and files != json.loads(pin.read_text(encoding="utf-8"))["sha256"]:
        raise ValueError(f"{name}: local inputs differ from pinned snapshot")
    pin.write_text(json.dumps({"repository": repo, "revision": revision, "sha256": files}, indent=2) + "\n", encoding="utf-8")
    print(f"{name}: {revision}, {len(files)} files -> {destination}", flush=True)


if __name__ == "__main__":
    fetch("cloudbytes", "CloudBytes-Academy/English-Dictionary-Open-Source", "main")
    fetch("wordset", "wordset/wordset-dictionary", "master")
