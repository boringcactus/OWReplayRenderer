use image::imageops::grayscale;
use image::Pixel;
use image::{GenericImageView, GrayImage, ImageFormat, Rgb, RgbImage, SubImage};
use imageproc::geometric_transformations::{warp, Interpolation, Projection};
use imageproc::stats::histogram;
use imageproc::template_matching::{find_extremes, match_template, MatchTemplateMethod};
use std::marker::PhantomData;

pub trait OWContext {}

#[allow(dead_code)]
pub struct ReplaysMenu;
impl OWContext for ReplaysMenu {}

pub struct InReplay;
impl OWContext for InReplay {}

pub struct Screenshot<C: OWContext> {
    data: RgbImage,
    marker: PhantomData<C>,
}

impl<C: OWContext> Screenshot<C> {
    pub fn new(data_uri: &str) -> Screenshot<C> {
        const PRELUDE: &'static str = "data:image/png;base64,";
        assert!(
            data_uri.starts_with(PRELUDE),
            "Image data poorly specified!"
        );
        let data = &data_uri[PRELUDE.len()..];
        let data = base64::decode(data).expect("Image data poorly specified!");
        let image = image::load_from_memory_with_format(&data, ImageFormat::PNG)
            .expect("Image data poorly specified!");
        let image = image.to_rgb();
        Screenshot {
            data: image,
            marker: PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct Replay {
    game_type: String,
}

// uses Manhattan distance
fn mean_color_distance(img: &SubImage<&RgbImage>, color: &Rgb<u8>) -> f32 {
    let (sum, count) = img
        .pixels()
        .map(|x| {
            let x_color: &Rgb<u8> = &x.2;
            let deltas = color.map2(x_color, |x1, x2| if x1 > x2 { x1 - x2 } else { x2 - x1 });
            let delta: f32 = deltas.channels().iter().map(|x| *x as f32).sum();
            delta
        })
        .fold((0.0, 0), |(sum, count), new| (sum + new, count + 1));
    sum / (count as f32)
}

#[allow(dead_code)]
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
    let mut colors: Vec<(f32, &Rgb<u8>)> = TYPES
        .keys()
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

#[allow(dead_code)]
fn is_replay(row: &SubImage<&RgbImage>) -> bool {
    // grayscale to let us histogram on value
    let image = grayscale(row);
    // build the histogram
    let histogram = histogram(&image);
    // get the count between 130 and 150
    let count: usize = histogram.channels[0][100..150].iter().sum::<u32>() as usize;
    // get the total count
    let total = row.pixels().count();
    // make sure that at least one third are included in that band
    count * 3 > total
}

#[allow(dead_code)]
fn get_replay(row: SubImage<&RgbImage>) -> Option<Replay> {
    if is_replay(&row) {
        let game_type = row.view(0, 0, 250, 40);
        let game_type = get_game_type(game_type);
        Some(Replay { game_type })
    } else {
        None
    }
}

#[allow(dead_code)]
impl Screenshot<ReplaysMenu> {
    pub fn get_replays(&self) -> Vec<Replay> {
        let data = &self.data;
        // This is wall-to-wall magic numbers, sorry about that.
        (1..=11)
            .filter_map(|index| {
                let offset = (index - 1) * 40;
                let row = data.view(70, 428 + offset, 1780, 40);
                get_replay(row)
            })
            .collect()
    }
}

fn warp_username_badge(badge: &RgbImage) -> GrayImage {
    let transform = Projection::from_matrix([
        0.86979, 0.25266, -465.5, 0.07896, 1.00069, -885.0, 0.0, 0.0, 1.0,
    ])
    .unwrap();
    let badge = warp(&badge, &transform, Interpolation::Bicubic, Rgb([0, 0, 0]));
    let badge = badge.view(0, 0, 180, 40).to_image();
    grayscale(&badge)
}

impl Screenshot<InReplay> {
    pub fn has_me() -> bool {
        std::fs::metadata("username_badge.png").is_ok()
    }

    pub fn is_me_score(&self) -> f32 {
        let data = &self.data;
        let actual_name_badge = warp_username_badge(data);
        let expected_name_badge = image::open("username_badge.png").unwrap();
        let expected_name_badge = warp_username_badge(&expected_name_badge.to_rgb());
        let overlap = match_template(
            &actual_name_badge,
            &expected_name_badge,
            MatchTemplateMethod::CrossCorrelationNormalized,
        );
        let extremes = find_extremes(&overlap);
        extremes.max_value
    }

    pub fn is_gameover(&self) -> bool {
        let data = &self.data;
        // this only works bc the controls autoexpand on game end
        let data = data.view(1689, 948, 50, 14);
        let distance = mean_color_distance(&data, &Rgb([46, 181, 229]));
        distance < 3.0
    }

    // we measure with the middle of the pause button
    pub fn is_definitely_paused(&self) -> bool {
        let data = &self.data;
        let data = data.view(316, 997, 4, 15);
        let distance = mean_color_distance(&data, &Rgb([193, 193, 193]));
        distance < 10.0
    }
}
