#![allow(unused)]
use image::{imageops::FilterType, ColorType, RgbImage};
use serde::{Deserialize, Serialize};
use std::{path::{Path, PathBuf}, task::Wake};
use thiserror::Error;

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

fn slot_to_x_offset_px(slot: usize, item_width: u32) -> u32 {
    let slot_u32: u32 = slot.try_into().unwrap();
    slot_u32 * item_width
}

const PIXEL_SIZE: usize = 3;

fn create_sprite_sheet_image(image_path: &Path, item_width: u32, item_height: u32, capacity: usize) {
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

fn open_sprite_sheet_image(image_path: &Path, item_width: u32, item_height: u32, capacity: usize) -> RgbImage {
    if !image_path.exists() {
        create_sprite_sheet_image(&image_path, item_width, item_height, capacity);
    }

    let sprite_sheet = image::open(image_path).unwrap();
    sprite_sheet.to_rgb8()
}

fn load_image_for_sprite_sheet_insertion(image_path: &Path, item_width: u32, item_height: u32) -> RgbImage {
    let img_data = image::open(image_path).unwrap();
    img_data
        .resize(
            item_width,
            item_height,
            FilterType::Lanczos3,
        )
        .to_rgb8()
}


struct RgbSpriteSheet {
    sheet_image_path: PathBuf,
    sheet_width_bytes: usize,
    item_width_bytes: usize,
    buf: Vec<u8>,
}

impl RgbSpriteSheet {
    fn new(sheet_image_path: PathBuf, item_width_px: u32, item_height_px: u32, capacity: usize) -> RgbSpriteSheet {
        let img = open_sprite_sheet_image(&sheet_image_path, item_width_px, item_height_px, capacity);
        let sheet_width_bytes = img.width() as usize * PIXEL_SIZE;
        let item_width_bytes = item_width_px  as usize * PIXEL_SIZE;

        RgbSpriteSheet {
            sheet_image_path,
            sheet_width_bytes,
            item_width_bytes,
            buf: img.into_raw()
        }
    }

    fn slot_to_x_offset_bytes(&self, slot: usize) -> usize {
        slot * self.item_width_bytes
    }

    /// returns inserted x offset in px
    fn copy_image( &mut self, image: &RgbImage, slot_idx: usize) -> usize {
        let x_start_bytes = self.slot_to_x_offset_bytes(slot_idx);
        let x_end_bytes = x_start_bytes + self.item_width_bytes;

        let dest_lines = self.buf
            .chunks_mut(self.sheet_width_bytes)
            .map(|line| &mut line[x_start_bytes..x_end_bytes]);

        let source_lines = image
            .as_raw()
            .chunks(image.width() as usize * PIXEL_SIZE);

        for (dest_line, source_line) in dest_lines.zip(source_lines) {
            dest_line[0..source_line.len()].copy_from_slice(source_line);
        }

        x_start_bytes / PIXEL_SIZE
    }

    fn save(&self) {
        image::save_buffer(
            &self.sheet_image_path,
            &self.buf,
            self.sheet_width_bytes as u32 / PIXEL_SIZE as u32,
            self.buf.len() as u32 / self.sheet_width_bytes as u32,
            ColorType::Rgb8,
        )
        .unwrap();
    }
}

struct SpriteSheetFolder {
    path: PathBuf,
}

impl SpriteSheetFolder {
    fn metadata_path(&self) -> PathBuf {
        self.path.join("metadata.json")
    }

    fn image_path(&self) -> PathBuf {
        self.path.join("image.jpg")
    }
}


#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ItemMetadata {
    pub id: ImageId,
    pub url: String,
    pub x_offset: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
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

    fn find_id(&self, id: ImageId) -> Option<&ItemMetadata> {
        self.images.iter().find(|item| {
            item.id == id
        })
    }

    fn update(&mut self, to_insert: ItemMetadata) {
        if let Some(idx) = self.images.iter().position(|item| {
            item.id == to_insert.id
        }) {
            self.images[idx] = to_insert;
        } else {
            self.images.push(to_insert);
        }
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

#[derive(Debug, Error)]
enum InsertImageErrorKind {
    #[error("sprite sheet full")]
    Full
}

#[derive(Debug, Error)]
#[error(transparent)]
pub struct InsertImageError(#[from] InsertImageErrorKind);

pub struct ImageSpriteSheet {
    folder: SpriteSheetFolder,
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

        let folder = SpriteSheetFolder { path };

        let metadata = Metadata::load(&folder.metadata_path()).and_then(|metadata| {
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

        ImageSpriteSheet { metadata, folder }
    }

    pub fn push_image(&mut self, id: ImageId, url: &str, image_path: &Path) -> Result<(), InsertImageError> {
        println!("Pushing image");
        let slot = slot_for_id(&self.metadata.images, id);

        if let Some(existing_metadata) = self.metadata.find_id(id) {
            if existing_metadata.url == url {
                return Ok(());
            }
        }

        if self.remaining_capacity() == 0 {
            return Err(InsertImageErrorKind::Full.into());
        }

        let img_data = load_image_for_sprite_sheet_insertion(image_path, self.metadata.item_width, self.metadata.item_height);

        println!("Loading image");
        let mut sprite_sheet_image = RgbSpriteSheet::new(
            self.folder.image_path(),
            self.metadata.item_width,
            self.metadata.item_height,
            self.metadata.capacity,
        );
        println!("Done loading");
        let x_offs_px = sprite_sheet_image.copy_image(&img_data, slot);

        println!("Saving");
        sprite_sheet_image.save();
        println!("Done saving");

        self.metadata.update(ItemMetadata {
            id,
            url: url.to_string(),
            width: img_data.width(),
            height: img_data.height(),
            x_offset: x_offs_px as u32,
        });

        self.metadata.save(&self.folder.metadata_path());

        Ok(())
    }

    pub fn data(&self) -> Vec<u8> {
        std::fs::read(&self.folder.image_path()).unwrap()
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
        for y in 0..item_metadata.height {
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

        fixture.sheet.push_image(ImageId(0), "test", &img_path).expect("failed to push image");
        let loaded = load_image_from_sprite_sheet(&fixture.sheet, ImageId(0));
        // Not a perfect match as we save jpgs which are lossy
        let distance = calc_slice_distance(&loaded, img.as_raw());
        assert!(distance < 500);
    }

    #[test]
    fn test_image_update() {
        // Push image with same id, but different URL, we should see that the image is the second
        // thing we inserted, not the first
        let mut fixture = Fixture::new();

        let img = generate_image([0, 0, 0], [255, 255, 255]);
        let img_path = fixture.tmp.path().join("img.png");
        img.save(&img_path);

        fixture.sheet.push_image(ImageId(0), "test", &img_path).expect("failed to push image");

        let img = generate_image([255, 255, 255], [0, 0, 0]);
        img.save(&img_path);
        fixture.sheet.push_image(ImageId(0), "test2", &img_path).expect("failed to push image");

        assert_eq!(fixture.sheet.metadata().images.len(), 1);

        let loaded = load_image_from_sprite_sheet(&fixture.sheet, ImageId(0));
        // Not a perfect match as we save jpgs which are lossy
        let distance = calc_slice_distance(&loaded, img.as_raw());
        assert!(distance < 500);
    }

    #[test]
    fn test_existing_image() {
        // Push image with same id, but same URL, we should see that the second image was not
        // inserted as it was already there
        let mut fixture = Fixture::new();

        let img1 = generate_image([0, 0, 0], [255, 255, 255]);
        let img_path = fixture.tmp.path().join("img.png");
        img1.save(&img_path);

        fixture.sheet.push_image(ImageId(0), "test", &img_path).expect("failed to push image");

        let img2 = generate_image([255, 255, 255], [0, 0, 0]);
        img2.save(&img_path);
        fixture.sheet.push_image(ImageId(0), "test", &img_path).expect("failed to push image");

        assert_eq!(fixture.sheet.metadata().images.len(), 1);

        let loaded = load_image_from_sprite_sheet(&fixture.sheet, ImageId(0));
        // Not a perfect match as we save jpgs which are lossy
        let distance = calc_slice_distance(&loaded, img1.as_raw());
        assert!(distance < 500);
    }

    #[test]
    fn test_push_full() {
        // Fill up the sprite sheet, try to push one too many. Should complain that no more items
        // will fit
        let mut fixture = Fixture::new();

        let img = generate_image([0, 0, 0], [255, 255, 255]);
        let img_path = fixture.tmp.path().join("img.png");
        img.save(&img_path);

        for i in 0..DEFAULT_ITEM_CAPACITY {
            fixture.sheet.push_image(ImageId(i as i64), "test", &img_path).expect("failed to push image");
        }

        let res = fixture.sheet.push_image(ImageId(DEFAULT_ITEM_CAPACITY as i64), "test", &img_path);
        assert!(matches!(res, Err(InsertImageError(InsertImageErrorKind::Full))));
    }

    #[test]
    fn test_remaining_capacity() {
        // Ensure capacity is correctly flagged
        let mut fixture = Fixture::new();

        let img = generate_image([0, 0, 0], [255, 255, 255]);
        let img_path = fixture.tmp.path().join("img.png");
        img.save(&img_path);

        for i in 0..DEFAULT_ITEM_CAPACITY {
            fixture.sheet.push_image(ImageId(i as i64), "test", &img_path).expect("failed to push image");
            assert_eq!(fixture.sheet.remaining_capacity(), DEFAULT_ITEM_CAPACITY - i - 1);
        }

    }

    #[test]
    fn test_load() {
        // Ensure capacity is correctly flagged
        // Insert different images, check the metadata, ensure that loading the data again matches
        let mut fixture = Fixture::new();

        for i in 0..DEFAULT_ITEM_CAPACITY {
            let min = (i * 255 / DEFAULT_ITEM_CAPACITY) as u8;
            let max = min + 255 / DEFAULT_ITEM_CAPACITY as u8;
            let img = generate_image([min, min, min], [max, max, max]);
            let img_path = fixture.tmp.path().join("img.png");
            img.save(&img_path);

            fixture.sheet.push_image(ImageId(i as i64), "test", &img_path).expect("failed to push image");
        }

        let reloaded = ImageSpriteSheet::new(
            fixture.tmp.path().join("sheet"),
            DEFAULT_ITEM_CAPACITY,
            DEFAULT_ITEM_WIDTH,
            DEFAULT_ITEM_HEIGHT);
        assert_eq!(reloaded.metadata(), fixture.sheet.metadata());
        assert_eq!(reloaded.data(), fixture.sheet.data());
    }
}
