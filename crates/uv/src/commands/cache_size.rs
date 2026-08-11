use std::fmt::Write;

use anstream::stream::IsTerminal;
use anyhow::Result;
#[cfg(not(target_family = "wasm"))]
use diskus::DiskUsage;

use crate::commands::{ExitStatus, human_readable_bytes};
use crate::printer::Printer;
use uv_cache::Cache;
use uv_cli::CacheSizeOutputFormat;
use uv_preview::{Preview, PreviewFeature};
use uv_warnings::warn_user;
use uv_vfs::VfsPathExt as _;

#[cfg(not(target_family = "wasm"))]
fn cache_total_bytes(root: &std::path::Path) -> u64 {
    DiskUsage::new(vec![root.to_path_buf()]).count_ignoring_errors()
}

#[cfg(target_family = "wasm")]
fn cache_total_bytes(root: &std::path::Path) -> u64 {
    uv_vfs::walk::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| entry.metadata().ok())
        .map(|metadata| metadata.len())
        .sum()
}

/// Display the total size of the cache.
pub(crate) fn cache_size(
    cache: &Cache,
    output_format: CacheSizeOutputFormat,
    printer: Printer,
    preview: Preview,
) -> Result<ExitStatus> {
    if !preview.is_enabled(PreviewFeature::CacheSize) {
        warn_user!(
            "`uv cache size` is experimental and may change without warning. Pass `--preview-features {}` to disable this warning.",
            PreviewFeature::CacheSize
        );
    }

    let human_readable = match output_format {
        CacheSizeOutputFormat::Auto => std::io::stdout().is_terminal(),
        CacheSizeOutputFormat::Human => true,
        CacheSizeOutputFormat::Machine => false,
    };

    if !cache.root().vfs_exists() {
        if human_readable {
            writeln!(printer.stdout_important(), "0B")?;
        } else {
            writeln!(printer.stdout_important(), "0")?;
        }
        return Ok(ExitStatus::Success);
    }

    let total_bytes = cache_total_bytes(cache.root());

    if human_readable {
        let (bytes, unit) = human_readable_bytes(total_bytes);
        writeln!(printer.stdout_important(), "{bytes:.1}{unit}")?;
    } else {
        writeln!(printer.stdout_important(), "{total_bytes}")?;
    }

    Ok(ExitStatus::Success)
}
