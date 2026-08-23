use std::path::{Component, Path, PathBuf};

use percent_encoding::percent_decode_str;
use url::Url;

#[expect(
    clippy::result_unit_err,
    reason = "this mirrors url::Url::from_file_path, whose signature uv already depends on"
)]
pub trait UrlFilePathExt: Sized {
    fn from_file_path<P: AsRef<Path>>(path: P) -> Result<Self, ()>;

    fn from_directory_path<P: AsRef<Path>>(path: P) -> Result<Self, ()>;

    fn to_file_path(&self) -> Result<PathBuf, ()>;
}

impl UrlFilePathExt for Url {
    fn from_file_path<P: AsRef<Path>>(path: P) -> Result<Self, ()> {
        build(path.as_ref(), false)
    }

    fn from_directory_path<P: AsRef<Path>>(path: P) -> Result<Self, ()> {
        build(path.as_ref(), true)
    }

    fn to_file_path(&self) -> Result<PathBuf, ()> {
        if self.scheme() != "file" {
            return Err(());
        }
        if let Some(host) = self.host_str() {
            if !host.is_empty() && host != "localhost" {
                return Err(());
            }
        }

        let mut decoded = PathBuf::from("/");
        let Some(segments) = self.path_segments() else {
            return Err(());
        };
        for segment in segments {
            if segment.is_empty() {
                continue;
            }
            let Ok(text) = percent_decode_str(segment).decode_utf8() else {
                return Err(());
            };
            decoded.push(text.as_ref());
        }
        Ok(decoded)
    }
}

fn build(path: &Path, directory: bool) -> Result<Url, ()> {
    let normalized = crate::path::normalize(path);
    let Ok(mut url) = Url::parse("file:///") else {
        return Err(());
    };

    {
        let Ok(mut segments) = url.path_segments_mut() else {
            return Err(());
        };
        segments.clear();
        for component in normalized.components() {
            if let Component::Normal(segment) = component {
                let Some(text) = segment.to_str() else {
                    return Err(());
                };
                segments.push(text);
            }
        }
        if directory {
            segments.push("");
        }
    }

    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::UrlFilePathExt;
    use std::path::{Path, PathBuf};
    use url::Url;

    fn to_url(path: &str) -> Url {
        <Url as UrlFilePathExt>::from_file_path(Path::new(path)).expect("path should convert")
    }

    #[test]
    fn converts_an_absolute_path() {
        assert_eq!(to_url("/work/project").as_str(), "file:///work/project");
    }

    #[test]
    fn percent_encodes_reserved_characters() {
        assert_eq!(to_url("/work/a b").as_str(), "file:///work/a%20b");
    }

    #[test]
    fn marks_directories_with_a_trailing_slash() {
        let url = <Url as UrlFilePathExt>::from_directory_path(Path::new("/work/project"))
            .expect("path should convert");
        assert_eq!(url.as_str(), "file:///work/project/");
    }

    #[test]
    fn round_trips_a_simple_path() {
        let url = to_url("/work/project/pyproject.toml");
        assert_eq!(
            UrlFilePathExt::to_file_path(&url),
            Ok(PathBuf::from("/work/project/pyproject.toml"))
        );
    }

    #[test]
    fn round_trips_an_encoded_path() {
        let url = to_url("/work/a b/c+d");
        assert_eq!(
            UrlFilePathExt::to_file_path(&url),
            Ok(PathBuf::from("/work/a b/c+d"))
        );
    }

    #[test]
    fn rejects_a_non_file_scheme() {
        let url = Url::parse("https://example.com/x").expect("valid url");
        assert_eq!(UrlFilePathExt::to_file_path(&url), Err(()));
    }

    #[test]
    fn accepts_a_localhost_host() {
        let url = Url::parse("file://localhost/work/project").expect("valid url");
        assert_eq!(
            UrlFilePathExt::to_file_path(&url),
            Ok(PathBuf::from("/work/project"))
        );
    }

    #[test]
    fn rejects_a_remote_host() {
        let url = Url::parse("file://example.com/work").expect("valid url");
        assert_eq!(UrlFilePathExt::to_file_path(&url), Err(()));
    }

    #[test]
    fn normalises_relative_input() {
        assert_eq!(to_url("work/./project").as_str(), "file:///work/project");
    }
}
