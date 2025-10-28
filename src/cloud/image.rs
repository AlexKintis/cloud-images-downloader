use reqwest::Url;

#[derive(Debug, Clone)]
pub struct Image {
    os: String,
    name: String,
    distro_version: String,
    version: String,
    arch: String,
    url: String,
    sha256: Option<String>,
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
        sha256: Option<String>,
        image_type: String,
    ) -> Self {
        Self {
            os,
            name,
            distro_version,
            version,
            arch,
            url,
            sha256,
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

    // Sha256 hash for image validation
    pub fn sha256(&self) -> Option<&str> {
        self.sha256.as_deref()
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
            sha256,
            image_type,
        )
    }

    // TODO: implement
    //pub fn from_parts(&self, name: String, url, arch, image_type, version, distro_version) {

    //}
}
