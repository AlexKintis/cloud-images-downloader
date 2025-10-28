pub mod fzf_invoker;
pub mod image_resolver;

use self::fzf_invoker::FzfInvoker;
use anyhow::Result;
use anyhow::bail;

pub fn choose_one<S: ToString>(title: &str, items: Vec<S>) -> Result<String> {
    let display_items: Vec<String> = items.into_iter().map(|s| s.to_string()).collect();
    let picker = FzfInvoker::new(title.to_string(), display_items);
    if let Some(choice) = picker.invoke() {
        Ok(choice)
    } else {
        bail!("No selection made");
    }
}

/// Return reasonable arch options per distro
pub fn arch_options_for(distro: &str) -> Vec<&'static str> {
    match distro {
        // You can widen these as your indexers evolve
        "Ubuntu" => vec!["amd64", "arm64", "ppc64el", "s390x"],
        "Debian" => vec!["amd64", "arm64"], // TODO(debian): confirm available arches from debian_list(...)
        "AlmaLinux" => vec!["x86_64", "aarch64"], // TODO(almalinux): confirm from almalinux_list(...)
        _ => vec!["amd64"],
    }
}
