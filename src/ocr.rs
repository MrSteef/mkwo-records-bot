use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;
use std::{env, fs};
use std::io::Cursor;
use std::path::Path;

use image::codecs::png::PngEncoder;
use image::{DynamicImage, ImageBuffer, ImageEncoder, Luma, Rgb};
use imageproc::{
    contrast::{ThresholdType, otsu_level, threshold_mut},
    distance_transform::Norm,
    morphology::{dilate, erode},
    region_labelling::{Connectivity, connected_components},
};
use leptess::{LepTess, Variable};

/// Unified data type for pipeline
pub enum PipelineData {
    Rgb8(ImageBuffer<Rgb<u8>, Vec<u8>>),
    Luma8(ImageBuffer<Luma<u8>, Vec<u8>>),
    PngBytes(Vec<u8>),
    Text(String),
}

impl PipelineData {
    pub fn into_rgb8(self) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>> {
        if let PipelineData::Rgb8(buf) = self {
            Ok(buf)
        } else {
            Err(anyhow!("Expected RGB image"))
        }
    }

    pub fn into_luma8(self) -> Result<ImageBuffer<Luma<u8>, Vec<u8>>> {
        if let PipelineData::Luma8(buf) = self {
            Ok(buf)
        } else {
            Err(anyhow!("Expected Luma image"))
        }
    }

    pub fn into_bytes(self) -> Result<Vec<u8>> {
        if let PipelineData::PngBytes(v) = self {
            Ok(v)
        } else {
            Err(anyhow!("Expected PNG bytes"))
        }
    }

    pub fn into_text(self) -> Result<String> {
        if let PipelineData::Text(s) = self {
            Ok(s)
        } else {
            Err(anyhow!("Expected text"))
        }
    }
}

/// A single pipeline step
pub trait Step: Send + Sync {
    fn name(&self) -> &'static str;
    fn process(&self, data: PipelineData) -> Result<PipelineData>;
    fn run(
        &self,
        data: PipelineData,
        debug: bool,
        idx: usize,
        msg_id: u64,
    ) -> Result<PipelineData> {
        let out = self
            .process(data)
            .with_context(|| format!("step {} failed", self.name()))?;
        if debug {
            let debug_path = env::var("DEBUG_FOLDER");
            match debug_path {
                Ok(folder) => {
                    let msg_dir = Path::new(&folder).join(msg_id.to_string());
                    fs::create_dir_all(&msg_dir)?;
                    let filename = msg_dir.join(format!("{:02}_{}.png", idx, self.name()));
                    eprintln!("Saving debug image to {}", filename.display());
                    
                    match &out {
                        PipelineData::Rgb8(buf) => buf.save(&filename)?,
                        PipelineData::Luma8(buf) => buf.save(&filename)?,
                        _ => (),
                    }
                }
                Err(why) => {
                    eprintln!("failed to save debug image file: {}", why);
                }
            }
        }
        Ok(out)
    }
}

pub fn run_pipeline_from_bytes(bytes: &[u8], debug: bool, msg_id: u64) -> Result<String> {
    // 1) load into an RGB buffer
    let img = image::load_from_memory(bytes)
        .context("failed to load image from bytes")?
        .to_rgb8();
    let original = img.clone();

    // 2) build up your Vec<Box<dyn Step>>
    let steps: Vec<Box<dyn Step>> = vec![
        Box::new(LoadFromBuffer {
            img: original.clone(),
        }),
        Box::new(YellowMask),
        Box::new(Morph {
            kind: MorphKind::Open,
            radius: 3,
        }),
        Box::new(Morph {
            kind: MorphKind::Close,
            radius: 3,
        }),
        Box::new(Morph {
            kind: MorphKind::Open,
            radius: 3,
        }),
        Box::new(Morph {
            kind: MorphKind::Close,
            radius: 7,
        }),
        Box::new(FindCardCrop {
            orig: original.clone(),
        }),
        Box::new(GrayscaleThresh),
        Box::new(Morph {
            kind: MorphKind::Open,
            radius: 3,
        }),
        Box::new(Morph {
            kind: MorphKind::Close,
            radius: 3,
        }),
        Box::new(CropRegion),
        Box::new(EncodePng),
        Box::new(OcrText),
    ];

    // 3) run them
    let mut data = PipelineData::Rgb8(original);
    for (i, step) in steps.into_iter().enumerate() {
        data = step.run(data, debug, i, msg_id)?;
    }

    // 4) pull out the String
    Ok(data
        .into_text()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?)
}

// -- Step implementations --

pub struct LoadFromBuffer {
    pub img: ImageBuffer<Rgb<u8>, Vec<u8>>,
}
impl Step for LoadFromBuffer {
    fn name(&self) -> &'static str {
        "load"
    }
    fn process(&self, _data: PipelineData) -> Result<PipelineData> {
        Ok(PipelineData::Rgb8(self.img.clone()))
    }
}

pub struct YellowMask;
impl Step for YellowMask {
    fn name(&self) -> &'static str {
        "mask_yellow"
    }
    fn process(&self, data: PipelineData) -> Result<PipelineData> {
        let rgb = data.into_rgb8()?;
        let (w, h) = rgb.dimensions();
        let mask = ImageBuffer::from_fn(w, h, |x, y| {
            let p = rgb.get_pixel(x, y);
            if is_yellow((p[0], p[1], p[2])) {
                Luma([255])
            } else {
                Luma([0])
            }
        });
        Ok(PipelineData::Luma8(mask))
    }
}

enum MorphKind {
    Open,
    Close,
}

pub struct Morph {
    pub kind: MorphKind,
    pub radius: u8,
}
impl Step for Morph {
    fn name(&self) -> &'static str {
        match self.kind {
            MorphKind::Open => "open",
            MorphKind::Close => "close",
        }
    }
    fn process(&self, data: PipelineData) -> Result<PipelineData> {
        let img = data.into_luma8()?;
        let out = match self.kind {
            MorphKind::Open => dilate(
                &erode(&img, Norm::LInf, self.radius),
                Norm::LInf,
                self.radius,
            ),
            MorphKind::Close => erode(
                &dilate(&img, Norm::LInf, self.radius),
                Norm::LInf,
                self.radius,
            ),
        };
        Ok(PipelineData::Luma8(out))
    }
}

pub struct FindCardCrop {
    pub orig: ImageBuffer<Rgb<u8>, Vec<u8>>,
}
impl Step for FindCardCrop {
    fn name(&self) -> &'static str {
        "find_card"
    }
    fn process(&self, data: PipelineData) -> Result<PipelineData> {
        let closed = data.into_luma8()?;
        let labels = connected_components(&closed, Connectivity::Eight, Luma([0u8]));
        let (lw, lh) = labels.dimensions();
        let mut extents = HashMap::new();
        for y in 0..lh {
            for x in 0..lw {
                let lab = labels.get_pixel(x, y)[0];
                if lab == 0 {
                    continue;
                }
                let e = extents.entry(lab).or_insert((x, y, x, y));
                e.0 = e.0.min(x);
                e.1 = e.1.min(y);
                e.2 = e.2.max(x);
                e.3 = e.3.max(y);
            }
        }
        let card_rect = extents
            .values()
            .map(|&(minx, miny, maxx, maxy)| {
                imageproc::rect::Rect::at(minx as i32, miny as i32)
                    .of_size(maxx - minx + 1, maxy - miny + 1)
            })
            .filter(|r| {
                let ar = r.width() as f32 / r.height() as f32;
                (3.0..5.0).contains(&ar)
            })
            .max_by_key(|r| r.width() * r.height())
            .ok_or(anyhow!("no card found"))?;
        let cropped = image::imageops::crop_imm(
            &self.orig,
            card_rect.left() as u32,
            card_rect.top() as u32,
            card_rect.width(),
            card_rect.height(),
        )
        .to_image();
        Ok(PipelineData::Rgb8(cropped))
    }
}

pub struct GrayscaleThresh;
impl Step for GrayscaleThresh {
    fn name(&self) -> &'static str {
        "gray_thresh"
    }
    fn process(&self, data: PipelineData) -> Result<PipelineData> {
        let rgb = data.into_rgb8()?;
        let mut gray = DynamicImage::ImageRgb8(rgb).into_luma8();
        let thresh = otsu_level(&gray);
        threshold_mut(&mut gray, thresh, ThresholdType::Binary);
        Ok(PipelineData::Luma8(gray))
    }
}

pub struct CropRegion;
impl Step for CropRegion {
    fn name(&self) -> &'static str {
        "crop_region"
    }
    fn process(&self, data: PipelineData) -> Result<PipelineData> {
        let img = data.into_luma8()?;
        let (cw, ch) = (img.width(), img.height());
        let crop = image::imageops::crop_imm(&img, cw / 4, ch / 2, cw / 2, ch / 12 * 5).to_image();
        Ok(PipelineData::Luma8(crop))
    }
}

pub struct EncodePng;
impl Step for EncodePng {
    fn name(&self) -> &'static str {
        "encode_png"
    }
    fn process(&self, data: PipelineData) -> Result<PipelineData> {
        let img = data.into_luma8()?;
        let mut buf = Vec::new();
        {
            let cursor = Cursor::new(&mut buf);
            PngEncoder::new(cursor).write_image(
                img.as_raw(),
                img.width(),
                img.height(),
                image::ExtendedColorType::L8,
            )?;
        }
        Ok(PipelineData::PngBytes(buf))
    }
}

pub struct OcrText;
impl Step for OcrText {
    fn name(&self) -> &'static str {
        "ocr"
    }
    fn process(&self, data: PipelineData) -> Result<PipelineData> {
        let bytes = data.into_bytes()?;
        let mut tess = LepTess::new(None, "eng")?;
        tess.set_image_from_mem(&bytes)?;
        tess.set_variable(Variable::TesseditPagesegMode, "7")?;
        tess.set_variable(Variable::TesseditCharWhitelist, "0123456789:.")?;
        let text = tess.get_utf8_text()?.trim().to_string();
        Ok(PipelineData::Text(text))
    }
}

/// Helper: HSV-based yellow test
fn is_yellow((r, g, b): (u8, u8, u8)) -> bool {
    const H_RANGE: (f32, f32) = (40.0, 65.0);
    const MIN_S: f32 = 0.35;
    const MIN_V: f32 = 0.70;
    let (rf, gf, bf) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let delta = max - min;
    if delta < f32::EPSILON {
        return false;
    }
    let mut h = if (max - rf).abs() < f32::EPSILON {
        (gf - bf) / delta
    } else if (max - gf).abs() < f32::EPSILON {
        (bf - rf) / delta + 2.0
    } else {
        (rf - gf) / delta + 4.0
    } * 60.0;
    if h < 0.0 {
        h += 360.0;
    }
    let s = delta / max;
    let v = max;
    (h >= H_RANGE.0 && h <= H_RANGE.1) && s >= MIN_S && v >= MIN_V
}
