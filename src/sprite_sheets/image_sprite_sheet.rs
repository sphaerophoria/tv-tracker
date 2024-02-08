#![allow(unused)]
use image::{imageops::FilterType, ColorType, RgbImage};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::types::ImageId;

fn slot_for_id(ids: &[ItemMetadata], id: ImageId) -> usize {
    match ids
        .iter()
        .enumerate()
        .find(|(_i, item)| item.id == id)
        .map(|(i, _item)| i)
    {
        Some(idx) => idx,
        None => ids.len(),
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

fn create_image(image_path: &Path, item_width: u32, item_height: u32, capacity: usize) {
    let width = item_width as usize * capacity;
    let height = item_height as usize;
    let data = vec![0; width * height * PIXEL_SIZE];

    image::save_buffer(
        image_path,
        &data,
        width as u32,
        height as u32,
        ColorType::Rgb8,
    )
    .unwrap();
}

fn open_image(data_path: &Path, item_width: u32, item_height: u32, capacity: usize) -> RgbImage {
    let image_path = get_image_path(&data_path);
    if !image_path.exists() {
        create_image(&image_path, item_width, item_height, capacity);
    }

    let sprite_sheet = image::open(image_path).unwrap();
    sprite_sheet.to_rgb8()
}


#[derive(Serialize, Deserialize)]
pub struct ItemMetadata {
    pub id: ImageId,
    pub x_offset: u32,
    pub y_offset: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Metadata {
    pub images: Vec<ItemMetadata>,
    pub capacity: usize,
    pub item_width: u32,
    pub item_height: u32,
}

impl Metadata {
    fn load(path: &Path) -> Option<Metadata> {
        let Ok(metadata_file) = std::fs::File::open(path) else {
            return None;
        };
        let metadata = serde_json::from_reader(metadata_file).unwrap();
        Some(metadata)
    }

    fn save(&self, path: &Path) {
        let f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .unwrap();

        serde_json::to_writer_pretty(f, self).unwrap()
    }
}

pub struct ImageSpriteSheet {
    path: PathBuf,
    metadata: Metadata,
}

impl ImageSpriteSheet {
    pub fn new(
        path: PathBuf,
        capacity: usize,
        item_width: u32,
        item_height: u32,
    ) -> ImageSpriteSheet {
        if !path.exists() {
            std::fs::create_dir_all(&path).unwrap();
        }

        let metadata = Metadata::load(&get_metadata_path(&path)).and_then(|metadata| {
            if metadata.capacity == capacity
                && metadata.item_width == item_width
                && metadata.item_height == item_height
            {
                Some(metadata)
            } else {
                None
            }
        });

        let metadata = match metadata {
            Some(v) => v,
            None => Metadata {
                images: Vec::new(),
                capacity,
                item_width,
                item_height,
            },
        };

        ImageSpriteSheet { metadata, path }
    }

    pub fn push_image(&mut self, id: ImageId, image_path: &Path) {
        // FIXME: return early if url matches
        // FIXME: return error if capacity is already full
        // FIXME: Store width and height of inserted item (may not be perfectly the slot size)
        // FIXME: replace image if exists
        let slot = slot_for_id(&self.metadata.images, id);
        let x_offs = slot_to_x_offs(slot, self.metadata.item_width) as usize;

        let img_data = image::open(image_path).unwrap();
        let img_data = img_data
            .resize(
                self.metadata.item_width,
                self.metadata.item_height,
                FilterType::Lanczos3,
            )
            .to_rgb8();

        let sprite_sheet = open_image(
            &self.path,
            self.metadata.item_width,
            self.metadata.item_height,
            self.metadata.capacity,
        );
        let mut sprite_sheet_data = sprite_sheet.into_raw();


        let dest_lines = sprite_sheet_data
            .chunks_mut(self.metadata.item_width as usize * self.metadata.capacity * PIXEL_SIZE)
            .map(|line| &mut line[x_offs * PIXEL_SIZE ..x_offs * PIXEL_SIZE  + self.metadata.item_width as usize * PIXEL_SIZE]);

        let source_lines = img_data
            .as_raw()
            .chunks(self.metadata.item_width as usize * PIXEL_SIZE);

        for (dest_line, source_line) in dest_lines.zip(source_lines) {
            dest_line.copy_from_slice(source_line);
        }

        image::save_buffer(
            &get_image_path(&self.path),
            &sprite_sheet_data,
            self.metadata.item_width * self.metadata.capacity as u32,
            self.metadata.item_height,
            ColorType::Rgb8,
        )
        .unwrap();

        self.metadata.images.push(ItemMetadata {
            id,
            width: img_data.width(),
            height: img_data.height(),
            x_offset: x_offs as u32,
            y_offset: 0,
        });

        self.metadata.save(&get_metadata_path(&self.path));
    }

    pub fn data(&self) -> Vec<u8> {
        std::fs::read(get_image_path(&self.path)).unwrap()
    }

    pub fn remaining_capacity(&self) -> usize {
        self.metadata.capacity - self.metadata.images.len()
    }

    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    fn sheet_width_px(&self) -> usize {
        self.metadata.item_width as usize * self.metadata.capacity
    }
}

#[cfg(test)]
mod test {
    use image::Rgb;
    use tempfile::TempDir;

    use super::*;

    const DEFAULT_ITEM_WIDTH: u32 = 10;
    const DEFAULT_ITEM_HEIGHT: u32 = 10;
    const DEFAULT_ITEM_CAPACITY: usize = 3;

    fn interpolate(start: u8, end: u8, val: f32) -> u8 {
        let distance = end as i16 - start as i16;
        let ret = start as f32 + val * distance as f32;
        ret as u8
    }

    fn generate_image(top: [u8; 3], bottom: [u8; 3]) -> RgbImage {
        let w = DEFAULT_ITEM_WIDTH as usize;
        let h = DEFAULT_ITEM_HEIGHT as usize;
        let mut img = Vec::with_capacity(w * h * 3);

        for y in 0..h {
            let y_ratio = y as f32 / (h - 1) as f32;
            let r = interpolate(top[0], bottom[0], y_ratio);
            let g = interpolate(top[1], bottom[1], y_ratio);
            let b = interpolate(top[2], bottom[2], y_ratio);
            for x in 0..w {
                img.push(r);
                img.push(g);
                img.push(b);
            }
        }

        RgbImage::from_raw(w as u32, h as u32, img).expect("failed to generate image")
    }

    struct Fixture {
        sheet: ImageSpriteSheet,
        tmp: TempDir,
    }

    impl Fixture {
        fn new() -> Fixture {
            let tmp = TempDir::new().expect("failed to create temp dir");
            let sheet = ImageSpriteSheet::new(
                tmp.path().join("sheet"),
                DEFAULT_ITEM_CAPACITY,
                DEFAULT_ITEM_WIDTH,
                DEFAULT_ITEM_HEIGHT,
            );

            Fixture { tmp, sheet }
        }
    }

    fn load_image_from_sprite_sheet(sheet: &ImageSpriteSheet, id: ImageId) -> Vec<u8> {
        let item_metadata = sheet.metadata().images.iter().find(|item| item.id == id).expect("failed to find image");
        let sheet_width_bytes = sheet.metadata().capacity * sheet.metadata().item_width  as usize * PIXEL_SIZE;
        let mut output = Vec::new();
        let data = sheet.data();
        let data = image::load_from_memory(&data).unwrap().to_rgb8().into_raw();
        for y in item_metadata.y_offset..item_metadata.y_offset + item_metadata.height {
            for x in item_metadata.x_offset * PIXEL_SIZE as u32..item_metadata.x_offset * PIXEL_SIZE  as u32+ item_metadata.width * (PIXEL_SIZE as u32) {
                output.push(data[(y * sheet_width_bytes as u32 + x) as usize])
            }
        }
        output
    }

    fn calc_slice_distance(a: &[u8], b: &[u8]) -> u64 {
        assert_eq!(a.len(), b.len());
        a.iter().zip(b).map(|(a, b)| (*a as i64 - *b as i64).abs() as u64).sum()
    }

    #[test]
    fn test_image_insertion() {
        let mut fixture = Fixture::new();

        // Push an image, check that the inserted thing is == to what we inserted
        let img = generate_image([0, 0, 0], [255, 255, 255]);
        let img_path = fixture.tmp.path().join("img.png");
        img.save(&img_path);

        fixture.sheet.push_image(ImageId(0), &img_path);
        let loaded = load_image_from_sprite_sheet(&fixture.sheet, ImageId(0));
        // Not a perfect match as we save jpgs which are lossy
        let distance = calc_slice_distance(&loaded, img.as_raw());
        assert!(distance < 500);
    }

    #[test]
    fn test_image_update() {
        // Push image with same id, but different URL
        let mut fixture = Fixture::new();

        // Push an image, check that the inserted thing is == to what we inserted
        let img = generate_image([0, 0, 0], [255, 255, 255]);
        let img_path = fixture.tmp.path().join("img.png");
        img.save(&img_path);

        fixture.sheet.push_image(ImageId(0), &img_path);

        let img = generate_image([255, 255, 255], [0, 0, 0]);
        img.save(&img_path);
        fixture.sheet.push_image(ImageId(0), &img_path);

        assert_eq!(fixture.sheet.metadata().images.len(), 1);

        let loaded = load_image_from_sprite_sheet(&fixture.sheet, ImageId(0));
        // Not a perfect match as we save jpgs which are lossy
        let distance = calc_slice_distance(&loaded, img.as_raw());
        assert!(distance < 500);
    }

    #[test]
    fn test_existing_image() {
        // Push image with same id and same url
    }

    #[test]
    fn test_push_full() {
        // Try to push image when capacity is already met
    }

    #[test]
    fn test_remaining_capacity() {
        // Ensure capacity is correctly flagged
    }

    #[test]
    fn test_load() {
        // Test that re-loading the same sprite sheet results in cached data still being there
    }
}
