use std::{
    error::Error,
    fs::File,
    io::{BufReader, Read, Seek},
    path::Path,
};

use image::{self, ImageFormat};
use img_hash::HasherConfig;

pub async fn visit(path: &dyn AsRef<Path>) {
    use async_walkdir::WalkDir;
    use futures_lite::stream::StreamExt;

    let mut entries = WalkDir::new(path);
    loop {
        match entries.next().await {
            Some(Ok(entry)) => {
                match read_image_attributes(&entry.path().into_os_string().into_string().unwrap()) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("error: {} on {:?}", e, entry);
                    }
                }
            }
            Some(Err(e)) => {
                eprintln!("walk error: {}", e);
            }
            None => break,
        }
    }
}

pub fn visit_sync(path: &dyn AsRef<Path>) {
    use walkdir::WalkDir;

    for entry in WalkDir::new(path) {
        //println!("{}", entry.unwrap().path().display());
        let entry = entry.as_ref();
        match read_image_attributes(&entry.unwrap().path().to_string_lossy().into_owned()) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("error: {} on {:?}", e, entry);
            }
        }
    }
}

fn read_image_attributes(path: &dyn AsRef<Path>) -> Result<(), Box<dyn Error>> {
    let mut reader = BufReader::new(File::open(path)?);

    // Two in one go: get img dimensions instead of looking at magic numbers first.
    let dim = imagesize::reader_size(reader.by_ref())?;

    // Retrieve image contents.
    reader.rewind()?;
    let img = image::load(reader.by_ref(), ImageFormat::from_path(path)?)?;
    //let img = image::open(path)?;

    // Get perceptual hash.
    let hasher = HasherConfig::new().to_hasher();
    let hash = hasher.hash_image(&img);

    // Use what we got.
    println!("{:?} {:02x?} in {}", dim, hash, path.as_ref().display());
    Ok(())
}
