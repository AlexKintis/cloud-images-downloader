use std::path::{Path, PathBuf};

pub use crate::cloud::{Catalog, Image};
use crate::helpers::{arch_options_for, choose_one};
use crate::repositories;

use anyhow::{Context, Result, bail, ensure};
use reqwest::Client;
use std::fs;
use std::io::Write;

/// Build a human readable label for the picker so users can distinguish very
/// similar images at a glance.
fn format_image_label(image: &Image) -> String {
    format!("{} | {} | {}", image.name(), image.arch(), image.url())
}

/// Picking ubuntu
pub async fn pick_ubuntu(track: &str) -> Result<Image> {
    // 1) Arch
    let arch = choose_one("Select Architecture", arch_options_for("Ubuntu"))?;

    // 2) Fetch images for the chosen arch
    let mut images: Vec<Image> = ubuntu_list(track, &arch, false)
        .await
        .with_context(|| format!("fetch ubuntu images for track='{track}' arch='{arch}'"))?;

    ensure!(!images.is_empty(), "No Ubuntu images found for arch={arch}");

    // 3) Distro version (filter the working set after selection)
    let mut distro_versions = images
        .iter()
        .map(|i| i.distro_version().to_string())
        .collect::<Vec<_>>();
    distro_versions.sort();
    distro_versions.reverse();
    distro_versions.dedup();

    let distro_version = choose_one("Select Distro Version", distro_versions)?;
    images.retain(|i| i.distro_version() == distro_version);
    ensure!(
        !images.is_empty(),
        "No Ubuntu images found for distro_version={distro_version}"
    );

    // 4) Image version (filter again after selection)
    let mut image_versions: Vec<String> = images
        .iter()
        .map(|i| i.version().to_string())
        .collect::<Vec<_>>();
    image_versions.sort();
    image_versions.reverse();
    image_versions.dedup();

    let image_version = choose_one("Select Image Version", image_versions)?;
    images.retain(|i| i.version() == image_version);
    ensure!(
        !images.is_empty(),
        "No Ubuntu images found for distro_version={distro_version} and version={image_version}"
    );

    // 5) Pick image type (now uses the model's image_type; filter again after selection)
    let mut image_types: Vec<String> = images.iter().map(|i| i.image_type().to_string()).collect();
    image_types.sort();
    image_types.dedup();

    let image_type = choose_one("Select image type", image_types)?;
    images.retain(|i| i.image_type() == image_type);
    ensure!(
        !images.is_empty(),
        "No Ubuntu images found for distro_version={distro_version}, version={image_version}, type={image_type}"
    );

    // 6) If a version maps to multiple artifacts, let the user pick one (now the working set is already scoped)
    let chosen_label = choose_one(
        "Select Image Artifact",
        images.iter().map(format_image_label).collect(),
    )?;

    // Find back the chosen image
    let idx = images
        .iter()
        .position(|i| format_image_label(i) == chosen_label)
        .expect("selected label must match one candidate");

    Ok(images[idx].clone())
}

/// Download the JSON at `url` into `dest_path` inside the temp folder.
/// Returns the full path of the saved file.
/// Download the remote Simplestreams document into a deterministic location so
/// future runs can reuse the cached copy.
async fn fetch_repo_json_file_to_tmp(url: &str, dest_path: &Path) -> Result<PathBuf> {
    let client = Client::builder().build()?;

    let res = client
        .get(url)
        .header("User-Agent", "cloud-index-reader-rust/1.0")
        .send()
        .await
        .with_context(|| format!("GET {}", url))?;

    let status = res.status();
    if !status.is_success() {
        bail!("HTTP {} for {}", status, url);
    }

    let bytes = res
        .bytes()
        .await
        .with_context(|| format!("read body from {}", url))?;

    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;
    }

    // Write atomically: write to a tmp file then rename.
    let tmp = dest_path.with_extension("download");
    let mut file =
        fs::File::create(&tmp).with_context(|| format!("create file {}", tmp.display()))?;
    file.write_all(&bytes)
        .with_context(|| format!("write file {}", tmp.display()))?;
    drop(file);

    fs::rename(&tmp, dest_path)
        .with_context(|| format!("move {} -> {}", tmp.display(), dest_path.display()))?;

    Ok(dest_path.to_path_buf())
}

/// Build a catalogue by reading JSON either from a cached temp file (if it exists)
/// or by downloading it once and caching it. Deserializes into `T`.
async fn construct_repo_catalogue<T: for<'de> serde::Deserialize<'de>>(url: &str) -> Result<T> {
    // Decide the filename from the URL (fallback to "repo.json")
    let file_name = url
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("repo.json");

    // Get json file from temp folder
    let mut tmp_path: PathBuf = std::env::temp_dir();
    tmp_path.push(file_name);

    // If file does not exist, download it to tmp first
    if !tmp_path.exists() {
        match fetch_repo_json_file_to_tmp(url, &tmp_path).await {
            Ok(file) => {
                println!("Repo file successfully downloaded to {}", file.display());
            }
            Err(err) => {
                // Fail fast as in your intent
                panic!("Repo file did not download into the temp folder: {err}");
            }
        }
    }

    // Read from the cached file and deserialize
    let bytes =
        fs::read(&tmp_path).with_context(|| format!("read cached file {}", tmp_path.display()))?;

    let data: T = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse JSON from {}", tmp_path.display()))?;

    Ok(data)
}

/// Construct the repository url which contains the '{}' delimiter
///
/// The upstream configuration stores a template with placeholders for the
/// requested track (e.g. `releases` or `daily`). This helper replaces the first
/// placeholder while leaving the rest untouched for downstream consumers.
fn construct_repo_url(track: &str) -> String {
    let catalog_url: String = repositories::by_name("ubuntu")
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap()
        .url()
        .to_string();
    catalog_url.replacen("{}", track, 1)
}

/// Fetch a normalized list of Ubuntu images from Canonical Simplestreams.
/// - `track`: "releases" (stable) or "daily"
/// - `arch`: "amd64", "arm64", "ppc64el", "s390x"
/// - `only_disk_images`: if true, keep only `.img` and `.qcow2`
pub async fn ubuntu_list(
    release_track: &str,
    target_arch: &str,
    only_disk_images: bool,
) -> Result<Vec<Image>> {
    let repo_base_url_for_paths: String = repositories::by_name("ubuntu")
        .unwrap_or_else(|err| panic!("{err}"))
        .unwrap()
        .other_parameters()
        .unwrap()
        .get("base_for_paths")
        .unwrap_or_else(|| panic!("Key in extra parameters not found!"))
        .clone();

    let base_url_for_paths = repo_base_url_for_paths.replacen("{}", release_track, 1);
    let catalog_url = construct_repo_url(release_track);

    let catalog: Catalog = construct_repo_catalogue(&catalog_url).await?;

    let mut images: Vec<Image> = Vec::new();

    for (product_name, product_metadata) in catalog.products() {
        let mut resolved_architecture = product_metadata.arch().clone();

        if resolved_architecture.is_none()
            && let Some(product_tail) = product_name.rsplit(':').next()
            && matches!(product_tail, "amd64" | "arm64" | "ppc64el" | "s390x")
        {
            resolved_architecture = Some(product_tail.to_string());
        }

        if let Some(ref detected_architecture) = resolved_architecture {
            if detected_architecture != target_arch {
                continue;
            }
        } else {
            continue; // no arch info
        }

        let release_name = product_metadata
            .release()
            .clone()
            .unwrap_or_else(|| "ubuntu".to_string());
        let distro_version = product_metadata
            .distro_version()
            .clone()
            .unwrap_or_else(|| "No distro version found".to_string());

        // ⬇️ capture the version id so we can pass the correct version
        for (version_id, version_metadata) in product_metadata.versions() {
            // ⬇️ capture the alias key and pass ftype to Image::from_metadata
            for (alias, image_item) in version_metadata.items() {
                let Some(relative_path) = image_item.path().clone() else {
                    continue;
                };

                if only_disk_images
                    && !(relative_path.ends_with(".img") || relative_path.ends_with(".qcow2"))
                {
                    continue;
                }

                images.push(Image::from_metadata(
                    product_metadata.os().unwrap(), // keep as-is per your code
                    &release_name,
                    &distro_version,
                    version_id, // <-- use version id from loop (not product_metadata.version())
                    resolved_architecture.as_ref().unwrap(),
                    &base_url_for_paths,
                    &relative_path,
                    image_item.sha256().clone(),
                    alias.to_string(),
                ));
            }
        }
    }

    Ok(images)
}
