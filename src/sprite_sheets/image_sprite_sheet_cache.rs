#![allow(unused)]
use crate::types::{ImageId, SpriteSheetId, SpriteSheetMetadata, SpriteSheetMetadataItem};

use super::image_sprite_sheet::ImageSpriteSheet;
use std::path::{Path, PathBuf};

const SHEET_CAPACITY: usize = 50;
const SHEET_ITEM_WIDTH: u32 = 210;
const SHEET_ITEM_HEIGHT: u32 = 290;

fn sheet_idx_to_path(data_path: &Path, sheet_idx: usize) -> PathBuf {
    data_path.join(sheet_idx.to_string())
}

fn load_sheets(data_path: &Path) -> Vec<ImageSpriteSheet> {
    let mut i = 0;
    let mut ret = Vec::new();
    loop {
        let sheet_path = sheet_idx_to_path(data_path, i);
        if sheet_path.exists() {
            ret.push(ImageSpriteSheet::new(
                sheet_path,
                SHEET_CAPACITY,
                SHEET_ITEM_WIDTH,
                SHEET_ITEM_HEIGHT,
            ));
        } else {
            break;
        }
        i += 1;
    }
    ret
}

fn sheet_contains_image(sheet: &ImageSpriteSheet, id: ImageId) -> bool {
    sheet.metadata().images.iter().any(|item| item.id == id)
}

fn find_sheet_for_id(sheets: &[ImageSpriteSheet], id: ImageId) -> Option<usize> {
    sheets
        .iter()
        .enumerate()
        .find(|(i, sheet)| sheet_contains_image(sheet, id))
        .map(|(i, sheet)| i)
}

fn get_sheet_with_capacity(data_path: &Path, sheets: &mut Vec<ImageSpriteSheet>) -> usize {
    for (i, sheet) in sheets.iter_mut().enumerate() {
        if sheet.remaining_capacity() > 0 {
            return i;
        }
    }

    let sheet_path = sheet_idx_to_path(data_path, sheets.len());
    sheets.push(ImageSpriteSheet::new(
        sheet_path,
        SHEET_CAPACITY,
        SHEET_ITEM_WIDTH,
        SHEET_ITEM_HEIGHT,
    ));
    sheets.len() - 1
}

pub struct ImageSpriteSheetCache {
    sheets: Vec<ImageSpriteSheet>,
    data_path: PathBuf,
}

impl ImageSpriteSheetCache {
    pub fn new(data_path: PathBuf) -> ImageSpriteSheetCache {
        ImageSpriteSheetCache {
            sheets: load_sheets(&data_path),
            data_path,
        }
    }

    pub fn image_in_cache(&self, id: ImageId, url: &str) -> bool {
        if let Some(sheet_idx) = find_sheet_for_id(&self.sheets, id) {
            let image_metadata = self.sheets[sheet_idx]
                .metadata()
                .images
                .iter()
                .find(|item| item.id == id)
                .unwrap();
            image_metadata.url == url
        } else {
            false
        }
    }

    pub fn ensure_image_in_cache(&mut self, id: ImageId, url: &str, image_path: &Path) {
        if let Some(sheet_idx) = find_sheet_for_id(&self.sheets, id) {
            return;
        }

        let sheet_idx = get_sheet_with_capacity(&self.data_path, &mut self.sheets);
        let sheet = &mut self.sheets[sheet_idx];
        sheet.push_image(id, url, image_path);
    }

    pub fn metadata(&self) -> Vec<SpriteSheetMetadata> {
        let mut output = Vec::new();
        for (i, sheet) in self.sheets.iter().enumerate() {
            let mut sheet_output = SpriteSheetMetadata {
                id: SpriteSheetId(i as i64),
                width: sheet.sheet_width_px(),
                items: Vec::new(),
            };
            let sheet_metadata = sheet.metadata();
            for item_metadata in &sheet_metadata.images {
                let output_metadata = SpriteSheetMetadataItem {
                    id: item_metadata.id,
                    width: item_metadata.width as usize,
                    height: item_metadata.height as usize,
                    x_offset: item_metadata.x_offset,
                };
                sheet_output.items.push(output_metadata);
            }
            output.push(sheet_output);
        }
        output
    }

    pub fn data(&self, id: SpriteSheetId) -> Vec<u8> {
        self.sheets[id.0 as usize].data()
    }
}
