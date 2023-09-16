//! Where the synchronous things are done.

use image::{imageops, DynamicImage};
use tracing::{event_enabled, trace, Level};

#[derive(Debug, strum::AsRefStr)]
pub enum Calculation {
    Dhash(u64),
    Thumbnail,
}

/// A type for functions to be transmitted
pub type CalcFn = for<'a> fn(&'a image::DynamicImage) -> Calculation;

/// The algorithm described in
/// <https://www.hackerfactor.com/blog/index.php?/archives/529-Kind-of-Like-That.html>
pub fn make_dhash(img: &DynamicImage) -> Calculation {
    let img = img.resize_exact(9, 8, imageops::FilterType::Nearest);
    let img = img.grayscale();

    let array = img.into_bytes();
    let mut dhash: u64 = 0;
    let mut e: u32 = 0;
    let base: u64 = 2;

    // tracing follies
    let mut details = String::from("\n");

    for i in 0..8 {
        let row = &array[i * 10 - i..(i + 1) * 10 - (i + 1)];

        if event_enabled!(Level::TRACE) {
            details.push_str(&format!("{row:3?}\t"));
        }

        for j in 0..8 {
            if row[j + 1] > row[j] {
                dhash += base.pow(e); // Set e-th bit to `true`.
            }

            if event_enabled!(Level::TRACE) {
                details.push_str(&format!(" {:>5}", row[j + 1] > row[j]));
            }

            // println!("{e:2} {dhash:b}");
            e += 1; // Increment exponent!
        }

        if event_enabled!(Level::TRACE) {
            details.push('\n');
        }
    }

    if event_enabled!(Level::TRACE) {
        trace!("{details}");
    }

    Calculation::Dhash(dhash)
}

/// For the moment this is a mock thing, costing only few CPU cycles.
pub fn make_thumbnail(_: &DynamicImage) -> Calculation {
    Calculation::Thumbnail
}
