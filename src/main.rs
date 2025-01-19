mod place_client;
mod config;
mod args_parser;

use tokio;
use anyhow::Result;
use log::{info, LevelFilter};
use env_logger::Builder;
use image::{ImageBuffer, Rgb};
use chrono::{Local, Utc};
use tokio::time::sleep;
use clap::Parser;

use std::{
    fs,
    collections::HashMap,
    process::exit,
    time::Duration,
};

use args_parser::{
    parse_patterns,
    Args,
    ArgSpecs
};

use config::{
    MAX_PIXELS_PER_BATCH,
    BATCH_DELAY_MINUTES,
    MAX_RETRIES,
    RETRY_DELAY,
    BOARD_SIZE,
};

use place_client::{
    Color,
    PlaceClient,
    Pattern,
    Auth,
};

fn save_board_state(colors: &HashMap<u8, Color>, board: &Vec<Vec<u8>>, timestamp: &str) -> Result<()> {
    // Créer un fichier avec la correspondance des couleurs
    let mut color_info = String::new();
    for (id, color) in colors {
        color_info.push_str(&format!("Color {}: {} (RGB: {},{},{})\n", 
            id, color.name, color.red, color.green, color.blue));
    }
    fs::write(format!("map/colors_{}.txt", timestamp), color_info)?;
    
    // Sauvegarder la matrice
    let mut board_output = String::new();
    for row in board.iter() {
        for color_id in row {
            board_output.push_str(&format!("{:2} ", color_id));
        }
        board_output.push('\n');
    }
    fs::write(format!("map/board_{}.txt", timestamp), board_output)?;
    
    // Créer l'image PNG
    let mut img = ImageBuffer::new(BOARD_SIZE as u32, BOARD_SIZE as u32);
    for (y, row) in board.iter().enumerate() {
        for (x, &color_id) in row.iter().enumerate() {
            if let Some(color) = colors.get(&color_id) {
                img.put_pixel(
                    x as u32,
                    y as u32,
                    Rgb([color.red, color.green, color.blue])
                ); 
            }
        }
    }
    img.save(format!("map/board_{}.png", timestamp))?;
    info!("Board data saved to map folder with timestamp {}", timestamp);
    Ok(())
}



#[tokio::main]
async fn main() -> Result<()> {
    Builder::new()
        .filter_level(LevelFilter::Debug)
        .format_timestamp_millis()
        .init();

    info!("Starting Place client with multiple patterns support");

    let args = Args::parse();
    // Get pattern path, x, y, and priority into a vector
    let mut patterns: Vec<ArgSpecs> = args.patterns
        .iter()
        .filter_map(|pattern| {
            match parse_patterns(pattern) {
                Ok(pattern) => Some(pattern),
                Err(e) => {
                    eprintln!("Error parsing pattern: {} {}", e, pattern);
                    exit(1);
                }
            }
        })
        .collect();

    patterns.sort();

    fs::create_dir_all("map")?;

    let client = PlaceClient::new()?;
    let mut auth = Auth {
        refresh_token: args.refresh_token,
        token: args.token,
    };

    let mut next_update = Utc::now();

    loop {
        let mut total_pixels_placed = 0;
        let mut wait_duration = None;

        for (_, pattern) in patterns.iter().enumerate() {
            let pattern_content = fs::read_to_string(&pattern.pattern_path).unwrap();
            let pattern_json: Pattern = serde_json::from_str(&pattern_content)
                .expect("Couldn't deserilize into");

            if Utc::now() >= next_update {
                //WARN: this could go wrong if the local time is not sync
                let now = Local::now();
                let timestamp = now.format("%Y-%m-%d_%H-%M-%S").to_string();

                let (colors, board) = client.get_board().await?;
                save_board_state(&colors, &board, &timestamp)?;

                let (defensive1_pixels, def1_wait) = client.process_pattern(
                    &mut auth,
                    &pattern_json,
                    pattern.x,
                    pattern.y,
                    &board,
                    MAX_PIXELS_PER_BATCH
                ).await?;

                total_pixels_placed += defensive1_pixels;
                if let Some(duration) = def1_wait {
                    wait_duration = Some(duration);
                }

                if total_pixels_placed < MAX_PIXELS_PER_BATCH {
                    continue;
                } else {
                    next_update = Utc::now() + if let Some(duration) = wait_duration {
                        chrono::Duration::from_std(duration)?
                    } else {
                        chrono::Duration::minutes(BATCH_DELAY_MINUTES as i64)
                    };
                    break;
                }
            }
        }
        let wait_time = next_update.signed_duration_since(Utc::now());
        if wait_time.num_seconds() > 0 {
            let mins = wait_time.num_minutes();
            let secs = wait_time.num_seconds() % 60;
            info!("Remaining time: {}m {}s", mins, secs);
            sleep(Duration::from_secs(10)).await;  // Update every 10 seconds
        }
    }
}
