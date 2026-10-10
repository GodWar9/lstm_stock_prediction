import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("bundle", Path(__file__).with_name("deployment_bundle.py"))
bundle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bundle)
SOURCE = Path(__file__).resolve().parents[1] / "deploy/artifacts.tar.gz"

class BundleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.archive = self.root / "artifacts.tar.gz"
        self.archive.write_bytes(SOURCE.read_bytes())
        self.archive.with_suffix(".json").write_bytes(SOURCE.with_suffix(".json").read_bytes())

    def test_valid_bundle_extracts_and_repacking_is_identical(self):
        output = self.root / "workspace"
        bundle.extract(self.archive, output)
        info = json.loads((output / "bundle.json").read_text())
        repacked = self.root / "repacked.tar.gz"
        bundle.pack(output, info["model"], info["dataset"], info["symbol"], repacked)
        self.assertEqual(repacked.read_bytes(), self.archive.read_bytes())

    def test_modified_archive_is_rejected(self):
        self.archive.write_bytes(self.archive.read_bytes() + b"tampered")
        with self.assertRaisesRegex(ValueError, "checksum"):
            bundle.extract(self.archive, self.root / "out")

    def test_nonempty_target_is_preserved(self):
        output = self.root / "workspace"
        output.mkdir()
        (output / "keep.txt").write_text("keep")
        with self.assertRaisesRegex(ValueError, "empty"):
            bundle.extract(self.archive, output)
        self.assertEqual((output / "keep.txt").read_text(), "keep")

    def test_paths_links_and_unexpected_members_rejected_even_with_updated_archive_hash(self):
        for name, kind in [("../escape", tarfile.REGTYPE), ("/escape", tarfile.REGTYPE),
                           ("models/link", tarfile.SYMTYPE), ("C:/escape", tarfile.REGTYPE)]:
            with self.subTest(name=name):
                entry = tarfile.TarInfo(name)
                entry.type = kind
                entry.size = 1 if kind == tarfile.REGTYPE else 0
                with tarfile.open(self.archive, "w:gz") as archive:
                    archive.addfile(entry, io.BytesIO(b"x"))
                meta = json.loads(SOURCE.with_suffix(".json").read_text())
                meta["bundle_sha256"] = hashlib.sha256(self.archive.read_bytes()).hexdigest()
                self.archive.with_suffix(".json").write_text(json.dumps(meta))
                with self.assertRaises(ValueError):
                    bundle.extract(self.archive, self.root / "out")

if __name__ == "__main__":
    unittest.main()
