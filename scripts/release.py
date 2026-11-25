"""Steps of .github/workflows/release.yml.

    python scripts/release.py plan           what has to happen for this commit
    python scripts/release.py bump patch     next version in Cargo.toml, Cargo.lock and CHANGELOG.md
    python scripts/release.py version        the version in Cargo.toml
    python scripts/release.py notes          changelog of the current version
    python scripts/release.py published      exit code 0 if the current version is on crates.io

`plan` prints one of:

    none      nothing to do
    bump      the crate changed since the last release
    release   the version in Cargo.toml is not on crates.io or has no tag yet
"""

import datetime
import json
import pathlib
import re
import subprocess
import sys
import urllib.error
import urllib.request

CRATE = "rust-glass"
REPO = "https://github.com/MasterKPG/rust-glass"
# Everything that ends up in the package.
PACKAGED = ["crates/rust-glass", "README.md", "Cargo.toml"]
ROOT = pathlib.Path(__file__).resolve().parent.parent


def read(name):
    return (ROOT / name).read_text(encoding="utf-8")


def write(name, text):
    (ROOT / name).write_text(text, encoding="utf-8", newline="\n")


def git(*args):
    r = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True, encoding="utf-8", check=True)
    return r.stdout.strip()


def current():
    return re.search(r'^version = "(\d+\.\d+\.\d+)"$', read("Cargo.toml"), re.M).group(1)


def published():
    """Versions on crates.io. Empty if the crate does not exist."""
    url = f"https://index.crates.io/{CRATE[:2]}/{CRATE[2:4]}/{CRATE}"
    try:
        with urllib.request.urlopen(url, timeout=30) as r:
            return {json.loads(line)["vers"] for line in r.read().decode().splitlines()}
    except urllib.error.HTTPError as e:
        if e.code in (404, 410, 451):
            return set()
        raise


def plan():
    versions = published()
    if not versions:
        # The first version is published by hand. Trusted Publishing needs it.
        return "none"
    v = current()
    if v not in versions or not git("tag", "--list", f"v{v}"):
        return "release"
    changed = git("diff", "--name-only", f"v{v}", "HEAD", "--", *PACKAGED)
    return "bump" if changed else "none"


def next_version(old, kind):
    major, minor, patch = map(int, old.split("."))
    return f"{major}.{minor + 1}.0" if kind == "minor" else f"{major}.{minor}.{patch + 1}"


def section(text, name):
    """Body of a `## [name]` section of the changelog."""
    m = re.search(rf"^## \[{re.escape(name)}\][^\n]*\n(.*?)(?=^## \[|^\[[^\]]+\]: |\Z)", text, re.M | re.S)
    return m.group(1).strip() if m else ""


def bump_changelog(text, old, new, today, subjects):
    body = section(text, "Unreleased")
    if not body:
        body = "### Changed\n" + "\n".join(f"- {s}" for s in subjects)
    head = text[: text.index("## [Unreleased]")]
    rest = text[text.index(f"## [{old}]"):]
    rest = re.sub(r"^\[Unreleased\]: .*\n", "", rest, flags=re.M)
    rest = rest.replace(
        f"\n[{old}]: ",
        f"\n[Unreleased]: {REPO}/compare/v{new}...HEAD\n[{new}]: {REPO}/compare/v{old}...v{new}\n[{old}]: ",
        1,
    )
    return f"{head}## [Unreleased]\n\n## [{new}] - {today}\n\n{body}\n\n{rest}"


def bump(kind):
    old = current()
    new = next_version(old, kind)
    subjects = git("log", "--format=%s", f"v{old}..HEAD").splitlines()
    today = datetime.date.today().isoformat()
    write("Cargo.toml", read("Cargo.toml").replace(f'version = "{old}"', f'version = "{new}"', 1))
    lock = read("Cargo.lock")
    for name in (CRATE, "showcase"):
        lock = lock.replace(f'name = "{name}"\nversion = "{old}"', f'name = "{name}"\nversion = "{new}"', 1)
    write("Cargo.lock", lock)
    write("CHANGELOG.md", bump_changelog(read("CHANGELOG.md"), old, new, today, subjects))
    return new


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    if cmd == "plan":
        print(plan())
    elif cmd == "version":
        print(current())
    elif cmd == "bump" and sys.argv[2:] in (["patch"], ["minor"]):
        print(bump(sys.argv[2]))
    elif cmd == "notes":
        print(section(read("CHANGELOG.md"), current()) or f"See [CHANGELOG.md]({REPO}/blob/main/CHANGELOG.md).")
    elif cmd == "published":
        sys.exit(0 if current() in published() else 1)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main()
