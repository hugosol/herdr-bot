#!/usr/bin/env python3
"""Check the shipped pstack files without building or starting an agent."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import ntpath
import os
from pathlib import Path, PurePosixPath
import platform
import posixpath
import re
import stat
import subprocess
import sys
from urllib.parse import unquote, urlsplit


HERE = Path(__file__).resolve().parent
LINK = re.compile(r"(?<!!)\[[^\]\n]*\]\(([^)\s]+)\)")
IMPORT = re.compile(r"\b(?:from\s+|import\s*\(\s*|import\s+)[\"'](\.{1,2}/[^\"']+)[\"']")
SKILL = re.compile(r"(?:/skill:|skill://)([a-z][a-z0-9-]*)")
LOCAL_PRODUCTS = {"roles.local.json"}


def stamp():
    return datetime.now(timezone.utc).isoformat()


def fingerprint(data):
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def path_parts(name):
    parts = name.split("/")
    if not name or "\\" in name or ntpath.splitdrive(name)[0] or any(part in ("", ".", "..") for part in parts):
        raise ValueError(f"not a portable package path: {name!r}")
    return parts


def is_link_or_junction(mode):
    return stat.S_ISLNK(mode.st_mode) or bool(getattr(mode, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT)


def read_file(root, name):
    parts = path_parts(name)
    path = root
    for part in parts:
        # Match the spelling even on a case-insensitive filesystem.
        with os.scandir(path) as children:
            entry = next((item for item in children if item.name == part), None)
        if entry is None:
            raise FileNotFoundError(name)
        mode = entry.stat(follow_symlinks=False)
        path = path / part
        if is_link_or_junction(mode):
            raise ValueError(f"linked package path: {path}")
    if not stat.S_ISREG(mode.st_mode):
        raise ValueError(f"not a regular file: {path}")
    return path.read_bytes()


def inventory(directory):
    mode = directory.lstat()
    if not stat.S_ISDIR(mode.st_mode) or is_link_or_junction(mode):
        raise ValueError(f"not a real package directory: {directory}")

    def fail(error):
        raise error

    for root, dirs, files in os.walk(directory, followlinks=False, onerror=fail):
        for name in dirs[:]:
            child = Path(root) / name
            if is_link_or_junction(child.lstat()):
                dirs.remove(name)
                yield child.relative_to(directory).as_posix()
        for name in files:
            yield (Path(root) / name).relative_to(directory).as_posix()


def no_duplicate_keys(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def decode_json(data):
    return json.loads(data.decode("utf-8"), object_pairs_hook=no_duplicate_keys)


def root_identity(path):
    result = {"path": str(path)}
    # git is only used to identify the source; actual bytes are read below.
    try:
        env = dict(os.environ, GIT_OPTIONAL_LOCKS="0")
        for key, args in {"head": ["rev-parse", "HEAD"], "status": ["status", "--porcelain=v1"]}.items():
            command = ["git", "-C", str(path), *args]
            proc = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", errors="replace", env=env)
            result[key] = {"command": command, "exit": proc.returncode, "stdout": proc.stdout, "stderr": proc.stderr}
    except OSError as error:
        result["git_error"] = str(error)
    return result


class PackageCheck:
    def __init__(self, package, report):
        self.package = package
        self.report = report
        self.errors = report["errors"]
        self.files = {}

    def read(self, name):
        if name not in self.files:
            self.files[name] = read_file(self.package, name)
            self.report["files"][name] = fingerprint(self.files[name])
        return self.files[name]

    def json(self, name):
        return decode_json(self.read(name))

    def inventory(self, area, declared):
        try:
            actual = set(inventory(self.package / area))
        except OSError as error:
            self.errors.append(f"cannot list {area}: {error}")
            return
        for missing in sorted(declared - actual):
            self.errors.append(f"missing {area} asset: {missing}")
        for extra in sorted(actual - declared):
            if area == "generated" and (extra in LOCAL_PRODUCTS or re.fullmatch(r"agents/[a-z0-9-]+\.md", extra)):
                self.report["local_products"].append(extra)
            else:
                self.errors.append(f"undeclared {area} asset: {extra}")

    def check_assets(self, closure):
        manifest = self.json("manifest.json")
        ownership = self.json("generated-ownership.json")
        if ownership["version"] != 1:
            raise ValueError("unsupported generated ownership version")
        owners = ownership["files"]
        assets = self.report["assets"]
        declared = {area: set() for area in ("generated", "upstream", "runtime", "mappings")}
        asset_rows = manifest["files"]
        for row in asset_rows:
            output = row["output"]
            origin = row.get("origin", "upstream")
            if origin not in ("upstream", "runtime"):
                raise ValueError(f"unknown source origin: {origin}")
            path_parts(output)
            path_parts(row["source"])
            if output in declared["generated"]:
                self.errors.append(f"duplicate generated declaration: {output}")
            declared["generated"].add(output)
            declared[origin].add(row["source"])
            if "mapping" in row:
                path_parts(row["mapping"])
                declared["mappings"].add(row["mapping"])
        for area, paths in declared.items():
            self.inventory(area, paths)
        if set(owners) != declared["generated"]:
            self.errors.append("ownership inventory differs from manifest outputs")
        for row in asset_rows:
            output, source = row["output"], row["source"]
            origin = row.get("origin", "upstream")
            source_path = f"{origin}/{source}"
            try:
                delivered = self.read("generated/" + output)
                original = self.read(source_path)
                asset = {**fingerprint(delivered), "source": source_path, "kind": row["kind"]}
                assets[output] = asset
                if owners.get(output) != asset["sha256"]:
                    self.errors.append(f"generated ownership hash differs: {output}")
                kind = row["kind"]
                if kind == "text":
                    delivered.decode("utf-8")
                    original = original.decode("utf-8").replace("\r\n", "\n").encode("utf-8")
                elif kind != "binary":
                    raise ValueError(f"unknown asset kind {kind!r}")
                if "mapping" in row:
                    if kind != "text" or origin != "upstream":
                        raise ValueError("only upstream text can have a mapping")
                    mapping = "mappings/" + row["mapping"]
                    operations = self.json(mapping)
                    names = set()
                    asset["mapping"] = mapping
                    asset["operations"] = []
                    for action in operations:
                        name, reason = action["name"], action["reason"]
                        if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", name) or name in names or not reason.strip():
                            raise ValueError(f"invalid mapping name or reason: {name!r}")
                        names.add(name)
                        asset["operations"].append({"name": name, "reason": reason})
                    if not names:
                        raise ValueError("mapping has no named operations")
                elif delivered != original:
                    self.errors.append(f"unmapped asset differs from source: {output}")
                if origin == "upstream":
                    identity = source.split("/", 1)[0]
                    asset["license"] = closure["sources"][identity]["license_output"]
            except (OSError, ValueError, KeyError, TypeError) as error:
                self.errors.append(f"cannot check asset {output}: {error}")
        self.report["sources"] = closure["sources"]
        for identity, source in closure["sources"].items():
            prefix = f"{identity}/"
            expected_sources = sorted(path for path in declared["upstream"] if path.startswith(prefix))
            try:
                hashes = "".join(f"{path}\t{fingerprint(self.read('upstream/' + path))['sha256']}\n" for path in expected_sources)
                if fingerprint(hashes.encode("utf-8"))["sha256"] != source["selected_tree_sha256"]:
                    self.errors.append(f"selected snapshot differs from reviewed upstream: {identity}")
            except (OSError, ValueError) as error:
                self.errors.append(f"cannot fingerprint upstream snapshot {identity}: {error}")
            if source["repository"] != manifest["upstream"]["repository"] or source["revision"] != manifest["upstream"]["revision"]:
                self.errors.append(f"reviewed upstream revision differs from manifest: {identity}")
            if identity == "pstack" and manifest["upstream"]["version"] != f"pstack {source['version']}":
                self.errors.append("reviewed pstack version differs from manifest")
            output = source["license_output"]
            license_asset = assets.get(output, {})
            if license_asset.get("source") != f"upstream/{identity}/LICENSE":
                self.errors.append(f"license not delivered from its declared upstream: {identity}")
            if license_asset.get("sha256") != source["license_sha256"]:
                self.errors.append(f"delivered license differs from reviewed upstream: {identity}")
        if set(closure["external_skills"]) != {"deslop", "control-cli", "control-ui"}:
            self.errors.append("closure must declare the three approved Cursor Team Kit skills")
        expected = {f"cursor-team-kit/skills/{name}/SKILL.md" for name in closure["external_skills"]}
        actual = {path for path in declared["upstream"] if path.startswith("cursor-team-kit/skills/")}
        if expected != actual:
            self.errors.append(f"Cursor Team Kit skill selection differs: {sorted(actual ^ expected)}")
        for name in closure["external_skills"]:
            output = f"skills/{name}/SKILL.md"
            if assets.get(output, {}).get("source") != f"upstream/cursor-team-kit/{output}":
                self.errors.append(f"external skill not delivered from its declared source: {name}")
        return declared["generated"]

    def reference(self, source, literal, target, allowed):
        try:
            contents = self.read(target)
            if target not in allowed:
                raise ValueError("target is not a declared package asset")
            self.report["references"].append({"source": source, "reference": literal, "target": target, **fingerprint(contents)})
        except (OSError, ValueError) as error:
            self.errors.append(f"unreadable reference {source}: {literal} -> {target}: {error}")

    def check_references(self, outputs, closure):
        allowed = set(self.files) | {"generated/" + output for output in outputs} | set(closure["package_files"])
        reviewed = {(row["source"], row["literal"]) for row in closure["references"]}
        for name in closure["package_files"]:
            try:
                self.read(name)
            except (OSError, ValueError) as error:
                self.errors.append(f"cannot read package asset {name}: {error}")
        consumers = {"generated/" + output for output in outputs} | set(closure["package_files"])
        for name in sorted(consumers & self.files.keys()):
            suffix = PurePosixPath(name).suffix
            if suffix not in (".md", ".ts"):
                continue
            text = self.files[name].decode("utf-8")
            refs = set()
            if suffix == ".md":
                for link in LINK.findall(text):
                    url = urlsplit(link)
                    if not url.scheme and not url.netloc and (url.path or url.fragment):
                        target = posixpath.normpath(posixpath.join(posixpath.dirname(name), unquote(url.path))) if url.path else name
                        refs.add((link, target))
                for skill in SKILL.findall(text):
                    refs.add((f"/skill:{skill}", f"generated/skills/{skill}/SKILL.md"))
            else:
                for specifier in IMPORT.findall(text):
                    if (name, specifier) in reviewed:
                        continue
                    target = posixpath.normpath(posixpath.join(posixpath.dirname(name), specifier))
                    if not PurePosixPath(target).suffix:
                        target += ".ts"
                    refs.add((specifier, target))
            for literal, target in sorted(refs):
                self.reference(name, literal, target, allowed)
        for row in closure["references"]:
            source, literal, target, kind = (row[k] for k in ("source", "literal", "target", "kind"))
            content = self.files.get(source, b"")
            if literal.encode("utf-8") not in content:
                self.errors.append(f"reviewed reference absent from {source}: {literal}")
            if kind == "package":
                self.reference(source, literal, target, allowed)
            elif kind == "toolkit":
                try:
                    data = read_file(self.package.parent, target)
                    self.report["references"].append({**row, **fingerprint(data)})
                except (OSError, ValueError) as error:
                    self.errors.append(f"unreadable reference {source}: {literal} -> {target}: {error}")
            elif kind == "embedded":
                body = re.search(rb"^" + re.escape(literal.encode()) + rb"\r?\n(.+?)^```\s*$", content, re.M | re.S)
                if body is None or not body[1].strip():
                    self.errors.append(f"unreadable embedded probe: {source}: {literal}")
                else:
                    self.report["references"].append({**row, **fingerprint(body[1])})
            elif kind in ("setup-product", "project-product", "skill-template"):
                self.report["non_package_references"].append(row)
            else:
                self.errors.append(f"unknown reference category: {kind}")
        for name in closure["excluded_skills"]:
            path = f"skills/{name}/SKILL.md"
            if path in outputs:
                self.errors.append(f"deferred skill is declared: {path}")

    def run(self, bot):
        mode = self.package.lstat()
        if not stat.S_ISDIR(mode.st_mode) or is_link_or_junction(mode):
            raise ValueError(f"not a real package directory: {self.package}")
        raw_catalog = read_file(bot, "meta-data/suites.json")
        selection = [row for row in decode_json(raw_catalog)["suites"] if row["id"] == "pstack"]
        roots = {"skills": ["${roots.toolkit}/pstack-suite/generated/skills"], "extensions": [
            "${roots.toolkit}/pstack-suite/omp.ts", "${roots.toolkit}/pstack-suite/generated",
        ]}
        if len(selection) != 1 or any(selection[0][key].get("omp") != value for key, value in roots.items()):
            self.errors.append("declared pstack Suite does not select the checked kit resources")
        self.report["bot_catalog"] = {"path": "meta-data/suites.json", **fingerprint(raw_catalog), "selection": selection}
        closure = self.json("delivery-closure.json")
        if closure["version"] != 1:
            raise ValueError("unsupported delivery closure version")
        outputs = self.check_assets(closure)
        self.check_references(outputs, closure)
        self.report["runtime_prerequisites"] = closure["runtime_prerequisites"]
        self.report["project_products"] = closure["project_products"]
        support = set(closure["package_files"])
        sections = {"upstream", "runtime", "mappings", "generated", "maintenance"}
        user_setup = {"models.local.json", "models-ownership.local.json"}
        for entry in sorted(self.package.iterdir()):
            if entry.name in sections:
                if not entry.is_dir() or is_link_or_junction(entry.lstat()):
                    self.errors.append(f"package section is not a real directory: {entry.name}")
            elif entry.name in user_setup:
                self.report["local_products"].append(entry.name)
            elif entry.name not in support:
                self.errors.append(f"undeclared package asset: {entry.name}")
        self.inventory("maintenance", {name.removeprefix("maintenance/") for name in support if name.startswith("maintenance/")})


def absolute_path(value):
    path = Path(value)
    if not path.is_absolute():
        raise argparse.ArgumentTypeError("use an absolute path")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__, epilog="Exit 0 means the read-only package check passed, not generation, native behavior, or human approval.")
    for option in ("--herdr-bot-worktree", "--toolkit-worktree", "--evidence-dir"):
        parser.add_argument(option, required=True, type=absolute_path)
    args = parser.parse_args()
    bot, toolkit = args.herdr_bot_worktree.resolve(), args.toolkit_worktree.resolve()
    evidence = args.evidence_dir.resolve()
    if Path.cwd() != bot or HERE.parent != bot:
        parser.error("run this checkout's script from the declared herdr-bot worktree root")
    if evidence.is_relative_to(toolkit / "pstack-suite"):
        parser.error("evidence directory cannot be inside the package being checked")
    try:
        evidence.mkdir(parents=True, exist_ok=False)
    except OSError as error:
        parser.error(f"evidence directory must be new: {error}")
    report = {
        "schema": 1, "started_at": stamp(), "command": [sys.executable, *sys.argv], "cwd": str(bot),
        "versions": {"python": platform.python_version(), "platform": platform.platform()},
        "checker": fingerprint(Path(__file__).read_bytes()),
        "inputs": {"herdr-bot": root_identity(bot), "toolkit": root_identity(toolkit)},
        "errors": [], "assets": {}, "files": {}, "references": [], "non_package_references": [],
        "local_products": [], "human": {"H1": "unverified", "H2": "unverified"},
        "not_certified": ["content review", "generation", "assembly", "native mechanisms", "model behavior"],
        "side_effects": {"created_evidence_dir": str(evidence), "written_files": [str(evidence / "summary.json")]},
    }
    try:
        PackageCheck(toolkit / "pstack-suite", report).run(bot)
    except (OSError, ValueError, TypeError, KeyError) as error:
        report["errors"].append(f"cannot check package: {error}")
    report["passed"] = not report["errors"]
    report["exit"] = 0 if report["passed"] else 1
    report["finished_at"] = stamp()
    (evidence / "summary.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    for error in report["errors"]:
        print(error, file=sys.stderr)
    print(f"pstack package: {'passed' if report['passed'] else 'failed'}; {evidence / 'summary.json'}")
    return report["exit"]


if __name__ == "__main__":
    sys.exit(main())
