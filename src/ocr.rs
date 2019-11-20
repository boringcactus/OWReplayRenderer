use image::{GrayImage, ImageFormat, GenericImageView};
use std::collections::HashMap;
use image::imageops::{grayscale, invert};
use imageproc::template_matching::{match_template, MatchTemplateMethod};
use imageproc::map::map_subpixels;
use imageproc::contrast::threshold_mut;

fn load_char(buf: &[u8]) -> GrayImage {
    let image = image::load_from_memory_with_format(buf, ImageFormat::PNG).unwrap();
    let mut result = grayscale(&image);
    invert(&mut result);
    result
}

fn get_chars() -> HashMap<char, GrayImage> {
    let mut result = HashMap::new();
    result.insert('0', load_char(include_bytes!("../templates/0.png")));
    result.insert('1', load_char(include_bytes!("../templates/1.png")));
    result.insert('2', load_char(include_bytes!("../templates/2.png")));
    result.insert('3', load_char(include_bytes!("../templates/3.png")));
    result.insert('4', load_char(include_bytes!("../templates/4.png")));
    result.insert('5', load_char(include_bytes!("../templates/5.png")));
    result.insert('6', load_char(include_bytes!("../templates/6.png")));
    result.insert('7', load_char(include_bytes!("../templates/7.png")));
    result.insert('8', load_char(include_bytes!("../templates/8.png")));
    result.insert('9', load_char(include_bytes!("../templates/9.png")));
    result.insert(':', load_char(include_bytes!("../templates/colon.png")));
    result.insert('\'', load_char(include_bytes!("../templates/apostrophe.png")));
    result.insert('A', load_char(include_bytes!("../templates/A.png")));
    result.insert('B', load_char(include_bytes!("../templates/B.png")));
    result.insert('C', load_char(include_bytes!("../templates/C.png")));
    result.insert('D', load_char(include_bytes!("../templates/D.png")));
    result.insert('E', load_char(include_bytes!("../templates/E.png")));
//    result.insert('F', load_char(include_bytes!("../templates/F.png")));
    result.insert('G', load_char(include_bytes!("../templates/G.png")));
    result.insert('H', load_char(include_bytes!("../templates/H.png")));
    result.insert('I', load_char(include_bytes!("../templates/I.png")));
//    result.insert('J', load_char(include_bytes!("../templates/J.png")));
    result.insert('K', load_char(include_bytes!("../templates/K.png")));
    result.insert('L', load_char(include_bytes!("../templates/L.png")));
    result.insert('M', load_char(include_bytes!("../templates/M.png")));
    result.insert('N', load_char(include_bytes!("../templates/N.png")));
    result.insert('O', load_char(include_bytes!("../templates/O.png")));
    result.insert('P', load_char(include_bytes!("../templates/P.png")));
//    result.insert('Q', load_char(include_bytes!("../templates/Q.png")));
    result.insert('R', load_char(include_bytes!("../templates/R.png")));
    result.insert('S', load_char(include_bytes!("../templates/S.png")));
    result.insert('T', load_char(include_bytes!("../templates/T.png")));
    result.insert('U', load_char(include_bytes!("../templates/U.png")));
//    result.insert('V', load_char(include_bytes!("../templates/V.png")));
    result.insert('W', load_char(include_bytes!("../templates/W.png")));
//    result.insert('X', load_char(include_bytes!("../templates/X.png")));
    result.insert('Y', load_char(include_bytes!("../templates/Y.png")));
    result.insert('Z', load_char(include_bytes!("../templates/Z.png")));
    result
}

lazy_static! {
    static ref CHAR_TEMPLATES: HashMap<char, GrayImage> = { get_chars() };
}

const THRESHOLD: f32 = 0.95;

/// This is a horrendously inefficient OCR algorithm that only even remotely works because we know
/// the exact characters being used
pub fn ocr(mut text: GrayImage) -> String {
    let mut result = String::new();
    let (width, height) = text.dimensions();
    // map from x-coordinate to (score, char)
    let x_char_options: HashMap<u32, Vec<(f32, char)>> = {
        let mut options: HashMap<u32, Vec<(f32, char)>> = HashMap::new();
        for (ch, template) in CHAR_TEMPLATES.iter() {
            let result = match_template(&text, template, MatchTemplateMethod::CrossCorrelationNormalized);
            for (x, y, score) in result.enumerate_pixels() {
                if score[0] > THRESHOLD {
                    options.entry(x).or_default().push((score[0], *ch));
                }
            }
        }
        for list in options.values_mut() {
            list.sort_by(|(score1, _), (score2, _)| score1.partial_cmp(score2).unwrap());
        }
        options
    };
    let mut x = 0;
    while x < width {
        if let Some(options) = x_char_options.get(&x) {
            let best_choice = options[0].1;
            result.push(best_choice);
            x += CHAR_TEMPLATES[&best_choice].dimensions().0;
        } else {
            x += 1;
        }
    }
    dbg!(&result);
    result
}
