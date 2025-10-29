# Cloud Images Downloader

Cloud Images Downloader is a Rust-based terminal utility for discovering and
fetching cloud-ready virtual machine images directly from the official
distribution indexes of Ubuntu, Debian, and AlmaLinux. The tool wraps the
available metadata in a friendly menu-driven workflow so you can search,
inspect, and download the exact image you need for KVM or other hypervisors
without leaving the terminal.

## Features

- **Interactive selection** – Navigate distro, architecture, release, and image
  choices through an `fzf`-style picker powered by [`termenu`](https://crates.io/crates/termenu).
- **Offline-friendly metadata** – Bundles a curated `resources/indexes.json`
  file so you can browse the available images even when you are temporarily
  offline; refresh it whenever you need new releases.
- **Progress reporting downloads** – Retrieve the selected image with
  [`reqwest`](https://crates.io/crates/reqwest) while an
  [`indicatif`](https://crates.io/crates/indicatif) progress bar keeps you up to
  date on transfer speed and completion.
- **Extensible architecture** – Repository-specific logic lives in
  `src/repositories`, making it straightforward to add more distributions or
  customize selection prompts.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.75+ (stable toolchain)
- Network access to the upstream cloud-image mirrors when downloading
- A UTF-8 capable terminal (for the interactive menu UI)

## Quick Start

```bash
# Clone the repository
$ git clone https://github.com/<your-org>/cloud-images-downloader.git
$ cd cloud-images-downloader

# Run the interactive downloader
$ cargo run
```

When the application starts it will guide you through three menus:

1. **Distribution** – choose between Ubuntu, Debian, or AlmaLinux.
2. **Architecture / Version** – narrow down the release track (e.g., `releases`
   vs. `daily`) and architecture (e.g., `amd64`, `arm64`).
3. **Image** – inspect the available builds and confirm the one you want.

After you confirm the final selection the program prints a summary, downloads
the image into your current directory, and displays the save path. If you cancel
any menu the run exits without side effects.

## Configuration

All metadata consumed by the pickers lives in [`resources/indexes.json`](./resources/indexes.json).
You can update this file manually or via your own automation to point to new
indexes, additional mirrors, or entirely new distributions. At startup the
application loads the file once and keeps it in memory for the rest of the
session.

## Troubleshooting

- **No menu appears or it closes immediately** – Ensure your terminal supports
  raw mode and that standard input/output are connected to a TTY.
- **Download fails with an HTTP error** – Verify that the URL referenced in
  `indexes.json` is publicly reachable and that you have network connectivity.
- **Checksum mismatch / validation needs** – The current downloader saves files
  without verifying checksums. If your workflow requires verification, extend
  `helpers::image_resolver` to compute and compare hashes before confirming
  success.

