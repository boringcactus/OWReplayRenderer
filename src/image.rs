use image::png::PNGDecoder;
use image::{ImageFormat, RgbImage, GenericImageView, SubImage, Rgb};
use crate::ReplaySpec;
use crate::ocr::ocr;
use std::time::Duration;
use std::marker::PhantomData;
use image::Pixel;
use image::imageops::{grayscale, invert};
use imageproc::stats::histogram;

pub trait OWContext {}

pub struct ReplaysMenu;

impl OWContext for ReplaysMenu {}

fn dump(image: &RgbImage, tag: &str) {
    image.save(format!("temp_{}.png", tag)).unwrap();
}

pub struct Screenshot<C: OWContext> {
    data: RgbImage,
    marker: PhantomData<C>,
}

impl<C: OWContext> Screenshot<C> {
    pub fn new(data_uri: &str) -> Screenshot<C> {
        const PRELUDE: &'static str = "data:image/png;base64,";
        assert!(data_uri.starts_with(PRELUDE), "Image data poorly specified!");
        let data = &data_uri[PRELUDE.len()..];
        let data = base64::decode(data).expect("Image data poorly specified!");
        let image = image::load_from_memory_with_format(&data, ImageFormat::PNG).expect("Image data poorly specified!");
        let image = image.to_rgb();
        Screenshot {
            data: image,
            marker: PhantomData,
        }
    }

    pub fn dump(&self, tag: &str) {
        dump(&self.data, tag);
    }
}

#[derive(Debug)]
pub struct Replay {
    game_type: String,
    map: String,
    duration: Duration,
    played: String,
    outcome: String,
}

// uses Manhattan distance
fn mean_color_distance(img: &SubImage<&RgbImage>, color: &Rgb<u8>) -> f32 {
    let (sum, count) = img.pixels().map(|x| {
        let x_color: &Rgb<u8> = &x.2;
        let deltas = color.map2(x_color, |x1, x2| {
            if x1 > x2 {
                x1 - x2
            } else {
                x2 - x1
            }
        });
        let delta: f32 = deltas.channels().iter().map(|x| *x as f32).sum();
        delta
    }).fold((0.0, 0), |(sum, count), new| (sum + new, count + 1));
    sum / (count as f32)
}

fn get_duration(duration: SubImage<&RgbImage>, index: u32) -> Duration {
    let duration = duration.view(0, 13, 100, 12);
    let mut duration = grayscale(&duration);
    invert(&mut duration);
    duration.save(format!("temp_{}_duration.png", index)).unwrap();
    let spec = ocr(duration);
    let pieces: Vec<&str> = spec.split(':').collect();
    let minutes: u64 = pieces[0].parse().unwrap();
    let seconds: u64 = pieces[1].parse().unwrap();
    Duration::from_secs(minutes * 60 + seconds)
}

fn get_game_type(game_type: SubImage<&RgbImage>) -> String {
    use std::collections::HashMap;
    lazy_static! {
        static ref TYPES: HashMap<Rgb<u8>, String> = {
            let mut types = HashMap::new();
            types.insert(Rgb([120, 120, 120]), "Custom Game".to_string());
            types.insert(Rgb([100, 175, 100]), "Arcade".to_string());
            types.insert(Rgb([70, 140, 200]), "Quick Play".to_string());
            types
        };
    }
    let mut colors: Vec<(f32, &Rgb<u8>)> = TYPES.keys()
        .map(|color| (mean_color_distance(&game_type, color), color))
        .collect();
    colors.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let (distance, color) = colors[0];
    if distance > 100.0 {
        "Unknown".to_string()
    } else {
        TYPES[color].clone()
    }
}

fn is_replay(row: &SubImage<&RgbImage>, index: u32) -> bool {
    // grayscale to let us histogram on value
    let image = grayscale(row);
    image.save(format!("temp_{}_row_grayscale.png", index));
    // build the histogram
    let histogram = histogram(&row.to_image());
    // get the count between 130 and 150
    let count: usize = histogram.channels[0][100..150].iter().sum::<u32>() as usize;
    // get the total count
    let total = row.pixels().count();
    // make sure that at least one third are included in that band
    count * 3 > total
}

fn get_replay(row: SubImage<&RgbImage>, index: u32) -> Option<Replay> {
    if is_replay(&row, index) {
        let game_type = row.view(0, 0, 250, 40);
        let game_type = get_game_type(game_type);
        let map = row.view(250, 0, 800, 40);
        let map = "Unknown".to_string();
        let duration = row.view(1050, 0, 250, 40);
        let duration = get_duration(duration, index);
        let played = "Unknown".to_string();
        let outcome = "Unknown".to_string();
        Some(Replay {
            game_type,
            map,
            duration,
            played,
            outcome,
        })
    } else {
        None
    }
}

impl Screenshot<ReplaysMenu> {
    pub fn get_replays(&self) -> Vec<Replay> {
        let data = &self.data;
        // This is wall-to-wall magic numbers, sorry about that.
        (1..=11).filter_map(|index| {
            let offset = (index - 1) * 40;
            let row = data.view(70, 428 + offset, 1780, 40);
            dump(&row.to_image(), &format!("{}_row", index));
            get_replay(row, index)
        }).collect()
    }
}
