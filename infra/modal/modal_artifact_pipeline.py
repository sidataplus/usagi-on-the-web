# /// script
# dependencies = [
#   "huggingface_hub>=0.36.0",
#   "modal>=1.0.0",
#   "requests>=2.32.0",
# ]
# ///
"""Modal pipeline for GPU-targeted search and mapper artifact stages.

This script intentionally reuses the existing Rust build path:

1. start the relevant API service inside a Modal GPU function
2. submit the same async build job used by the runtime API
3. run api-worker for the target queue with --once
4. package the generated artifact directories

Inputs must already be staged in the Modal Volume at /mnt/usagi/data.
See infra/modal/README.md for operator commands.
"""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import time
from pathlib import Path
from typing import Any

import modal
import requests


APP_NAME = "usagi-artifact-builds"
VOLUME_NAME = os.environ.get("USAGI_MODAL_VOLUME", "usagi-artifacts-data")
GPU = os.environ.get("USAGI_MODAL_GPU", "L40S")
HF_SECRET_NAME = os.environ.get("USAGI_MODAL_HF_SECRET")
REPO_ROOT = Path(__file__).resolve().parents[2]
REMOTE_REPO = Path("/repo")
MOUNT_ROOT = Path("/mnt/usagi")
DATA_ROOT = MOUNT_ROOT / "data"
EXPORT_ROOT = MOUNT_ROOT / "exports"
BIN_DIR = REMOTE_REPO / "target" / "release"
TERMINAL_JOB_STATES = {"succeeded", "succeeded_with_errors", "failed", "cancelled"}

volume = modal.Volume.from_name(VOLUME_NAME, create_if_missing=True)
app = modal.App(APP_NAME)
function_secrets = [modal.Secret.from_name(HF_SECRET_NAME)] if HF_SECRET_NAME else []


def _ignore_local_repo(path: str) -> bool:
    ignored_parts = {
        ".git",
        ".bundle",
        ".cursor",
        ".venv",
        "data",
        "node_modules",
        "target",
        "tmp",
        "vendor",
    }
    return any(part in ignored_parts for part in Path(path).parts)


image = (
    modal.Image.from_registry("rust:1.85-bookworm", add_python="3.12")
    .apt_install(
        "build-essential",
        "ca-certificates",
        "clang",
        "curl",
        "libssl-dev",
        "pkg-config",
        "tar",
        "zstd",
    )
    .pip_install("huggingface_hub>=0.36.0", "requests>=2.32.0")
    .add_local_dir(
        str(REPO_ROOT),
        remote_path=str(REMOTE_REPO),
        ignore=_ignore_local_repo,
    )
    .run_commands(
        "cd /repo && cargo build --release -p search-api -p mapper-api -p api-worker",
    )
)


@app.function(
    image=image,
    gpu=GPU,
    timeout=24 * 60 * 60,
    volumes={str(MOUNT_ROOT): volume},
    ephemeral_disk=250_000,
    secrets=function_secrets,
)
def build_sapbert_index(
    *,
    idempotency_key: str,
    artifact_id: str,
    catalog_artifact_id: str,
    model_artifact_id: str,
    batch_size: int = 32,
    overwrite: bool = False,
    export_name: str | None = None,
    hf_repo_id: str | None = None,
    hf_private: bool = True,
) -> dict[str, Any]:
    """Build the SapBERT CLS + USearch artifact and optionally upload the pack."""

    _ensure_catalog_inputs()
    _ensure_sapbert_inputs()
    EXPORT_ROOT.mkdir(parents=True, exist_ok=True)
    _clean_job_state()

    api_key = "modal-build-secret"
    env = _build_env(api_key)
    search = subprocess.Popen(
        [str(BIN_DIR / "search-api")],
        cwd=str(REMOTE_REPO),
        env=env,
    )
    try:
        _wait_for_service("http://127.0.0.1:8789/search/health", "search-api")
        job = _submit_sapbert_job(
            api_key=api_key,
            idempotency_key=idempotency_key,
            model_artifact_id=model_artifact_id,
            batch_size=batch_size,
            overwrite=overwrite,
        )
        subprocess.run(
            [str(BIN_DIR / "usagi-worker"), "--queues", "embed", "--once"],
            cwd=str(REMOTE_REPO),
            env=env,
            check=True,
        )
        status = _poll_job(job["job_id"], port=8789)
        if status["state"] != "succeeded":
            raise RuntimeError(f"Modal SapBERT build job failed: {status}")

        sapbert_dir = DATA_ROOT / "search" / "sapbert"
        _assert_sapbert_output(sapbert_dir)
        _stamp_artifact_provenance(
            manifest_path=sapbert_dir / "manifest.json",
            artifact_id=artifact_id,
            catalog_artifact_id=catalog_artifact_id,
            model_artifact_id=model_artifact_id,
        )
        pack_path = _create_pack(
            export_name=export_name or f"usagi-sapbert-index-{artifact_id}.tar.zst",
            members=["search/sapbert"],
        )
        result = _pack_result(
            job=job,
            status=status,
            pack_path=pack_path,
            artifact_paths={"sapbert_dir": str(sapbert_dir)},
        )
        if hf_repo_id:
            result["huggingface"] = _upload_pack_to_hf(
                pack_path=pack_path,
                sha256=result["pack_sha256"],
                repo_id=hf_repo_id,
                private=hf_private,
            )
        volume.commit()
        return result
    finally:
        _terminate(search)


@app.function(
    image=image,
    gpu=GPU,
    timeout=24 * 60 * 60,
    volumes={str(MOUNT_ROOT): volume},
    ephemeral_disk=250_000,
    secrets=function_secrets,
)
def build_thirawat_doc_embeddings(
    *,
    idempotency_key: str,
    artifact_id: str,
    catalog_artifact_id: str,
    model_artifact_id: str,
    batch_size: int = 16,
    domain_id: str = "Drug",
    target_vocabulary_ids: tuple[str, ...] = ("RxNorm", "RxNorm Extension"),
    overwrite: bool = False,
    export_name: str | None = None,
    hf_repo_id: str | None = None,
    hf_private: bool = True,
) -> dict[str, Any]:
    """Build THIRAWAT doc embeddings and optionally upload the pack to HF Hub."""

    _ensure_catalog_inputs()
    _ensure_thirawat_inputs()
    EXPORT_ROOT.mkdir(parents=True, exist_ok=True)
    _clean_job_state()

    api_key = "modal-build-secret"
    env = _build_env(api_key)
    mapper = subprocess.Popen(
        [str(BIN_DIR / "mapper-api")],
        cwd=str(REMOTE_REPO),
        env=env,
    )
    try:
        _wait_for_mapper()
        job = _submit_embedding_job(
            api_key=api_key,
            idempotency_key=idempotency_key,
            model_artifact_id=model_artifact_id,
            batch_size=batch_size,
            domain_id=domain_id,
            target_vocabulary_ids=target_vocabulary_ids,
            overwrite=overwrite,
        )
        subprocess.run(
            [str(BIN_DIR / "usagi-worker"), "--queues", "embed", "--once"],
            cwd=str(REMOTE_REPO),
            env=env,
            check=True,
        )
        status = _poll_job(job["job_id"], port=8790)
        if status["state"] != "succeeded":
            raise RuntimeError(f"Modal THIRAWAT doc embedding job failed: {status}")

        doc_dir = DATA_ROOT / "mapper" / "thirawat-drug" / "doc_embeddings"
        _assert_doc_embedding_output(doc_dir)
        _stamp_modal_provenance(
            doc_dir=doc_dir,
            artifact_id=artifact_id,
            catalog_artifact_id=catalog_artifact_id,
            model_artifact_id=model_artifact_id,
        )
        pack_path = _create_pack(
            export_name=export_name or f"usagi-thirawat-docemb-{artifact_id}.tar.zst",
            members=["mapper/thirawat-drug/doc_embeddings"],
        )
        result = _pack_result(
            job=job,
            status=status,
            pack_path=pack_path,
            artifact_paths={"doc_embedding_dir": str(doc_dir)},
        )
        if hf_repo_id:
            result["huggingface"] = _upload_pack_to_hf(
                pack_path=pack_path,
                sha256=result["pack_sha256"],
                repo_id=hf_repo_id,
                private=hf_private,
            )
        volume.commit()
        return result
    finally:
        _terminate(mapper)


@app.function(
    image=image,
    gpu=GPU,
    timeout=24 * 60 * 60,
    volumes={str(MOUNT_ROOT): volume},
    ephemeral_disk=250_000,
    secrets=function_secrets,
)
def build_thirawat_indexes(
    *,
    doc_idempotency_key: str,
    tachiom_idempotency_key: str,
    doc_artifact_id: str,
    tachiom_artifact_id: str,
    catalog_artifact_id: str,
    model_artifact_id: str,
    batch_size: int = 16,
    domain_id: str = "Drug",
    target_vocabulary_ids: tuple[str, ...] = ("RxNorm", "RxNorm Extension"),
    overwrite: bool = False,
    export_name: str | None = None,
    hf_repo_id: str | None = None,
    hf_private: bool = True,
) -> dict[str, Any]:
    """Build THIRAWAT doc embeddings plus the Tachiom retrieval index."""

    _ensure_catalog_inputs()
    _ensure_thirawat_inputs()
    _ensure_tachiom_inputs()
    EXPORT_ROOT.mkdir(parents=True, exist_ok=True)
    _clean_job_state()

    api_key = "modal-build-secret"
    env = _build_env(api_key)
    mapper = subprocess.Popen(
        [str(BIN_DIR / "mapper-api")],
        cwd=str(REMOTE_REPO),
        env=env,
    )
    try:
        _wait_for_mapper()
        doc_job = _submit_embedding_job(
            api_key=api_key,
            idempotency_key=doc_idempotency_key,
            model_artifact_id=model_artifact_id,
            batch_size=batch_size,
            domain_id=domain_id,
            target_vocabulary_ids=target_vocabulary_ids,
            overwrite=overwrite,
        )
        subprocess.run(
            [str(BIN_DIR / "usagi-worker"), "--queues", "embed", "--once"],
            cwd=str(REMOTE_REPO),
            env=env,
            check=True,
        )
        doc_status = _poll_job(doc_job["job_id"], port=8790)
        if doc_status["state"] != "succeeded":
            raise RuntimeError(f"Modal THIRAWAT doc embedding job failed: {doc_status}")

        doc_dir = DATA_ROOT / "mapper" / "thirawat-drug" / "doc_embeddings"
        _assert_doc_embedding_output(doc_dir)
        _stamp_artifact_provenance(
            manifest_path=doc_dir / "manifest.json",
            artifact_id=doc_artifact_id,
            catalog_artifact_id=catalog_artifact_id,
            model_artifact_id=model_artifact_id,
        )

        tachiom_job = _submit_tachiom_job(
            api_key=api_key,
            idempotency_key=tachiom_idempotency_key,
            doc_embedding_artifact_id=doc_artifact_id,
            overwrite=overwrite,
        )
        subprocess.run(
            [str(BIN_DIR / "usagi-worker"), "--queues", "index", "--once"],
            cwd=str(REMOTE_REPO),
            env=env,
            check=True,
        )
        tachiom_status = _poll_job(tachiom_job["job_id"], port=8790)
        if tachiom_status["state"] != "succeeded":
            raise RuntimeError(f"Modal Tachiom build job failed: {tachiom_status}")

        tachiom_dir = DATA_ROOT / "mapper" / "thirawat-drug" / "tachiom"
        _assert_tachiom_output(tachiom_dir)
        _stamp_artifact_provenance(
            manifest_path=tachiom_dir / "manifest.json",
            artifact_id=tachiom_artifact_id,
            catalog_artifact_id=catalog_artifact_id,
            model_artifact_id=model_artifact_id,
        )
        pack_path = _create_pack(
            export_name=export_name or f"usagi-thirawat-index-{tachiom_artifact_id}.tar.zst",
            members=[
                "mapper/thirawat-drug/doc_embeddings",
                "mapper/thirawat-drug/tachiom",
            ],
        )
        result = _pack_result(
            job=tachiom_job,
            status=tachiom_status,
            pack_path=pack_path,
            artifact_paths={
                "doc_embedding_dir": str(doc_dir),
                "tachiom_dir": str(tachiom_dir),
            },
        )
        result["doc_embedding_job_id"] = doc_job["job_id"]
        if hf_repo_id:
            result["huggingface"] = _upload_pack_to_hf(
                pack_path=pack_path,
                sha256=result["pack_sha256"],
                repo_id=hf_repo_id,
                private=hf_private,
            )
        volume.commit()
        return result
    finally:
        _terminate(mapper)


@app.function(
    image=image,
    gpu=GPU,
    timeout=24 * 60 * 60,
    volumes={str(MOUNT_ROOT): volume},
    ephemeral_disk=250_000,
    secrets=function_secrets,
)
def build_all_indexes(
    *,
    sapbert_idempotency_key: str,
    doc_idempotency_key: str,
    tachiom_idempotency_key: str,
    sapbert_artifact_id: str,
    doc_artifact_id: str,
    tachiom_artifact_id: str,
    catalog_artifact_id: str,
    sapbert_model_artifact_id: str,
    thirawat_model_artifact_id: str,
    sapbert_batch_size: int = 32,
    thirawat_batch_size: int = 16,
    overwrite: bool = False,
    export_name: str | None = None,
    hf_repo_id: str | None = None,
    hf_private: bool = True,
) -> dict[str, Any]:
    """Build SapBERT, THIRAWAT doc embeddings, and Tachiom in one Modal run."""

    sapbert = build_sapbert_index.local(
        idempotency_key=sapbert_idempotency_key,
        artifact_id=sapbert_artifact_id,
        catalog_artifact_id=catalog_artifact_id,
        model_artifact_id=sapbert_model_artifact_id,
        batch_size=sapbert_batch_size,
        overwrite=overwrite,
    )
    thirawat = build_thirawat_indexes.local(
        doc_idempotency_key=doc_idempotency_key,
        tachiom_idempotency_key=tachiom_idempotency_key,
        doc_artifact_id=doc_artifact_id,
        tachiom_artifact_id=tachiom_artifact_id,
        catalog_artifact_id=catalog_artifact_id,
        model_artifact_id=thirawat_model_artifact_id,
        batch_size=thirawat_batch_size,
        overwrite=overwrite,
    )
    pack_path = _create_pack(
        export_name=export_name or f"usagi-indexes-{catalog_artifact_id}.tar.zst",
        members=[
            "search/sapbert",
            "mapper/thirawat-drug/doc_embeddings",
            "mapper/thirawat-drug/tachiom",
        ],
    )
    result = {
        "state": "succeeded",
        "sapbert": sapbert,
        "thirawat": thirawat,
        "pack_path": str(pack_path),
        "pack_sha256": _sha256(pack_path),
        "pack_size_bytes": pack_path.stat().st_size,
    }
    if hf_repo_id:
        result["huggingface"] = _upload_pack_to_hf(
            pack_path=pack_path,
            sha256=result["pack_sha256"],
            repo_id=hf_repo_id,
            private=hf_private,
        )
    volume.commit()
    return result


def _build_env(api_key: str) -> dict[str, str]:
    env = os.environ.copy()
    env.update(
        {
            "MAPPER_API_ADDR": "127.0.0.1:8790",
            "SEARCH_API_ADDR": "127.0.0.1:8789",
            "USAGI_API_ENV": "development",
            "USAGI_API_AUTH_MODE": "api_key",
            "USAGI_API_KEYS": api_key,
            "JOBS_DB_PATH": str(DATA_ROOT / "jobs" / "jobs.sqlite"),
            "JOB_RESULTS_DIR": str(DATA_ROOT / "jobs" / "results"),
            "CATALOG_DB_PATH": str(DATA_ROOT / "catalog" / "catalog.sqlite"),
            "SAPBERT_INDEX_DIR": str(DATA_ROOT / "search" / "sapbert"),
            "SAPBERT_MODEL_DIR": str(DATA_ROOT / "models" / "sapbert"),
            "THIRAWAT_MODEL_DIR": str(DATA_ROOT / "models" / "thirawat-sapbert"),
            "THIRAWAT_ARTIFACT_DIR": str(DATA_ROOT / "mapper" / "thirawat-drug"),
            "TACHIOM_INDEX_DIR": str(DATA_ROOT / "mapper" / "thirawat-drug" / "tachiom"),
            "THIRAWAT_QUERY_EMBEDDINGS_PATH": str(
                DATA_ROOT / "mapper" / "thirawat-drug" / "query_embeddings.json"
            ),
        }
    )
    return env


def _ensure_catalog_inputs() -> None:
    required = [
        DATA_ROOT / "catalog" / "catalog.sqlite",
        DATA_ROOT / "catalog" / "manifest.json",
    ]
    _ensure_paths(required)


def _ensure_sapbert_inputs() -> None:
    required = [
        DATA_ROOT / "models" / "sapbert" / "config.json",
        DATA_ROOT / "models" / "sapbert" / "tokenizer.json",
        DATA_ROOT / "models" / "sapbert" / "model.safetensors",
    ]
    _ensure_paths(required)


def _ensure_thirawat_inputs() -> None:
    required = [
        DATA_ROOT / "models" / "thirawat-sapbert" / "config.json",
        DATA_ROOT / "models" / "thirawat-sapbert" / "tokenizer.json",
        DATA_ROOT / "models" / "thirawat-sapbert" / "model.safetensors",
        DATA_ROOT / "models" / "thirawat-sapbert" / "colbert_projection.safetensors",
    ]
    _ensure_paths(required)


def _ensure_tachiom_inputs() -> None:
    build_bin = os.environ.get("TACHIOM_BUILD_BIN")
    if not build_bin:
        raise FileNotFoundError(
            "Set TACHIOM_BUILD_BIN to a staged Tachiom build binary before running "
            "the production THIRAWAT index build."
        )
    _ensure_paths([Path(build_bin)])


def _ensure_paths(required: list[Path]) -> None:
    missing = [str(path) for path in required if not path.exists()]
    if missing:
        raise FileNotFoundError(
            "Stage Modal inputs into the Volume before running the build: "
            + ", ".join(missing)
        )


def _clean_job_state() -> None:
    (DATA_ROOT / "jobs" / "results").mkdir(parents=True, exist_ok=True)
    jobs_db = DATA_ROOT / "jobs" / "jobs.sqlite"
    if jobs_db.exists():
        jobs_db.unlink()


def _wait_for_mapper() -> None:
    _wait_for_service("http://127.0.0.1:8790/mapper/health", "mapper-api")


def _wait_for_service(url: str, label: str) -> None:
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        try:
            response = requests.get(url, timeout=2)
            if response.status_code == 200:
                return
        except requests.RequestException:
            time.sleep(1)
    raise TimeoutError(f"{label} did not become healthy on Modal")


def _submit_sapbert_job(
    *,
    api_key: str,
    idempotency_key: str,
    model_artifact_id: str,
    batch_size: int,
    overwrite: bool,
) -> dict[str, Any]:
    response = requests.post(
        "http://127.0.0.1:8789/search/sapbert/build-job",
        headers={"X-API-Key": api_key, "Content-Type": "application/json"},
        json={
            "idempotency_key": idempotency_key,
            "overwrite": overwrite,
            "model_artifact_id": model_artifact_id,
            "batch_size": batch_size,
            "scope": {"standard_concept": "S", "invalid_reason": None},
        },
        timeout=30,
    )
    response.raise_for_status()
    return response.json()


def _submit_embedding_job(
    *,
    api_key: str,
    idempotency_key: str,
    model_artifact_id: str,
    batch_size: int,
    domain_id: str,
    target_vocabulary_ids: tuple[str, ...],
    overwrite: bool,
) -> dict[str, Any]:
    response = requests.post(
        "http://127.0.0.1:8790/mapper/thirawat/build-embeddings-job",
        headers={"X-API-Key": api_key, "Content-Type": "application/json"},
        json={
            "idempotency_key": idempotency_key,
            "overwrite": overwrite,
            "domain_id": domain_id,
            "model_artifact_id": model_artifact_id,
            "batch_size": batch_size,
            "target_scope": {
                "domain_id": [domain_id],
                "vocabulary_id": list(target_vocabulary_ids),
            },
        },
        timeout=30,
    )
    response.raise_for_status()
    return response.json()


def _submit_tachiom_job(
    *,
    api_key: str,
    idempotency_key: str,
    doc_embedding_artifact_id: str,
    overwrite: bool,
) -> dict[str, Any]:
    response = requests.post(
        "http://127.0.0.1:8790/mapper/tachiom/build-index-job",
        headers={"X-API-Key": api_key, "Content-Type": "application/json"},
        json={
            "idempotency_key": idempotency_key,
            "overwrite": overwrite,
            "doc_embedding_artifact_id": doc_embedding_artifact_id,
            "build_params": {
                "metric": "maxsim",
                "token_aware_clustering": True,
            },
        },
        timeout=30,
    )
    response.raise_for_status()
    return response.json()


def _poll_job(job_id: str, *, port: int) -> dict[str, Any]:
    deadline = time.monotonic() + 24 * 60 * 60
    while time.monotonic() < deadline:
        response = requests.get(f"http://127.0.0.1:{port}/jobs/{job_id}", timeout=10)
        response.raise_for_status()
        status = response.json()
        if status["state"] in TERMINAL_JOB_STATES:
            return status
        time.sleep(10)
    raise TimeoutError(f"timed out waiting for {job_id}")


def _assert_sapbert_output(sapbert_dir: Path) -> None:
    for name in ["sapbert_cls.usearch", "concept_ids.arrow", "manifest.json"]:
        path = sapbert_dir / name
        if not path.exists():
            raise FileNotFoundError(f"missing SapBERT output {path}")


def _assert_doc_embedding_output(doc_dir: Path) -> None:
    for name in [
        "token_vectors.npy",
        "token_ids.npy",
        "doclens.npy",
        "doc_ids.arrow",
        "manifest.json",
    ]:
        path = doc_dir / name
        if not path.exists():
            raise FileNotFoundError(f"missing THIRAWAT doc embedding output {path}")


def _assert_tachiom_output(tachiom_dir: Path) -> None:
    for name in ["index.bin", "manifest.json"]:
        path = tachiom_dir / name
        if not path.exists():
            raise FileNotFoundError(f"missing Tachiom output {path}")


def _stamp_modal_provenance(
    *,
    doc_dir: Path,
    artifact_id: str,
    catalog_artifact_id: str,
    model_artifact_id: str,
) -> None:
    _stamp_artifact_provenance(
        manifest_path=doc_dir / "manifest.json",
        artifact_id=artifact_id,
        catalog_artifact_id=catalog_artifact_id,
        model_artifact_id=model_artifact_id,
    )


def _stamp_artifact_provenance(
    *,
    manifest_path: Path,
    artifact_id: str,
    catalog_artifact_id: str,
    model_artifact_id: str,
) -> None:
    manifest = json.loads(manifest_path.read_text())
    manifest["artifact_id"] = artifact_id
    manifest.setdefault("created_by", {})
    manifest["created_by"].update({"service": "modal", "pipeline": APP_NAME})
    extra = manifest.setdefault("extra", {})
    catalog = extra.setdefault("catalog", {})
    catalog["artifact_id"] = catalog_artifact_id
    model = extra.setdefault("model", {})
    model["model_artifact_id"] = model_artifact_id
    extra["modal"] = {
        "volume": VOLUME_NAME,
        "gpu": GPU,
        "image": "rust:1.85-bookworm",
    }
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")


def _create_pack(*, export_name: str, members: list[str]) -> Path:
    pack_path = EXPORT_ROOT / export_name
    if pack_path.exists():
        pack_path.unlink()
    tar_args = [
        "tar",
        "--zstd",
        "-cf",
        str(pack_path),
        "-C",
        str(DATA_ROOT),
        *members,
    ]
    subprocess.run(
        tar_args,
        check=True,
    )
    manifests = {}
    for member in members:
        manifest_path = DATA_ROOT / member / "manifest.json"
        if manifest_path.exists():
            manifests[f"{member}/manifest.json"] = json.loads(manifest_path.read_text())
    manifest = {
        "pack_kind": "usagi-artifact-pack",
        "pack_path": str(pack_path),
        "contains": members,
        "manifests": manifests,
        "sha256": _sha256(pack_path),
        "created_by": {"service": "modal", "pipeline": APP_NAME},
    }
    (pack_path.with_suffix(pack_path.suffix + ".manifest.json")).write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n"
    )
    return pack_path


def _pack_result(
    *,
    job: dict[str, Any],
    status: dict[str, Any],
    pack_path: Path,
    artifact_paths: dict[str, str],
) -> dict[str, Any]:
    return {
        "job_id": job["job_id"],
        "state": status["state"],
        **artifact_paths,
        "pack_path": str(pack_path),
        "pack_sha256": _sha256(pack_path),
        "pack_size_bytes": pack_path.stat().st_size,
    }


def _terminate(process: subprocess.Popen[Any]) -> None:
    process.terminate()
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()


def _upload_pack_to_hf(
    *,
    pack_path: Path,
    sha256: str,
    repo_id: str,
    private: bool,
) -> dict[str, str]:
    from huggingface_hub import HfApi

    token = os.environ.get("HF_TOKEN")
    if not token:
        raise RuntimeError("set HF_TOKEN as a Modal secret before uploading to Hugging Face")
    api = HfApi(token=token)
    api.create_repo(repo_id=repo_id, repo_type="dataset", private=private, exist_ok=True)
    api.upload_file(
        path_or_fileobj=str(pack_path),
        path_in_repo=pack_path.name,
        repo_id=repo_id,
        repo_type="dataset",
    )
    manifest_path = pack_path.with_suffix(pack_path.suffix + ".manifest.json")
    api.upload_file(
        path_or_fileobj=str(manifest_path),
        path_in_repo=manifest_path.name,
        repo_id=repo_id,
        repo_type="dataset",
    )
    readme = (
        "# Usagi THIRAWAT Document Embeddings\n\n"
        "Private artifact pack for Usagi-on-the-Web runtime deployment.\n\n"
        f"- Pack: `{pack_path.name}`\n"
        f"- SHA256: `{sha256}`\n"
        "- Restore path: `data/mapper/thirawat-drug/doc_embeddings/`\n"
        "- Verify against included `manifest.json` and pack manifest before serving.\n"
    )
    readme_path = EXPORT_ROOT / "README.md"
    readme_path.write_text(readme)
    api.upload_file(
        path_or_fileobj=str(readme_path),
        path_in_repo="README.md",
        repo_id=repo_id,
        repo_type="dataset",
    )
    return {
        "repo_id": repo_id,
        "repo_type": "dataset",
        "pack": pack_path.name,
    }


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


@app.local_entrypoint()
def run(
    mode: str = "thirawat",
    vocabulary_version: str = "20250827",
    catalog_artifact_id: str | None = None,
    sapbert_model_artifact_id: str = "sapbert-xlmr-merged-v1",
    thirawat_model_artifact_id: str = "sidataplus-thirawat-sapbert-merged-v1",
    sapbert_batch_size: int = 32,
    thirawat_batch_size: int = 16,
    overwrite: bool = False,
    hf_repo_id: str | None = None,
    hf_private: bool = True,
) -> None:
    catalog_artifact_id = catalog_artifact_id or f"athena-{vocabulary_version}-standard-v1"
    sapbert_artifact_id = f"athena-{vocabulary_version}-sapbert-v1"
    doc_artifact_id = f"athena-{vocabulary_version}-thirawat-drug-docemb-v1"
    tachiom_artifact_id = f"athena-{vocabulary_version}-thirawat-drug-tachiom-v1"
    common = {
        "catalog_artifact_id": catalog_artifact_id,
        "overwrite": overwrite,
        "hf_repo_id": hf_repo_id,
        "hf_private": hf_private,
    }
    if mode == "sapbert":
        result = build_sapbert_index.remote(
            idempotency_key=f"sapbert-athena-{vocabulary_version}-standard-v1",
            artifact_id=sapbert_artifact_id,
            model_artifact_id=sapbert_model_artifact_id,
            batch_size=sapbert_batch_size,
            **common,
        )
    elif mode == "thirawat":
        result = build_thirawat_indexes.remote(
            doc_idempotency_key=f"thirawat-docemb-athena-{vocabulary_version}-drug-v1",
            tachiom_idempotency_key=f"tachiom-athena-{vocabulary_version}-thirawat-drug-v1",
            doc_artifact_id=doc_artifact_id,
            tachiom_artifact_id=tachiom_artifact_id,
            model_artifact_id=thirawat_model_artifact_id,
            batch_size=thirawat_batch_size,
            **common,
        )
    elif mode == "all":
        result = build_all_indexes.remote(
            sapbert_idempotency_key=f"sapbert-athena-{vocabulary_version}-standard-v1",
            doc_idempotency_key=f"thirawat-docemb-athena-{vocabulary_version}-drug-v1",
            tachiom_idempotency_key=f"tachiom-athena-{vocabulary_version}-thirawat-drug-v1",
            sapbert_artifact_id=sapbert_artifact_id,
            doc_artifact_id=doc_artifact_id,
            tachiom_artifact_id=tachiom_artifact_id,
            sapbert_model_artifact_id=sapbert_model_artifact_id,
            thirawat_model_artifact_id=thirawat_model_artifact_id,
            sapbert_batch_size=sapbert_batch_size,
            thirawat_batch_size=thirawat_batch_size,
            **common,
        )
    elif mode == "doc-embeddings":
        result = build_thirawat_doc_embeddings.remote(
            idempotency_key=f"thirawat-docemb-athena-{vocabulary_version}-drug-v1",
            artifact_id=doc_artifact_id,
            model_artifact_id=thirawat_model_artifact_id,
            batch_size=thirawat_batch_size,
            **common,
        )
    else:
        raise ValueError("mode must be one of: all, sapbert, thirawat, doc-embeddings")
    print(json.dumps(result, indent=2, sort_keys=True))
