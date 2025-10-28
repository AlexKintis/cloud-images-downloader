// you need this in your cargo.toml
// reqwest = { version = "0.11.3", features = ["stream"] }
// futures-util = "0.3.14"
// indicatif = "0.15.0"
use std::cmp::min;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use indicatif::{ProgressBar, ProgressStyle};

pub async fn download_file(url: &str) -> Result<String, String> {
    // HTTP client
    let client = reqwest::Client::new();

    // Request
    let mut res = client
        .get(url)
        .header("User-Agent", "cloud-index-reader-rust/1.0")
        .send()
        .await
        .map_err(|e| format!("Failed to GET from '{url}': {e}"))?;

    let total_size = res.content_length().ok_or_else(|| format!("Failed to get content length from '{url}'"))?;

    // Progress bar
    let pb = ProgressBar::new(total_size);
    let style = ProgressStyle::with_template(
        "{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] \
         {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
    )
    .map_err(|e| format!("Failed to build progress style: {e}"))?
    .progress_chars("#>-");
    pb.set_style(style);
    pb.set_message(format!("Downloading {url}"));

    // Output path: current directory + filename from the URL (fallback: "download")
    let mut out_path: PathBuf = std::env::current_dir().map_err(|e| format!("Failed to get current dir: {e}"))?;
    let filename = url.rsplit('/').find(|s| !s.is_empty()).unwrap_or("download");
    out_path.push(filename);

    // Download chunks (use chunk() to avoid bytes_stream() feature issues)
    let mut file = File::create(&out_path).map_err(|e| format!("Failed to create file '{}': {e}", out_path.display()))?;
    let mut downloaded: u64 = 0;

    while let Some(chunk) = res.chunk().await.map_err(|e| format!("Error while downloading file: {e}"))? {
        file.write_all(&chunk).map_err(|e| format!("Error while writing to file: {e}"))?;

        let new = min(downloaded + chunk.len() as u64, total_size);
        downloaded = new;
        pb.set_position(new);
    }

    let finish_download_message = format!("Downloaded {url} to {}", out_path.display());

    pb.finish_with_message(finish_download_message.clone());

    Ok(finish_download_message.clone())
}
