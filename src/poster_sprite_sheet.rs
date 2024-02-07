#![allow(unused)]
use std::path::{Path, PathBuf};
use image::{imageops::FilterType, RgbImage, ColorType};

use crate::types::ImageId;

fn slot_for_id(ids: &[ImageId], id: ImageId) -> usize {
    match ids.iter().enumerate().find(|(_i, item)| **item == id).map(|(i, _item)| i) {
        Some(idx) => idx,
        None => ids.len()
    }
}

fn slot_to_x_offs(slot: usize, item_width: u32) -> u32 {
    let slot_u32: u32 = slot.try_into().unwrap();
    slot_u32 * item_width
}

fn get_metadata_path(path: &Path) -> PathBuf {
    path.join("metadata.json")
}

fn get_image_path(path: &Path) -> PathBuf {
    path.join("image.jpg")
}

const PIXEL_SIZE: usize = 3;

fn create_sprite_sheet(data_path: &Path, item_width: u32, item_height: u32, capacity: usize) {
    let width = item_width as usize * capacity;
    let height = item_height as usize;
    let data = vec![0; width * height * capacity * PIXEL_SIZE];

    image::save_buffer(get_image_path(data_path), &data, width as u32, height as u32, ColorType::Rgb8).unwrap();
}

fn open_sprite_sheet(data_path: &Path, item_width: u32, item_height: u32, capacity: usize) -> RgbImage {
    if !data_path.exists() {
        create_sprite_sheet(data_path, item_width, item_height, capacity);
    }

    let sprite_sheet = image::open(get_image_path(data_path)).unwrap();
    sprite_sheet.to_rgb8()
}

struct ImageSpriteSheet {
    ids: Vec<ImageId>,
    path: PathBuf,
    capacity: usize,
    item_width: u32,
    item_height: u32,
}

impl ImageSpriteSheet {
    fn new(path: PathBuf, capacity: usize, item_width: u32, item_height: u32) -> ImageSpriteSheet {
        ImageSpriteSheet {
            ids: Vec::new(),
            path,
            capacity,
            item_width,
            item_height,
        }
    }

    fn push_image(&mut self, id: ImageId, image_path: &Path) {
        let slot = slot_for_id(&self.ids, id);
        let x_offs = slot_to_x_offs(slot, self.item_width) as usize;
        let img_data = image::open(image_path).unwrap();
        let img_data = img_data.resize(self.item_width, self.item_height, FilterType::Lanczos3).to_rgb8();

        let sprite_sheet = open_sprite_sheet(&self.path, self.item_width, self.item_height, self.capacity);
        let mut sprite_sheet_data = sprite_sheet.into_raw();

        let dest_lines = sprite_sheet_data.chunks_mut(self.item_width as usize * self.capacity)
            .map(|line| &mut line[x_offs..x_offs + self.item_width as usize]);

        let source_lines = img_data.as_raw().chunks(self.item_width as usize * PIXEL_SIZE);
        for (dest_line, source_line) in dest_lines.zip(source_lines) {
            dest_line.copy_from_slice(source_line);
        }
    }
}

struct ImageSpriteSheetCache {
    sheets: Vec<ImageSpriteSheet>,
    data_path: PathBuf
}

impl ImageSpriteSheetCache {
    fn new(data_path: PathBuf) {

    }
}

// Once a sprite sheet is generated, an item should never move
// /sprite_sheet_info
// [
//   {
//     id: 1,
//     images: [1, 2, 3],
//     offsets: [0, 210, 420],
//   }
//
// ]
// /sprite_sheets/id
