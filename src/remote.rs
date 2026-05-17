use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::{BufReader, Read, Write, copy};
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct RemoteResource {
    pub name: String,
    pub url: String,
}

pub type RemoteResources = HashMap<String, Vec<RemoteResource>>;

impl RemoteResource {
    pub fn fetch_remote_json(url: &str) -> Result<RemoteResources, Box<dyn Error>> {
        let response = ureq::get(url).call()?;
        let body = response.into_body().read_to_string()?;
        let resources: RemoteResources = serde_json::from_str(&body)?;
        Ok(resources)
    }

    pub fn download_to(&self, dest_dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
        self.download_to_with_progress(dest_dir, dest_dir, |_, _| {})
    }

    pub fn download_to_with_progress<F>(
        &self,
        download_dir: &Path,
        extract_dir: &Path,
        on_progress: F,
    ) -> Result<PathBuf, Box<dyn Error>>
    where
        F: Fn(u64, u64),
    {
        fs::create_dir_all(download_dir)?;
        fs::create_dir_all(extract_dir)?;

        let ext = archive_extension(&self.url);
        let archive_name = format!("{}{}", self.name, ext);
        let archive_path = download_dir.join(&archive_name);

        download_file_with_progress(&self.url, &archive_path, &on_progress)?;
        drop(on_progress);

        let output_dir = extract_dir.join(&self.name);
        extract_archive(&archive_path, &output_dir)?;
        flatten_single_subdivide(&output_dir)?;

        fs::remove_file(&archive_path)?;

        Ok(output_dir)
    }
}

pub fn download_file(url: &str, dest: &Path) -> Result<PathBuf, Box<dyn Error>> {
    download_file_with_progress(url, dest, |_, _| {})
}

pub fn download_file_with_progress<F>(
    url: &str,
    dest: &Path,
    on_progress: F,
) -> Result<PathBuf, Box<dyn Error>>
where
    F: Fn(u64, u64),
{
    let response = ureq::get(url).call()?;

    let total: u64 = response
        .headers()
        .get("content-length")
        .map(|v| v.to_str().unwrap_or("0").parse::<u64>().unwrap_or(0))
        .unwrap_or(0);

    let mut binding = response.into_body();
    let mut reader = binding.as_reader();
    let mut downloaded: u64 = 0;
    let mut buffer = [0u8; 65536];

    let mut file = fs::File::create(dest)?;

    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n])?;
        downloaded += n as u64;
        on_progress(downloaded, total);
    }

    Ok(dest.to_path_buf())
}

fn flatten_single_subdivide(dir: &Path) -> Result<(), Box<dyn Error>> {
    loop {
        let entries: Vec<_> = fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();

        if entries.len() == 1 && entries[0].file_type().map_or(false, |t| t.is_dir()) {
            let subdivide = entries[0].path();
            for entry in fs::read_dir(&subdivide)? {
                let entry = entry?;
                let target = dir.join(entry.file_name());
                fs::rename(entry.path(), &target)?;
            }
            fs::remove_dir(&subdivide)?;
        } else {
            break;
        }
    }
    Ok(())
}

pub fn extract_archive(archive_path: &Path, dest_dir: &Path) -> Result<(), Box<dyn Error>> {
    let path_str = archive_path.to_string_lossy().to_lowercase();

    if path_str.ends_with(".zip") {
        extract_zip(archive_path, dest_dir)
    } else if path_str.ends_with(".tar.gz") || path_str.ends_with(".tgz") {
        extract_tar_gz(archive_path, dest_dir)
    } else {
        Err(format!("unsupported archive format: {}", path_str).into())
    }
}

fn extract_zip(archive_path: &Path, dest_dir: &Path) -> Result<(), Box<dyn Error>> {
    let file = fs::File::open(archive_path)?;
    let reader = BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let entry_path = entry.mangled_name();
        let target_path = dest_dir.join(&entry_path);

        if entry.is_dir() {
            fs::create_dir_all(&target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out = fs::File::create(&target_path)?;
            copy(&mut entry, &mut out)?;
        }
    }

    Ok(())
}

fn archive_extension(url: &str) -> String {
    let lower = url.to_lowercase();
    if lower.ends_with(".tar.gz") {
        return ".tar.gz".into();
    }
    if lower.ends_with(".tgz") {
        return ".tgz".into();
    }
    if let Some(dot) = lower.rfind('.') {
        url[dot..].to_string()
    } else {
        String::new()
    }
}

fn extract_tar_gz(archive_path: &Path, dest_dir: &Path) -> Result<(), Box<dyn Error>> {
    let file = fs::File::open(archive_path)?;
    let reader = BufReader::new(file);
    let gz_decoder = flate2::read::GzDecoder::new(reader);
    let mut archive = tar::Archive::new(gz_decoder);

    archive.unpack(dest_dir)?;

    Ok(())
}
