use crate::types::ImageId;
use thiserror::Error;

use std::{io::Read, path::PathBuf};

#[derive(Debug, Error)]
pub enum GetImageError {
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("failed to retrieve result")]
    Isahc(#[from] isahc::Error),
}

pub struct ImageCache {
    cache_dir: PathBuf,
}

impl ImageCache {
    pub fn new(cache_dir: PathBuf) -> ImageCache {
        ImageCache { cache_dir }
    }

    pub fn get(&self, id: &ImageId, url: &str) -> Result<Vec<u8>, GetImageError> {
        if !self.cache_dir.exists() {
            std::fs::create_dir_all(&self.cache_dir)?;
        }

        let fs_path = self.cache_dir.join(id.0.to_string());
        if !fs_path.exists() {
            let mut response = isahc::get(url)?;
            let body = response.body_mut();
            let mut content = Vec::new();
            body.read_to_end(&mut content)?;
            std::fs::write(&fs_path, content)?;
        }
        Ok(std::fs::read(fs_path)?)
    }
}
