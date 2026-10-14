"""Synthetic protocol verification; never loads a user's database or assets."""
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
from tempfile import TemporaryDirectory

ROOT = Path(__file__).resolve().parents[2]
LABS = ROOT / "tools" / "development"
sys.path.insert(0, str(LABS / "research-package-reference"))
from adapter import export_project_metadata
from package import inspect, import_into_temporary_database


def cargo(module, binary, *arguments):
    environment = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / "src-tauri" / "target"))
    completed = subprocess.run(
        ["cargo", "run", "--locked", "--jobs", "1", "--manifest-path",
         str(LABS / module / "Cargo.toml"), "--bin", binary, "--", *map(str, arguments)],
        cwd=ROOT, env=environment, capture_output=True, text=True,
    )
    if completed.returncode:
        raise RuntimeError("archive_compatibility_build_or_fixture_failed")
    return completed.stdout


def main():
    with TemporaryDirectory(prefix="suna-synthetic-archive-") as directory:
        temporary = Path(directory)
        source, rust_zip, python_zip = [temporary / name for name in
                                      ("synthetic-manifest.json", "rust.zip", "python.zip")]
        cargo("research-workflow", "project_manifest_fixture", source)
        rust_summary = json.loads(cargo("research-package", "synthetic_package_fixture", source, rust_zip))
        python_read = inspect(rust_zip)
        assert rust_summary["receiptSha256"] == python_read["receipt_sha256"]
        assert len(python_read["objects"]) == 1101 and len(python_read["edges"]) == 4
        connection = sqlite3.connect(":memory:")
        assert import_into_temporary_database(connection, python_read) == import_into_temporary_database(connection, python_read)
        connection.close()
        python_write = export_project_metadata(json.loads(source.read_text(encoding="utf-8")), python_zip)
        rust_read = json.loads(cargo("research-package", "synthetic_package_fixture", "--inspect", python_zip))
        assert rust_read["receiptSha256"] == python_write["receipt_sha256"]
        assert rust_read["idempotenceVerified"] is True
        assert rust_read["assetsRestored"] is False and rust_read["writesToLiveWorkspace"] is False
        print(json.dumps({"rustPythonMutualRead": True, "recordCount": 1101, "edgeCount": 4,
                          "idempotenceVerified": True, "assetsRestored": False,
                          "writesToLiveWorkspace": False}, ensure_ascii=False))


if __name__ == "__main__":
    main()
