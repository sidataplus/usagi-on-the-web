#!/usr/bin/env python3
# /// script
# dependencies = [
#   "huggingface_hub>=0.36.0",
#   "peft>=0.18.0",
#   "safetensors>=0.7.0",
#   "torch>=2.8.0",
#   "transformers>=4.56.0",
# ]
# ///
"""Stage sidataplus/THIRAWAT-SapBERT in the runtime artifact layout."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from huggingface_hub import snapshot_download


REPO_ID = "sidataplus/THIRAWAT-SapBERT"
BASE_MODEL = "cambridgeltl/SapBERT-UMLS-2020AB-all-lang-from-XLMR"
REQUIRED = [
    "config.json",
    "tokenizer.json",
    "tokenizer_config.json",
    "special_tokens_map.json",
    "model.safetensors",
    "colbert_projection.safetensors",
]
PROJECTION_CANDIDATES = ["colbert_projection.safetensors", "1_Dense/model.safetensors"]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", default="data/models/thirawat-sapbert")
    parser.add_argument("--repo-id", default=REPO_ID)
    args = parser.parse_args()

    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    adapter_snapshot = Path(
        snapshot_download(
            repo_id=args.repo_id,
            allow_patterns=[
                "tokenizer.json",
                "tokenizer_config.json",
                "special_tokens_map.json",
                "sentencepiece.bpe.model",
                "adapter_config.json",
                "adapter_model.safetensors",
                "1_Dense/config.json",
                "1_Dense/model.safetensors",
                "*.safetensors",
            ],
        )
    )

    merge_transformer(adapter_snapshot, output_dir)
    copy_required(adapter_snapshot, output_dir)
    manifest = {
        "model_id": args.repo_id,
        "base_model": BASE_MODEL,
        "architecture": "pylate_colbert",
        "encoder_family": "xlm-roberta",
        "query_length": 96,
        "document_length": 96,
        "hidden_dim": 768,
        "projection_dim": 128,
        "similarity": "maxsim",
        "projection": {
            "in_features": 768,
            "out_features": 128,
            "bias": False,
        },
        "peft": {
            "merged": True,
        },
        "sha256": {
            "model.safetensors": sha256(output_dir / "model.safetensors"),
            "colbert_projection.safetensors": sha256(
                output_dir / "colbert_projection.safetensors"
            ),
        },
    }
    (output_dir / "manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n"
    )
    print(output_dir)


def merge_transformer(adapter_snapshot: Path, output_dir: Path) -> None:
    from peft import PeftModel
    from transformers import AutoModel, AutoTokenizer

    tokenizer = AutoTokenizer.from_pretrained(adapter_snapshot)
    base_model = AutoModel.from_pretrained(BASE_MODEL)
    base_model.resize_token_embeddings(len(tokenizer))
    merged = PeftModel.from_pretrained(base_model, adapter_snapshot).merge_and_unload()
    merged.save_pretrained(output_dir, safe_serialization=True)
    tokenizer.save_pretrained(output_dir)


def copy_required(snapshot: Path, output_dir: Path) -> None:
    for name in ["tokenizer.json", "tokenizer_config.json", "special_tokens_map.json"]:
        copy_one(snapshot / name, output_dir / name)
    sentencepiece = snapshot / "sentencepiece.bpe.model"
    if sentencepiece.exists():
        copy_one(sentencepiece, output_dir / "sentencepiece.bpe.model")

    projection = next((snapshot / name for name in PROJECTION_CANDIDATES if (snapshot / name).exists()), None)
    if projection is None:
        available = ", ".join(sorted(str(path.relative_to(snapshot)) for path in snapshot.glob("**/*.safetensors")))
        raise FileNotFoundError(
            "Could not find a projection safetensors file. "
            f"Tried {PROJECTION_CANDIDATES}; available: {available}"
        )
    copy_one(projection, output_dir / "colbert_projection.safetensors")

    missing = [name for name in REQUIRED if not (output_dir / name).exists()]
    if missing:
        raise FileNotFoundError(f"Missing staged THIRAWAT files: {', '.join(missing)}")


def copy_one(src: Path, dst: Path) -> None:
    if not src.exists():
        raise FileNotFoundError(src)
    if src.resolve() != dst.resolve():
        dst.write_bytes(src.read_bytes())


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


if __name__ == "__main__":
    main()
