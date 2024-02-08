#![allow(unused)]
use crate::types::ImageId;

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
        }
        i += 1;
    }
    ret
}

fn sheet_contains_image(sheet: &ImageSpriteSheet, id: ImageId) -> bool {
    sheet.metadata().images.iter()
        .any(|item| item.id == id)
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

    pub fn get_sheet_for_image(
        &mut self,
        id: ImageId,
        url: &str,
        image_path: &Path,
    ) -> &ImageSpriteSheet {
        if let Some(sheet_idx) = find_sheet_for_id(&self.sheets, id) {
            return &self.sheets[sheet_idx];
        }

        let sheet_idx = get_sheet_with_capacity(&self.data_path, &mut self.sheets);
        let sheet = &mut self.sheets[sheet_idx];
        sheet.push_image(id, url, image_path);
        sheet
    }
}
