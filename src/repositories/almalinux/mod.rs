use std::sync::OnceLock;

use anyhow::{Context, Result};
use anyhow::{bail, ensure};
use regex::Regex;
use reqwest::Client;

use crate::cloud::{ChecksumKind, Image, ImageChecksum};
use crate::helpers::{arch_options_for, choose_one};
use crate::repositories;

const DEFAULT_MAJORS: &[&str] = &["9", "8"];
const CHECKSUM_FILENAME: &str = "CHECKSUM";

fn checksum_line_regex() -> &'static Regex {
    static LINE_RE: OnceLock<Regex> = OnceLock::new();
    LINE_RE.get_or_init(|| {
        Regex::new(r"^SHA256 \((?P<file>AlmaLinux-[^)]+)\) = (?P<sha>[A-Fa-f0-9]{64})$")
            .expect("invalid AlmaLinux checksum line regex")
    })
}

fn filename_regex() -> &'static Regex {
    static FILE_RE: OnceLock<Regex> = OnceLock::new();
    FILE_RE.get_or_init(|| {
        Regex::new(
            r"^AlmaLinux-(?P<major>\d+)-(?P<variant>[A-Za-z0-9-]+)-(?P<version>[A-Za-z0-9.-]+)\.(?P<arch>[A-Za-z0-9_]+)\.(?P<ext>.+)$",
        )
        .expect("invalid AlmaLinux artifact filename regex")
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AlmaArtifact {
    filename: String,
    major: String,
    variant: String,
    distro_version: String,
    image_version: String,
    arch: String,
    format: String,
}

fn split_version_parts(version_fragment: &str, major: &str) -> (String, String) {
    if version_fragment.eq_ignore_ascii_case("latest") {
        return (major.to_string(), "latest".to_string());
    }

    if let Some((release, build)) = version_fragment.rsplit_once('-') {
        let release = release.trim().to_string();
        let build = build.trim().to_string();
        if !release.is_empty() && !build.is_empty() {
            return (release, build);
        }
    }

    (major.to_string(), version_fragment.to_string())
}

fn parse_artifact_filename(filename: &str, expected_arch: &str) -> Option<AlmaArtifact> {
    let caps = filename_regex().captures(filename)?;

    let arch = caps.name("arch")?.as_str();
    if !arch.eq_ignore_ascii_case(expected_arch) {
        return None;
    }

    let major = caps.name("major")?.as_str().to_string();
    let variant = caps.name("variant")?.as_str().to_string();
    let version_fragment = caps.name("version")?.as_str();
    let (distro_version, image_version) = split_version_parts(version_fragment, &major);
    let format = caps.name("ext")?.as_str().to_string();

    let format_lower = format.to_ascii_lowercase();
    if format_lower.contains("checksum") || format_lower.ends_with(".sig") {
        return None;
    }

    Some(AlmaArtifact {
        filename: filename.to_string(),
        major,
        variant,
        distro_version,
        image_version,
        arch: arch.to_string(),
        format,
    })
}

fn repository_config() -> Result<&'static repositories::Repository> {
    repositories::by_name("almalinux")
        .map_err(anyhow::Error::new)?
        .context("repository 'almalinux' is not configured")
}

fn repository_base_url(major: &str, arch: &str) -> Result<String> {
    let repo = repository_config()?;
    let template = repo.url();

    let replaced_major = template.replacen("{}", major, 1);
    ensure!(
        replaced_major.contains("{}"),
        "repository URL for almalinux must contain two '{{}}' placeholders"
    );

    let replaced_arch = replaced_major.replacen("{}", arch, 1);

    Ok(if replaced_arch.ends_with('/') {
        replaced_arch
    } else {
        format!("{replaced_arch}/")
    })
}

fn majors_root_url() -> Result<String> {
    let repo = repository_config()?;
    if let Some(params) = repo.other_parameters() {
        if let Some(root) = params.get("majors_root") {
            return Ok(root.clone());
        }
    }

    let template = repo.url();
    if let Some((prefix, _)) = template.split_once("{}") {
        return Ok(prefix.to_string());
    }

    bail!("unable to determine AlmaLinux majors root from repository config")
}

async fn fetch_major_versions() -> Result<Vec<String>> {
    let root = majors_root_url()?;
    let client = Client::new();

    let html = client
        .get(&root)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await
        .with_context(|| format!("fetch AlmaLinux directory listing from {root}"))?;

    let dir_re = Regex::new(r#"href=\"(\d{1,2})/\""#)?;
    let mut majors: Vec<String> = dir_re
        .captures_iter(&html)
        .map(|cap| cap[1].to_string())
        .collect();

    majors.sort();
    majors.dedup();
    majors.reverse();

    Ok(majors)
}

pub async fn available_majors() -> Result<Vec<String>> {
    match fetch_major_versions().await {
        Ok(list) if !list.is_empty() => Ok(list),
        _ => Ok(DEFAULT_MAJORS.iter().map(|s| s.to_string()).collect()),
    }
}

fn make_image(base_url: &str, artifact: AlmaArtifact, checksum: ImageChecksum) -> Image {
    let url = format!("{base_url}{}", artifact.filename);
    Image::from_parts(
        "almalinux".to_string(),
        artifact.variant,
        artifact.distro_version,
        artifact.image_version,
        artifact.arch,
        url,
        Some(checksum),
        artifact.format,
    )
}

pub async fn almalinux_list(major: &str, arch: &str) -> Result<Vec<Image>> {
    let base = repository_base_url(major, arch)?;
    let checksum_url = format!("{base}{CHECKSUM_FILENAME}");
    let client = Client::new();

    let checksum_body = client
        .get(&checksum_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await
        .with_context(|| format!("fetch AlmaLinux checksum list from {checksum_url}"))?;

    let mut images = Vec::new();
    for line in checksum_body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let caps = match checksum_line_regex().captures(trimmed) {
            Some(caps) => caps,
            None => continue,
        };

        let filename = caps.name("file").unwrap().as_str();
        let sha = caps.name("sha").unwrap().as_str();

        if let Some(artifact) = parse_artifact_filename(filename, arch) {
            let checksum = ImageChecksum::new(ChecksumKind::Sha256, sha);
            images.push(make_image(&base, artifact, checksum));
        }
    }

    images.sort_by(|a, b| {
        b.distro_version()
            .cmp(a.distro_version())
            .then_with(|| b.version().cmp(a.version()))
            .then_with(|| a.name().cmp(b.name()))
            .then_with(|| a.image_type().cmp(b.image_type()))
    });

    Ok(images)
}

pub async fn pick_almalinux(_track: &str) -> Result<Image> {
    let arch = choose_one("Select Architecture", arch_options_for("AlmaLinux"))?;

    let majors = available_majors().await?;
    ensure!(!majors.is_empty(), "No AlmaLinux major versions available");
    let major = choose_one("Select AlmaLinux Major Version", majors)?;

    let mut images = almalinux_list(&major, &arch).await?;
    ensure!(
        !images.is_empty(),
        "No AlmaLinux images found for major={major} arch={arch}"
    );

    let mut distro_versions: Vec<String> = images
        .iter()
        .map(|i| i.distro_version().to_string())
        .collect();
    distro_versions.sort();
    distro_versions.reverse();
    distro_versions.dedup();

    let distro_version = choose_one("Select Distro Version", distro_versions)?;
    images.retain(|i| i.distro_version() == distro_version);
    ensure!(
        !images.is_empty(),
        "No AlmaLinux images found for distro_version={distro_version}"
    );

    let mut image_versions: Vec<String> = images.iter().map(|i| i.version().to_string()).collect();
    image_versions.sort();
    image_versions.reverse();
    image_versions.dedup();

    let image_version = choose_one("Select Image Version", image_versions)?;
    images.retain(|i| i.version() == image_version);
    ensure!(
        !images.is_empty(),
        "No AlmaLinux images found for distro_version={distro_version} version={image_version}"
    );

    let mut variants: Vec<String> = images.iter().map(|i| i.name().to_string()).collect();
    variants.sort();
    variants.dedup();

    let variant = choose_one("Select Image Variant", variants)?;
    images.retain(|i| i.name() == variant);
    ensure!(
        !images.is_empty(),
        "No AlmaLinux images found for distro_version={distro_version}, version={image_version}, variant={variant}"
    );

    let mut formats: Vec<String> = images.iter().map(|i| i.image_type().to_string()).collect();
    formats.sort();
    formats.dedup();

    let format = choose_one("Select Image Format", formats)?;
    images.retain(|i| i.image_type() == format);
    ensure!(
        !images.is_empty(),
        "No AlmaLinux images found for distro_version={distro_version}, version={image_version}, variant={variant}, format={format}"
    );

    let labelize = |i: &Image| {
        format!(
            "{} | {} | {} | {} | {}",
            i.name(),
            i.image_type(),
            i.version(),
            i.arch(),
            i.url()
        )
    };

    let chosen_label = choose_one(
        "Select Image Artifact",
        images.iter().map(|i| labelize(i)).collect(),
    )?;

    let idx = images
        .iter()
        .position(|i| labelize(i) == chosen_label)
        .expect("selected label must match one candidate");

    Ok(images[idx].clone())
}

#[cfg(test)]
mod tests {
    use super::{AlmaArtifact, parse_artifact_filename, split_version_parts};

    #[test]
    fn split_version_with_latest() {
        let (distro, version) = split_version_parts("latest", "9");
        assert_eq!(distro, "9");
        assert_eq!(version, "latest");
    }

    #[test]
    fn split_version_with_release_and_build() {
        let (distro, version) = split_version_parts("9.4-20240513", "9");
        assert_eq!(distro, "9.4");
        assert_eq!(version, "20240513");
    }

    #[test]
    fn split_version_without_delimiter() {
        let (distro, version) = split_version_parts("20240513", "9");
        assert_eq!(distro, "9");
        assert_eq!(version, "20240513");
    }

    #[test]
    fn parse_valid_artifact_filename() {
        let artifact = parse_artifact_filename(
            "AlmaLinux-9-GenericCloud-9.4-20240513.x86_64.qcow2",
            "x86_64",
        )
        .expect("expected artifact to parse");

        assert_eq!(
            artifact,
            AlmaArtifact {
                filename: "AlmaLinux-9-GenericCloud-9.4-20240513.x86_64.qcow2".to_string(),
                major: "9".to_string(),
                variant: "GenericCloud".to_string(),
                distro_version: "9.4".to_string(),
                image_version: "20240513".to_string(),
                arch: "x86_64".to_string(),
                format: "qcow2".to_string(),
            }
        );
    }

    #[test]
    fn parse_skips_other_architectures() {
        assert!(
            parse_artifact_filename(
                "AlmaLinux-9-GenericCloud-9.4-20240513.aarch64.qcow2",
                "x86_64"
            )
            .is_none()
        );
    }

    #[test]
    fn parse_skips_checksum_artifacts() {
        assert!(
            parse_artifact_filename(
                "AlmaLinux-9-GenericCloud-9.4-20240513.x86_64.qcow2.CHECKSUM",
                "x86_64"
            )
            .is_none()
        );
    }
}
