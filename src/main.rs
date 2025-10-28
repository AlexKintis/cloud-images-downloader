mod cloud;
mod debian;
mod helpers;
mod repositories;
mod ubuntu;
// TODO(debian): add `mod debian;` once debian_list(...) is implemented
// TODO(almalinux): add `mod almalinux;` once almalinux_list(...) is implemented

use anyhow::{Result, bail};
use std::{env, path::PathBuf};

//use debian::debian_select_image;
use helpers::{fzf_invoker::FzfInvoker, image_resolver::download_file};
use repositories::{self as repos};
use ubuntu::{Image, pick_ubuntu};

use crate::helpers::choose_one;

fn construct_properties_file_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join("indexes.json")
}

/// A tiny wrapper to render the final selection cleanly
fn print_selection(distro: &str, arch: &str, version: &str, image: &Image) {
    // If your Image implements getters, use them here
    println!("\n=== Selection ===");
    println!("Distro:   {distro}");
    println!("Arch:     {arch}");
    println!("Version:  {version}");
    println!("Image:");
    println!("  name:    {}", image.name());
    println!("  version: {}", image.version());
    println!("  arch:    {}", image.arch());
    println!("  url:     {}", image.url());
    println!("  sha256:  {}", image.sha256().unwrap_or("<none>"));
}

async fn pick_almalinux(_track: &str) -> Result<Image> {
    // TODO(almalinux): implement almalinux_list(track, arch, only_disk_images)
    bail!("AlmaLinux picker not yet implemented (TODO).")
}

/// NOTE: remove
async fn debian_select_image(_track: &str) -> Result<Image> {
    // TODO(almalinux): implement almalinux_list(track, arch, only_disk_images)
    bail!("Debian picker not yet implemented (TODO).")
}

/// Full 3-step wizard: distro -> arch -> version -> image
async fn prompt_and_select(track: &str) -> Result<(String, String, String, Image)> {
    // 0) Distro
    let distro = choose_one("Select Distro", vec!["Ubuntu", "Debian", "AlmaLinux"])?;

    match distro.as_str() {
        "Ubuntu" => {
            // pick_ubuntu also asks for arch + version internally
            let img = pick_ubuntu(track).await?;
            let arch = img.arch().to_string();
            let version = img.version().to_string();
            Ok((distro, arch, version, img))
        }
        "Debian" => {
            // When implemented, mirror Ubuntu flow
            let img = debian_select_image(track).await?;
            let arch = img.arch().to_string();
            let version = img.version().to_string();
            Ok((distro, arch, version, img))
        }
        "AlmaLinux" => {
            let img = pick_almalinux(track).await?;
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
