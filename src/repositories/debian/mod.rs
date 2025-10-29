pub mod models;
pub use models::{DebianProvider, ImageAsset, ImageRequest, Provider};

use anyhow::{Context, Result, anyhow, ensure};
use regex::Regex;
use reqwest::Client;
use sha2::{Digest, Sha512};
use std::fs::write;

use crate::cloud::{ChecksumKind, Image, ImageChecksum};
use crate::helpers::{arch_options_for, choose_one};
use crate::repositories;

const DEFAULT_CODENAMES: &[&str] = &["stable", "bookworm", "trixie"];

pub fn codename_options() -> Vec<&'static str> {
    DEFAULT_CODENAMES.to_vec()
}

pub async fn available_codenames() -> Result<Vec<String>> {
    let client = Client::new();
    let root = repository_root()?;

    let html = client
        .get(&root)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await
        .with_context(|| format!("fetch Debian codename listing from {root}"))?;

    let dir_re = Regex::new(r#"href=\"([a-z0-9][a-z0-9-]+)/\""#)?;
    let mut names: Vec<String> = dir_re.captures_iter(&html).map(|cap| cap[1].to_string()).collect();

    names.sort();
    names.dedup();

    if names.is_empty() {
        return Ok(DEFAULT_CODENAMES.iter().map(|s| s.to_string()).collect());
    }

    Ok(names)
}

pub async fn prompt_for_codename() -> Result<String> {
    let dynamic = available_codenames().await.unwrap_or_default();
    let options = if dynamic.is_empty() {
        DEFAULT_CODENAMES.iter().map(|s| s.to_string()).collect::<Vec<_>>()
    } else {
        dynamic
    };

    choose_one("Select Debian Codename", options)
}

pub async fn pick_debian_interactive() -> Result<(String, Image)> {
    let codename = prompt_for_codename().await?;
    let image = pick_debian(&codename).await?;
    Ok((codename, image))
}

struct DebianRepoUrls {
    latest: String,
    listing_root: String,
}

fn repository_template() -> Result<(String, String)> {
    let repo = repositories::by_name("debian")
        .map_err(anyhow::Error::new)?
        .context("repository 'debian' is not configured")?;

    repo.url()
        .split_once("{}")
        .map(|(prefix, suffix)| (prefix.to_string(), suffix.to_string()))
        .ok_or_else(|| anyhow!("repository URL for debian must contain '{{}}' placeholder"))
}

fn repository_root() -> Result<String> {
    let (prefix, _) = repository_template()?;
    Ok(if prefix.ends_with('/') { prefix } else { format!("{prefix}/") })
}

fn repository_urls(codename: &str) -> Result<DebianRepoUrls> {
    let (prefix, suffix) = repository_template()?;

    let mut latest = format!("{prefix}{codename}{suffix}");
    if !latest.ends_with('/') {
        latest.push('/');
    }

    let listing_root = if let Some(stripped) = latest.strip_suffix("latest/") {
        stripped.to_string()
    } else {
        latest.clone()
    };

    let listing_root = if listing_root.ends_with('/') {
        listing_root
    } else {
        format!("{listing_root}/")
    };

    Ok(DebianRepoUrls { latest, listing_root })
}

pub async fn pick_debian(codename: &str) -> Result<Image> {
    // 1) Arch (use your existing helper; ensure it includes amd64/arm64 at least)
    let arch = choose_one("Select Architecture", arch_options_for("Debian"))?;

    // 2) Fetch images for the chosen arch (treat `codename` like "bookworm", "trixie", or "stable")
    let mut images: Vec<Image> = debian_list(codename, &arch, /*include_testing=*/ false)
        .await
        .with_context(|| format!("fetch debian images for codename='{codename}' arch='{arch}'"))?;

    ensure!(!images.is_empty(), "No Debian images found for codename={codename} arch={arch}");

    // 3) Distro major version (e.g., "12", "13")
    let mut distro_versions = images.iter().map(|i| i.distro_version().to_string()).collect::<Vec<_>>();
    distro_versions.sort();
    distro_versions.reverse();
    distro_versions.dedup();

    let distro_version = choose_one("Select Distro Version", distro_versions)?;
    images = images.into_iter().filter(|i| i.distro_version() == distro_version).collect();
    ensure!(!images.is_empty(), "No Debian images found for distro_version={distro_version}");

    // 4) Image version / build (e.g., point release or date-stamped build)
    let mut image_versions: Vec<String> = images.iter().map(|i| i.version().to_string()).collect::<Vec<_>>();
    image_versions.sort();
    image_versions.reverse();
    image_versions.dedup();

    let image_version = choose_one("Select Image Version", image_versions)?;
    images = images.into_iter().filter(|i| i.version() == image_version).collect();
    ensure!(
        !images.is_empty(),
        "No Debian images for distro_version={distro_version} and version={image_version}"
    );

    // 5) Image type / variant (Debian usually: "genericcloud" or "nocloud")
    let mut image_types: Vec<String> = images.iter().map(|i| i.image_type().to_string()).collect();
    image_types.sort();
    image_types.dedup();

    let image_type = choose_one("Select image type (variant)", image_types)?;
    images = images.into_iter().filter(|i| i.image_type() == image_type).collect();
    ensure!(
        !images.is_empty(),
        "No Debian images found for distro_version={distro_version}, version={image_version}, type={image_type}"
    );

    // 6) If multiple artifacts remain (qcow2/raw), let user pick the exact one
    let labelize = |i: &Image| format!("{} | {} | {} | {} | {}", i.name(), i.image_type(), i.version(), i.arch(), i.url());
    let chosen_label = choose_one("Select Image Artifact", images.iter().map(|i| labelize(i)).collect())?;

    let idx = images
        .iter()
        .position(|i| labelize(i) == chosen_label)
        .expect("selected label must match one candidate");

    Ok(images[idx].clone())
}

fn make_image(
    codename: &str,
    url: String,
    arch: String,
    image_type: String,
    version: String,
    distro_version: String,
    checksum: Option<ImageChecksum>,
) -> Image {
    Image::from_parts(
        "debian".to_string(),
        codename.to_string(),
        distro_version,
        version,
        arch,
        url,
        checksum,
        image_type,
    )
}

/// List Debian cloud images for a given codename & arch.
///
/// - `codename`: "bookworm", "trixie", or "stable" (etc)
/// - `arch`: "amd64" | "arm64" (accepts "x86_64" and normalizes to "amd64")
/// - `include_testing`: currently unused (kept for API symmetry)
pub async fn debian_list(codename: &str, arch: &str, _include_testing: bool) -> Result<Vec<Image>> {
    let client = Client::new();

    // Debian calls x86_64 -> amd64
    let want_arch = match arch {
        "x86_64" => "amd64",
        other => other,
    }
    .to_string();

    let repo_urls = repository_urls(codename)?;
    let base = repo_urls.listing_root;

    // 1) Fetch directory index and extract subdirs: latest/ and YYYYMMDD-HHMM/
    let index_html = client
        .get(&base)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await
        .with_context(|| format!("fetch directory listing: {base}"))?;

    let dir_re = Regex::new(r#"href="((?:latest|\d{8}-\d{4}))/""#)?;
    let mut dirs: Vec<String> = dir_re.captures_iter(&index_html).map(|c| c[1].to_string()).collect();

    // Dedup + keep latest first for nice UX (latest, then most recent builds)
    dirs.sort();
    dirs.dedup();
    // Move "latest" to front if present
    if let Some(pos) = dirs.iter().position(|d| d == "latest") {
        let latest = dirs.remove(pos);
        dirs.insert(0, latest);
    }
    // Reverse to have newest date dirs first (after "latest")
    // (If "latest" exists, it's already at index 0.)
    if dirs.len() > 1 {
        let (head, tail) = dirs.split_at_mut(1);
        tail.reverse();
        // head (latest) + reversed tail
        dirs = head.iter().cloned().chain(tail.iter().cloned()).collect();
    }

    // 2) For each subdir, read SHA512SUMS and parse artifacts
    // Filenames look like:
    //   debian-12-genericcloud-amd64.qcow2
    //   debian-12-nocloud-amd64.qcow2
    // We’ll capture:
    //   distro_version = 12
    //   image_type     = genericcloud|nocloud
    //   arch           = amd64|arm64
    //   ext            = qcow2|raw (you can keep/filter later)
    //
    // SHA512SUMS lines are typically:
    //   <sha256>  debian-12-genericcloud-amd64.qcow2
    //
    let line_re = Regex::new(
        r#"(?i)^(?P<sha>(?:[a-f0-9]{64}|[a-f0-9]{128}))\s+\*?(?P<file>debian-(?P<dver>\d+)-(?P<variant>[a-z0-9]+)-(?P<arch>amd64|arm64)\.(?P<ext>qcow2|raw))$"#,
    )?;

    let mut out = Vec::new();

    for d in dirs {
        let sums_url = format!("{base}{d}/SHA512SUMS");
        let sums = match client.get(&sums_url).send().await {
            Ok(resp) => match resp.error_for_status() {
                Ok(ok) => ok.text().await.unwrap_or_default(),
                Err(_) => continue, // no SHA512SUMS in this dir; skip
            },
            Err(_) => continue,
        };

        for line in sums.lines() {
            if let Some(c) = line_re.captures(line.trim()) {
                let file_arch = c.name("arch").unwrap().as_str();
                if file_arch != want_arch {
                    continue;
                }

                let filename = c.name("file").unwrap().as_str().to_string();
                let distro_version = c.name("dver").unwrap().as_str().to_string();
                let variant = c.name("variant").unwrap().as_str().to_string();
                let checksum = c.name("sha").map(|cap| ImageChecksum::new(ChecksumKind::Sha512, cap.as_str()));

                // You can choose to filter by ext here if you only want qcow2:
                // let ext = c.name("ext").unwrap().as_str();
                // if ext != "qcow2" { continue; }

                let url = format!("{base}{d}/{filename}");

                // "version" in your picker is the build dir (e.g., "latest" or "20241013-1744")
                // "image_type" is the Debian variant (e.g., "genericcloud", "nocloud")
                out.push(make_image(codename, url, want_arch.clone(), variant, d.clone(), distro_version, checksum));
            }
        }
    }

    Ok(out)
}
