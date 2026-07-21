use std::{
    collections::VecDeque,
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};

use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    models::{ImportResult, MediaItem, MediaKind, MediaOrigin, SkippedImport},
    network::RemoteDownload,
    settings::write_json_transaction,
};

const MAX_IMAGE_BYTES: u64 = 50 * 1024 * 1024;
const MAX_VIDEO_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Copy)]
struct MediaType {
    kind: MediaKindStatic,
    mime_type: &'static str,
    maximum: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MediaKindStatic {
    Image,
    Video,
}

impl MediaKindStatic {
    fn owned(self) -> MediaKind {
        match self {
            Self::Image => MediaKind::Image,
            Self::Video => MediaKind::Video,
        }
    }
}

fn media_type(extension: &str) -> Option<MediaType> {
    match extension {
        ".png" => Some(MediaType {
            kind: MediaKindStatic::Image,
            mime_type: "image/png",
            maximum: MAX_IMAGE_BYTES,
        }),
        ".jpg" | ".jpeg" => Some(MediaType {
            kind: MediaKindStatic::Image,
            mime_type: "image/jpeg",
            maximum: MAX_IMAGE_BYTES,
        }),
        ".webp" => Some(MediaType {
            kind: MediaKindStatic::Image,
            mime_type: "image/webp",
            maximum: MAX_IMAGE_BYTES,
        }),
        ".gif" => Some(MediaType {
            kind: MediaKindStatic::Image,
            mime_type: "image/gif",
            maximum: MAX_IMAGE_BYTES,
        }),
        ".avif" => Some(MediaType {
            kind: MediaKindStatic::Image,
            mime_type: "image/avif",
            maximum: MAX_IMAGE_BYTES,
        }),
        ".mp4" => Some(MediaType {
            kind: MediaKindStatic::Video,
            mime_type: "video/mp4",
            maximum: MAX_VIDEO_BYTES,
        }),
        ".webm" => Some(MediaType {
            kind: MediaKindStatic::Video,
            mime_type: "video/webm",
            maximum: MAX_VIDEO_BYTES,
        }),
        ".ogv" => Some(MediaType {
            kind: MediaKindStatic::Video,
            mime_type: "video/ogg",
            maximum: MAX_VIDEO_BYTES,
        }),
        ".mov" => Some(MediaType {
            kind: MediaKindStatic::Video,
            mime_type: "video/quicktime",
            maximum: MAX_VIDEO_BYTES,
        }),
        _ => None,
    }
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{}", value.to_ascii_lowercase()))
        .unwrap_or_default()
}

fn safe_display_name(name: &str) -> String {
    let value = name
        .chars()
        .map(|character| {
            if matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            ) || character.is_control()
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    let trimmed = value.trim();
    let result = if trimmed.is_empty() {
        "未命名媒体".to_string()
    } else {
        trimmed.chars().take(180).collect()
    };
    result
}

fn sha256(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn validate_media(path: &Path, media_type: MediaType, extension: &str) -> Result<(), String> {
    let mut header = [0u8; 64];
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let count = file.read(&mut header).map_err(|error| error.to_string())?;
    let header = &header[..count];
    let matches = match extension {
        ".png" => header.starts_with(b"\x89PNG\r\n\x1a\n"),
        ".jpg" | ".jpeg" => header.starts_with(&[0xff, 0xd8, 0xff]),
        ".webp" => header.len() >= 12 && &header[..4] == b"RIFF" && &header[8..12] == b"WEBP",
        ".gif" => header.starts_with(b"GIF87a") || header.starts_with(b"GIF89a"),
        ".avif" => {
            header.len() >= 12
                && &header[4..8] == b"ftyp"
                && header[8..]
                    .windows(4)
                    .any(|brand| matches!(brand, b"avif" | b"avis"))
        }
        ".webm" => header.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]),
        ".ogv" => header.starts_with(b"OggS"),
        ".mp4" | ".mov" => header.len() >= 12 && &header[4..8] == b"ftyp",
        _ => false,
    };
    if !matches {
        return Err(match media_type.kind {
            MediaKindStatic::Image => "图片内容损坏或格式与扩展名不匹配。".to_string(),
            MediaKindStatic::Video => "视频内容损坏或格式与扩展名不匹配。".to_string(),
        });
    }
    if media_type.kind == MediaKindStatic::Image {
        let dimensions =
            imagesize::size(path).map_err(|_| "图片内容损坏或格式与扩展名不匹配。".to_string())?;
        let width = dimensions.width as u64;
        let height = dimensions.height as u64;
        if width < 1
            || height < 1
            || width > 16_384
            || height > 16_384
            || width.saturating_mul(height) > 50_000_000
        {
            return Err("图片尺寸超过 16384 像素或 5000 万总像素上限。".to_string());
        }
    }
    Ok(())
}

struct IngestOptions {
    name: Option<String>,
    origin: MediaOrigin,
    source_url: Option<String>,
    remove_source: bool,
    allow_duplicate: bool,
    extension: Option<String>,
}

pub struct MediaLibrary {
    pub media_directory: PathBuf,
    pub temporary_directory: PathBuf,
    pub catalog_path: PathBuf,
    items: Vec<MediaItem>,
}

impl MediaLibrary {
    pub fn load(data_directory: &Path) -> Result<Self, String> {
        let media_directory = data_directory.join("media");
        let temporary_directory = data_directory.join("temporary");
        let catalog_path = data_directory.join("library.json");
        fs::create_dir_all(&media_directory).map_err(|error| error.to_string())?;
        fs::create_dir_all(&temporary_directory).map_err(|error| error.to_string())?;
        let mut library = Self {
            media_directory,
            temporary_directory,
            catalog_path,
            items: Vec::new(),
        };
        match fs::read_to_string(&library.catalog_path) {
            Ok(content) => match serde_json::from_str::<Vec<MediaItem>>(&content) {
                Ok(items) => {
                    library.items = items
                        .into_iter()
                        .filter(|item| {
                            library
                                .path_for(item)
                                .and_then(|path| {
                                    fs::metadata(path).map_err(|error| error.to_string())
                                })
                                .is_ok_and(|metadata| metadata.is_file())
                        })
                        .collect();
                }
                Err(_) => {
                    let invalid = library
                        .catalog_path
                        .with_extension(format!("json.invalid-{}", Utc::now().timestamp_millis()));
                    let _ = fs::rename(&library.catalog_path, invalid);
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                let invalid = library
                    .catalog_path
                    .with_extension(format!("json.invalid-{}", Utc::now().timestamp_millis()));
                let _ = fs::rename(&library.catalog_path, invalid);
            }
        }
        library.save_catalog()?;
        Ok(library)
    }

    pub fn items(&self) -> Vec<MediaItem> {
        self.items.clone()
    }

    pub fn get_by_id(&self, id: &str) -> Option<MediaItem> {
        self.items.iter().find(|item| item.id == id).cloned()
    }

    pub fn path_for(&self, item: &MediaItem) -> Result<PathBuf, String> {
        let path = Path::new(&item.file_name);
        let mut components = path.components();
        if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
            return Err("媒体目录校验失败。".to_string());
        }
        Ok(self.media_directory.join(path))
    }

    fn save_catalog(&self) -> Result<(), String> {
        write_json_transaction(&self.catalog_path, &self.items)
    }

    fn ingest(
        &mut self,
        source_path: &Path,
        options: IngestOptions,
    ) -> Result<(MediaItem, bool), String> {
        let source = source_path
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let chosen_extension = options
            .extension
            .unwrap_or_else(|| extension(options.name.as_deref().map(Path::new).unwrap_or(&source)))
            .to_ascii_lowercase();
        let media_type =
            media_type(&chosen_extension).ok_or_else(|| "不支持此图片或视频格式。".to_string())?;
        let metadata = fs::metadata(&source).map_err(|error| error.to_string())?;
        if !metadata.is_file() || metadata.len() < 1 {
            return Err("媒体文件为空或不可读取。".to_string());
        }
        if metadata.len() > media_type.maximum {
            return Err(format!(
                "媒体文件超过 {} MB 上限。",
                media_type.maximum / 1024 / 1024
            ));
        }
        validate_media(&source, media_type, &chosen_extension)?;
        let digest = sha256(&source)?;
        if !options.allow_duplicate {
            if let Some(item) = self.items.iter().find(|item| item.sha256 == digest) {
                if options.remove_source {
                    let _ = fs::remove_file(&source);
                }
                return Ok((item.clone(), true));
            }
        }

        let id = Uuid::new_v4().to_string();
        let stored_name = format!("{id}{chosen_extension}");
        let target = self.media_directory.join(&stored_name);
        let temporary = self.media_directory.join(format!(".{id}.incoming"));
        let copy_result = if options.remove_source {
            fs::rename(&source, &temporary).or_else(|_| {
                fs::copy(&source, &temporary)?;
                fs::remove_file(&source)
            })
        } else {
            fs::copy(&source, &temporary).map(|_| ())
        };
        if let Err(error) = copy_result {
            let _ = fs::remove_file(&temporary);
            return Err(error.to_string());
        }
        let result = (|| {
            let copied = fs::metadata(&temporary).map_err(|error| error.to_string())?;
            if copied.len() != metadata.len() {
                return Err("媒体复制校验失败。".to_string());
            }
            fs::rename(&temporary, &target).map_err(|error| error.to_string())?;
            let source_name = source
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("未命名媒体");
            let item = MediaItem {
                id,
                name: safe_display_name(options.name.as_deref().unwrap_or(source_name)),
                kind: media_type.kind.owned(),
                origin: options.origin,
                file_name: stored_name,
                mime_type: media_type.mime_type.to_string(),
                byte_size: metadata.len(),
                sha256: digest,
                source_url: options.source_url,
                created_at: Utc::now().to_rfc3339(),
                preview_url: None,
            };
            self.items.insert(0, item.clone());
            self.save_catalog()?;
            Ok(item)
        })();
        match result {
            Ok(item) => Ok((item, false)),
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                let _ = fs::remove_file(&target);
                Err(error)
            }
        }
    }

    pub fn import_files(&mut self, paths: &[PathBuf]) -> ImportResult {
        let mut result = ImportResult::default();
        for path in paths {
            match self.ingest(
                path,
                IngestOptions {
                    name: None,
                    origin: MediaOrigin::Local,
                    source_url: None,
                    remove_source: false,
                    allow_duplicate: false,
                    extension: None,
                },
            ) {
                Ok((_, true)) => result.skipped.push(SkippedImport {
                    path: path.to_string_lossy().into_owned(),
                    reason: "媒体已存在".to_string(),
                }),
                Ok((item, false)) => result.added.push(item),
                Err(error) => result.skipped.push(SkippedImport {
                    path: path.to_string_lossy().into_owned(),
                    reason: error,
                }),
            }
        }
        result
    }

    pub fn discover_folder(&self, folder: &Path) -> Result<Vec<PathBuf>, String> {
        let root = folder.canonicalize().map_err(|error| error.to_string())?;
        let mut pending = VecDeque::from([root]);
        let mut files = Vec::new();
        while let Some(directory) = pending.pop_front() {
            for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
                let entry = entry.map_err(|error| error.to_string())?;
                let file_type = entry.file_type().map_err(|error| error.to_string())?;
                if file_type.is_symlink() {
                    continue;
                }
                let path = entry.path();
                if file_type.is_dir() {
                    pending.push_back(path);
                } else if file_type.is_file() && media_type(&extension(&path)).is_some() {
                    files.push(path);
                }
                if files.len() + pending.len() > 10_000 {
                    return Err("文件夹内容过多，请选择更具体的目录。".to_string());
                }
            }
        }
        Ok(files)
    }

    pub fn import_download(
        &mut self,
        input_url: &str,
        dynamic: bool,
        download: RemoteDownload,
    ) -> ImportResult {
        let extension = extension(Path::new(&download.original_name));
        let hostname = url::Url::parse(input_url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_string))
            .unwrap_or_else(|| "unknown".to_string());
        let result = self.ingest(
            &download.temporary_path,
            IngestOptions {
                name: Some(if dynamic {
                    format!("随机 API · {hostname}")
                } else {
                    download.original_name
                }),
                origin: if dynamic {
                    MediaOrigin::Api
                } else {
                    MediaOrigin::Remote
                },
                source_url: Some(if dynamic {
                    input_url.to_string()
                } else {
                    download.source_url
                }),
                remove_source: true,
                allow_duplicate: dynamic,
                extension: Some(extension),
            },
        );
        match result {
            Ok((_, true)) => ImportResult {
                added: Vec::new(),
                skipped: vec![SkippedImport {
                    path: input_url.to_string(),
                    reason: "媒体已存在".to_string(),
                }],
            },
            Ok((item, false)) => ImportResult {
                added: vec![item],
                skipped: Vec::new(),
            },
            Err(error) => {
                let _ = fs::remove_file(download.temporary_path);
                ImportResult {
                    added: Vec::new(),
                    skipped: vec![SkippedImport {
                        path: input_url.to_string(),
                        reason: error,
                    }],
                }
            }
        }
    }

    pub fn refresh_with_download(
        &mut self,
        id: &str,
        download: RemoteDownload,
    ) -> Result<MediaItem, String> {
        let index = self
            .items
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| "媒体项目不存在。".to_string())?;
        let item = self.items[index].clone();
        if item.origin != MediaOrigin::Api || item.source_url.is_none() {
            let _ = fs::remove_file(download.temporary_path);
            return Err("该媒体不是随机 API 来源。".to_string());
        }
        let extension = extension(Path::new(&download.original_name));
        let media_type =
            media_type(&extension).ok_or_else(|| "不支持此图片或视频格式。".to_string())?;
        validate_media(&download.temporary_path, media_type, &extension)?;
        let digest = sha256(&download.temporary_path)?;
        if digest == item.sha256 {
            let _ = fs::remove_file(download.temporary_path);
            return Ok(item);
        }
        let previous = self.path_for(&item)?;
        let stored_name = format!("{}-{}{}", item.id, &digest[..12], extension);
        let target = self.media_directory.join(&stored_name);
        fs::rename(&download.temporary_path, &target)
            .or_else(|_| {
                fs::copy(&download.temporary_path, &target)?;
                fs::remove_file(&download.temporary_path)
            })
            .map_err(|error| error.to_string())?;
        if previous != target {
            let _ = fs::remove_file(previous);
        }
        let updated = MediaItem {
            file_name: stored_name,
            mime_type: download.mime_type,
            kind: download.kind,
            byte_size: download.byte_size,
            sha256: digest,
            preview_url: None,
            ..item
        };
        self.items[index] = updated.clone();
        self.save_catalog()?;
        Ok(updated)
    }

    pub fn remove(&mut self, id: &str) -> Result<bool, String> {
        let Some(item) = self.get_by_id(id) else {
            return Ok(false);
        };
        self.items.retain(|candidate| candidate.id != id);
        self.save_catalog()?;
        let path = self.path_for(&item)?;
        match fs::remove_file(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
            Err(error) => Err(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn png_header() -> Vec<u8> {
        let mut bytes = vec![0u8; 24];
        bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes[8..12].copy_from_slice(&13u32.to_be_bytes());
        bytes[12..16].copy_from_slice(b"IHDR");
        bytes[16..20].copy_from_slice(&2u32.to_be_bytes());
        bytes[20..24].copy_from_slice(&2u32.to_be_bytes());
        bytes
    }

    #[test]
    fn imports_deduplicates_and_removes_media() {
        let root = std::env::temp_dir().join(format!("codex-media-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("中文背景.png");
        File::create(&source)
            .unwrap()
            .write_all(&png_header())
            .unwrap();
        let mut library = MediaLibrary::load(&root.join("data")).unwrap();
        let first = library.import_files(std::slice::from_ref(&source));
        let duplicate = library.import_files(std::slice::from_ref(&source));
        assert_eq!(first.added.len(), 1);
        assert_eq!(first.added[0].name, "中文背景.png");
        assert!(duplicate.added.is_empty());
        assert_eq!(duplicate.skipped[0].reason, "媒体已存在");
        assert!(library.remove(&first.added[0].id).unwrap());
        assert!(library.items().is_empty());
        let _ = fs::remove_dir_all(root);
    }
}
