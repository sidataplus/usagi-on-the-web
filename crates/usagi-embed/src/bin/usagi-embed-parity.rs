use std::path::PathBuf;

use usagi_embed::parity::{assert_cls_parity_files, assert_token_parity_files};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("cls") => run_cls(&args),
        Some("token") => run_token(&args),
        _ => {
            eprintln!(
                "usage: usagi-embed-parity cls <candidate.json> <reference.json> [min_cosine]\n       usagi-embed-parity token <candidate.json> <reference.json> [min_mean_cosine] [max_length] [output_dim]"
            );
            std::process::exit(2);
        }
    }
}

fn run_cls(args: &[String]) -> anyhow::Result<()> {
    let candidate = required_path(args, 2, "candidate.json")?;
    let reference = required_path(args, 3, "reference.json")?;
    let min_cosine = optional_f32(args, 4, 0.999)?;
    assert_cls_parity_files(&candidate, &reference, min_cosine)?;
    println!(
        "{}",
        serde_json::json!({
            "status": "ok",
            "mode": "cls",
            "min_cosine": min_cosine
        })
    );
    Ok(())
}

fn run_token(args: &[String]) -> anyhow::Result<()> {
    let candidate = required_path(args, 2, "candidate.json")?;
    let reference = required_path(args, 3, "reference.json")?;
    let min_mean_cosine = optional_f32(args, 4, 0.999)?;
    let max_length = optional_usize(args, 5, 96)?;
    let output_dim = optional_usize(args, 6, 128)?;
    assert_token_parity_files(
        &candidate,
        &reference,
        min_mean_cosine,
        max_length,
        output_dim,
    )?;
    println!(
        "{}",
        serde_json::json!({
            "status": "ok",
            "mode": "token",
            "min_mean_cosine": min_mean_cosine,
            "max_length": max_length,
            "output_dim": output_dim
        })
    );
    Ok(())
}

fn required_path(args: &[String], index: usize, label: &str) -> anyhow::Result<PathBuf> {
    args.get(index)
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("missing {label}"))
}

fn optional_f32(args: &[String], index: usize, default: f32) -> anyhow::Result<f32> {
    args.get(index)
        .map(|value| value.parse::<f32>())
        .transpose()
        .map_err(Into::into)
        .map(|value| value.unwrap_or(default))
}

fn optional_usize(args: &[String], index: usize, default: usize) -> anyhow::Result<usize> {
    args.get(index)
        .map(|value| value.parse::<usize>())
        .transpose()
        .map_err(Into::into)
        .map(|value| value.unwrap_or(default))
}
