use std::{error::Error, fs::File, io::BufReader, path::Path};

use imagesize::reader_size;

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
                        eprintln!("error: {}", e);
                    }
                }
            }
            Some(Err(e)) => {
                eprintln!("error: {}", e);
            }
            None => break,
        }
    }
}

fn read_image_attributes(path: &dyn AsRef<Path>) -> Result<(), Box<dyn Error>> {
    let reader = BufReader::new(File::open(path)?);

    // Two in one go: get img dimensions instead of looking at magic numbers first.
    let dim = reader_size(reader)?;

    // Use what we got.
    println!("{:?} in {}", dim, path.as_ref().display());
    Ok(())
}
