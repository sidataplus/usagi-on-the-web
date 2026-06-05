from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import types
import unittest
from pathlib import Path
from unittest import mock


def load_pipeline():
    module_name = "modal_artifact_pipeline_under_test"
    sys.modules.pop(module_name, None)

    fake_modal = types.ModuleType("modal")

    class FakeVolume:
        @classmethod
        def from_name(cls, *_args, **_kwargs):
            return cls()

        def commit(self):
            return None

    class FakeApp:
        def __init__(self, *_args, **_kwargs):
            pass

        def function(self, *_args, **_kwargs):
            def decorator(fn):
                fn.remote = fn
                fn.local = fn
                return fn

            return decorator

        def local_entrypoint(self, *_args, **_kwargs):
            def decorator(fn):
                return fn

            return decorator

    class FakeImage:
        @classmethod
        def from_registry(cls, *_args, **_kwargs):
            return cls()

        def apt_install(self, *_args, **_kwargs):
            return self

        def pip_install(self, *_args, **_kwargs):
            return self

        def add_local_dir(self, *_args, **_kwargs):
            return self

        def run_commands(self, *_args, **_kwargs):
            return self

    class FakeSecret:
        @classmethod
        def from_name(cls, *_args, **_kwargs):
            return cls()

    fake_modal.Volume = FakeVolume
    fake_modal.App = FakeApp
    fake_modal.Image = FakeImage
    fake_modal.Secret = FakeSecret
    sys.modules["modal"] = fake_modal

    path = Path(__file__).with_name("modal_artifact_pipeline.py")
    spec = importlib.util.spec_from_file_location(module_name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec and spec.loader
    sys.modules[module_name] = module
    spec.loader.exec_module(module)
    return module


class ModalArtifactPipelineTest(unittest.TestCase):
    def test_submit_sapbert_build_job_posts_to_search_api(self):
        pipeline = load_pipeline()
        response = mock.Mock()
        response.json.return_value = {"job_id": "job_sapbert_1"}

        with mock.patch.object(pipeline.requests, "post", return_value=response) as post:
            result = pipeline._submit_sapbert_job(
                api_key="secret",
                idempotency_key="sapbert-athena-20260227-standard-v1",
                model_artifact_id="sapbert-xlmr-merged-v1",
                batch_size=64,
                overwrite=True,
            )

        self.assertEqual(result["job_id"], "job_sapbert_1")
        response.raise_for_status.assert_called_once_with()
        post.assert_called_once_with(
            "http://127.0.0.1:8789/search/sapbert/build-job",
            headers={"X-API-Key": "secret", "Content-Type": "application/json"},
            json={
                "idempotency_key": "sapbert-athena-20260227-standard-v1",
                "overwrite": True,
                "model_artifact_id": "sapbert-xlmr-merged-v1",
                "batch_size": 64,
                "scope": {"standard_concept": "S", "invalid_reason": None},
            },
            timeout=30,
        )

    def test_poll_job_uses_service_port(self):
        pipeline = load_pipeline()
        response = mock.Mock()
        response.json.return_value = {"state": "succeeded"}

        with mock.patch.object(pipeline.requests, "get", return_value=response) as get:
            status = pipeline._poll_job("job_1", port=8789)

        self.assertEqual(status["state"], "succeeded")
        get.assert_called_once_with("http://127.0.0.1:8789/jobs/job_1", timeout=10)

    def test_pack_artifacts_can_include_sapbert_and_tachiom_outputs(self):
        pipeline = load_pipeline()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            data_root = root / "data"
            export_root = root / "exports"
            sapbert_dir = data_root / "search" / "sapbert"
            doc_dir = data_root / "mapper" / "thirawat-drug" / "doc_embeddings"
            tachiom_dir = data_root / "mapper" / "thirawat-drug" / "tachiom"
            for directory in [sapbert_dir, doc_dir, tachiom_dir, export_root]:
                directory.mkdir(parents=True)
                (directory / "manifest.json").write_text(
                    json.dumps({"artifact_id": directory.name}) + "\n"
                )

            with mock.patch.object(pipeline, "DATA_ROOT", data_root), mock.patch.object(
                pipeline, "EXPORT_ROOT", export_root
            ), mock.patch.object(pipeline.subprocess, "run") as run, mock.patch.object(
                pipeline, "_sha256", return_value="abc123"
            ):
                pack_path = pipeline._create_pack(
                    export_name="full-pack.tar.zst",
                    members=[
                        "search/sapbert",
                        "mapper/thirawat-drug/doc_embeddings",
                        "mapper/thirawat-drug/tachiom",
                    ],
                )

            self.assertEqual(pack_path, export_root / "full-pack.tar.zst")
            run.assert_called_once()
            manifest = json.loads(
                (export_root / "full-pack.tar.zst.manifest.json").read_text()
            )
            self.assertEqual(
                manifest["contains"],
                [
                    "search/sapbert",
                    "mapper/thirawat-drug/doc_embeddings",
                    "mapper/thirawat-drug/tachiom",
                ],
            )
            self.assertIn("search/sapbert/manifest.json", manifest["manifests"])

    def test_run_dispatches_requested_pipeline_mode(self):
        pipeline = load_pipeline()

        with mock.patch.object(
            pipeline.build_all_indexes, "remote", return_value={"mode": "all"}
        ) as all_remote, mock.patch.object(
            pipeline.build_sapbert_index, "remote", return_value={"mode": "sapbert"}
        ) as sapbert_remote, mock.patch.object(
            pipeline.build_thirawat_indexes, "remote", return_value={"mode": "thirawat"}
        ) as thirawat_remote, mock.patch("builtins.print"):
            pipeline.run(mode="all")
            pipeline.run(mode="sapbert")
            pipeline.run(mode="thirawat")

        all_remote.assert_called_once()
        sapbert_remote.assert_called_once()
        thirawat_remote.assert_called_once()


if __name__ == "__main__":
    unittest.main()
