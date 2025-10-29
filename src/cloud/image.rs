use reqwest::Url;
use std::fmt;

/// Supported checksum algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumKind {
    Sha256,
    Sha512,
}

impl ChecksumKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChecksumKind::Sha256 => "sha256",
            ChecksumKind::Sha512 => "sha512",
        }
    }
}

impl fmt::Display for ChecksumKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Convenience wrapper that couples the checksum value with its algorithm.
#[derive(Debug, Clone)]
pub struct ImageChecksum {
    kind: ChecksumKind,
    value: String,
}

impl ImageChecksum {
    pub fn new(kind: ChecksumKind, value: impl Into<String>) -> Self {
        Self {
            kind,
            value: value.into(),
        }
    }

    pub fn kind(&self) -> ChecksumKind {
        self.kind
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Normalised representation of a cloud image, regardless of the upstream
/// repository format.
#[derive(Debug, Clone)]
pub struct Image {
    os: String,
    name: String,
    distro_version: String,
    version: String,
    arch: String,
    url: String,
    checksum: Option<ImageChecksum>,
    image_type: String,
}

#[allow(unused)]
impl Image {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        os: String,
        name: String,
        distro_version: String,
        version: String,
        arch: String,
        url: String,
        checksum: Option<ImageChecksum>,
        image_type: String,
    ) -> Self {
        Self {
            os,
            name,
            distro_version,
            version,
            arch,
            url,
            checksum,
            image_type,
        }
    }

    /// OS
    /// eg. ubuntu
    pub fn os(&self) -> &str {
        &self.os
    }

    /// Distro name:
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Version of the Distro
    /// eg. 24.04
    pub fn distro_version(&self) -> &str {
        &self.distro_version
    }

    /// Version of image
    /// 20251001
    pub fn version(&self) -> &str {
        &self.version
    }

    // Architecture
    // eg. amd64
    pub fn arch(&self) -> &str {
        &self.arch
    }

    // The url of the image
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn checksum(&self) -> Option<&ImageChecksum> {
        self.checksum.as_ref()
    }

    pub fn checksum_value(&self) -> Option<&str> {
        self.checksum.as_ref().map(|c| c.value())
    }

    pub fn checksum_kind(&self) -> Option<ChecksumKind> {
        self.checksum.as_ref().map(|c| c.kind())
    }

    /// Convenience for existing callers expecting SHA256 (returns `None` if the
    /// checksum is another algorithm).
    pub fn sha256(&self) -> Option<&str> {
        match self.checksum_kind() {
            Some(ChecksumKind::Sha256) => self.checksum_value(),
            _ => None,
        }
    }

    // Sha256 hash for image validation
    pub fn image_type(&self) -> &str {
        &self.image_type
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_metadata(
        os_name: String,
        release_name: &str,
        distro_version: &str,
        version: &str,
        architecture: &str,
        base_url: &str,
        relative_path: &str,
        sha256: Option<String>,
        image_type: String,
    ) -> Self {
        // Simplestreams metadata may expose multiple checksum types, but the
        // JSON files we consume currently only provide SHA256 values. Wrap the
        // optional string into the strongly typed helper so callers can
        // distinguish the hashing algorithm when more become available.
        let checksum = sha256.map(|value| ImageChecksum::new(ChecksumKind::Sha256, value));

        // Try to build an absolute URL, fallback to string concatenation
        let absolute_url = Url::parse(base_url)
            .and_then(|base| base.join(relative_path))
            .map(|u| u.into())
            .unwrap_or_else(|_| format!("{}{}", base_url, relative_path));

        Image::new(
            os_name,
            release_name.to_string(),
            distro_version.to_string(),
            version.to_string(),
            architecture.to_string(),
            absolute_url,
            checksum,
            image_type,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        os: String,
        name: String,
        distro_version: String,
        version: String,
        arch: String,
        url: String,
        checksum: Option<ImageChecksum>,
        image_type: String,
    ) -> Self {
        Image::new(
            os,
            name,
            distro_version,
            version,
            arch,
            url,
            checksum,
            image_type,
        )
    }
}
