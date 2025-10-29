mod cloud;
mod helpers;
mod repositories;

use anyhow::{Result, bail};
use std::{env, path::PathBuf};

use helpers::{choose_one, image_resolver::download_file};
use repositories::{self as repos, almalinux, debian, ubuntu};

use cloud::Image;

/// Resolve the absolute path to the bundled `indexes.json` file that contains
/// the repository metadata used by the pickers.
fn construct_properties_file_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("indexes.json")
}

/// A tiny wrapper to render the final selection cleanly
fn print_selection(distro: &str, arch: &str, version: &str, image: &Image) {
    // If your Image implements getters, use them here
    println!("\n=== Selection ===");
    println!("Distro:   {distro}");
    println!("Arch:     {arch}");
    println!("Version:  {version}");
    println!("Image:");
    println!("  name:        {}", image.name());
    println!("  distro ver:  {}", image.distro_version());
    println!("  version:     {}", image.version());
    println!("  type:        {}", image.image_type());
    println!("  arch:        {}", image.arch());
    println!("  url:         {}", image.url());
    if let Some(checksum) = image.checksum() {
        println!("  checksum:    {} ({})", checksum.value(), checksum.kind());
    } else {
        println!("  checksum:    <none>");
    }
}

/// Full 3-step wizard: distro -> arch -> version -> image
/// Ask the user to progressively narrow down their choice and return the final
/// image selection.
///
/// The function keeps the prompts generic so they can be reused for the
/// different distros supported by the tool while still returning a uniform
/// structure that the caller can work with.
async fn prompt_and_select(track: &str) -> Result<(String, String, String, Image)> {
    // 0) Distro
    let distro = choose_one("Select Distro", vec!["Ubuntu", "Debian", "AlmaLinux"])?;

    match distro.as_str() {
        "Ubuntu" => {
            // pick_ubuntu also asks for arch + version internally
            let img = ubuntu::pick_ubuntu(track).await?;
            let arch = img.arch().to_string();
            let version = img.version().to_string();
            Ok((distro, arch, version, img))
        }
        "Debian" => {
            let (codename, img) = debian::pick_debian_interactive().await?;
            let arch = img.arch().to_string();
            let version = format!("{codename} ({})", img.version());
            Ok((distro, arch, version, img))
        }
        "AlmaLinux" => {
            let img = almalinux::pick_almalinux(track).await?;
            let arch = img.arch().to_string();
            let version = img.version().to_string();
            Ok((distro, arch, version, img))
        }
        _ => bail!("Unsupported distro '{distro}'",),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let path = construct_properties_file_path();
    repos::init_from_file(&path)?; // stays sync

    // Get repos info from json by name
    // let repo = repos::by_name("ubuntu").unwrap();

    // You can toggle "daily" here if you want (already in your comments)
    let track = "releases";

    let (distro, arch, version, image) = prompt_and_select(track).await?;

    println!("{image:?}");

    // Print the chosen structure (clean summary)
    print_selection(&distro, &arch, &version, &image);

    let output = download_file(image.url()).await;

    match output {
        Ok(msg) => println!("{msg}"),
        Err(err) => eprintln!("{err}"),
    }

    Ok(())
}
