// Custom mods
mod place_client;

use serde::{Deserialize, Serialize};
use reqwest::Client;
use tokio;
use anyhow::{Result, anyhow}; 
use log::{info, error, debug, LevelFilter};
use env_logger::Builder;
use image::{ImageBuffer, Rgb};
use std::fs;
use chrono::{Local, Utc};
use clap::Parser;
use std::time::Duration;
use std::collections::HashMap;
use tokio::time::sleep;

use place_client::{
    Color,
    PlaceClient,
    Pattern,
    Auth,
};

// consts
const MAX_PIXELS_PER_BATCH: usize = 10;
pub const BATCH_DELAY_MINUTES: u64 = 31;
pub const MAX_RETRIES: u32 = 10;
pub const RETRY_DELAY: Duration = Duration::from_secs(120); // 2 minutes
pub const BOARD_SIZE: usize = 250;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    refresh_token: String,
    
    #[arg(long)]
    token: String,

    // Pattern obligatoire
    #[arg(long)]
    defensive1_x: u32,

    #[arg(long)]
    defensive1_y: u32,

    #[arg(long)]
    defensive1_pattern: String,

    // Patterns optionnels
    #[arg(long)]
    defensive2_x: Option<u32>,

    #[arg(long)]
    defensive2_y: Option<u32>,

    #[arg(long)]
    defensive2_pattern: Option<String>,

    #[arg(long)]
    build1_x: Option<u32>,

    #[arg(long)]
    build1_y: Option<u32>,

    #[arg(long)]
    build1_pattern: Option<String>,

    #[arg(long)]
    build2_x: Option<u32>,

    #[arg(long)]
    build2_y: Option<u32>,

    #[arg(long)]
    build2_pattern: Option<String>,

    #[arg(long)]
    build3_x: Option<u32>,

    #[arg(long)]
    build3_y: Option<u32>,

    #[arg(long)]
    build3_pattern: Option<String>,
}



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
    
    // Chargement du pattern obligatoire
    let defensive1_content = fs::read_to_string(&args.defensive1_pattern)?;
    let defensive1_pattern: Pattern = serde_json::from_str(&defensive1_content)?;
    
    // Chargement des patterns optionnels
    let defensive2_pattern = if let Some(pattern_path) = &args.defensive2_pattern {
        if let (Some(x), Some(y)) = (args.defensive2_x, args.defensive2_y) {
            let content = fs::read_to_string(pattern_path)?;
            Some((serde_json::from_str(&content)?, x, y))
        } else {
            None
        }
    } else {
        None
    };

    let build1_pattern = if let Some(pattern_path) = &args.build1_pattern {
        if let (Some(x), Some(y)) = (args.build1_x, args.build1_y) {
            let content = fs::read_to_string(pattern_path)?;
            Some((serde_json::from_str(&content)?, x, y))
        } else {
            None
        }
    } else {
        None
    };

    let build2_pattern = if let Some(pattern_path) = &args.build2_pattern {
        if let (Some(x), Some(y)) = (args.build2_x, args.build2_y) {
            let content = fs::read_to_string(pattern_path)?;
            Some((serde_json::from_str(&content)?, x, y))
        } else {
            None
        }
    } else {
        None
    };

    let build3_pattern = if let Some(pattern_path) = &args.build3_pattern {
        if let (Some(x), Some(y)) = (args.build3_x, args.build3_y) {
            let content = fs::read_to_string(pattern_path)?;
            Some((serde_json::from_str(&content)?, x, y))
        } else {
            None
        }
    } else {
        None
    };

    fs::create_dir_all("map")?;

    let client = PlaceClient::new()?;
    let mut auth = Auth {
        refresh_token: args.refresh_token,
        token: args.token,
    };

    let mut next_update = Utc::now();
    
    loop {
        if Utc::now() >= next_update {
            let now = Local::now();
            let timestamp = now.format("%Y-%m-%d_%H-%M-%S").to_string();

            let (colors, board) = client.get_board().await?;
            save_board_state(&colors, &board, &timestamp)?;

            let mut total_pixels_placed = 0;
            let mut wait_duration = None;
            
            // Traitement prioritaire du premier pattern défensif (obligatoire)
            let (defensive1_pixels, def1_wait) = client.process_pattern(
                &mut auth,
                &defensive1_pattern,
                args.defensive1_x,
                args.defensive1_y,
                &board,
                &colors,
                MAX_PIXELS_PER_BATCH
            ).await?;

            total_pixels_placed += defensive1_pixels;
            if let Some(duration) = def1_wait {
                wait_duration = Some(duration);
            }

            // Continue avec les pixels restants même si on a reçu un timer
            if total_pixels_placed < MAX_PIXELS_PER_BATCH {
                // Pattern défensif 2 (optionnel)
                if let Some((pattern, x, y)) = &defensive2_pattern {
                    let remaining_pixels = MAX_PIXELS_PER_BATCH - total_pixels_placed;
                    let (defensive2_pixels, def2_wait) = client.process_pattern(
                        &mut auth,
                        pattern,
                        *x,
                        *y,
                        &board,
                        &colors,
                        remaining_pixels
                    ).await?;

                    total_pixels_placed += defensive2_pixels;
                    if let Some(duration) = def2_wait {
                        if let Some(current) = wait_duration {
                            if duration < current {
                                wait_duration = Some(duration);
                            }
                        } else {
                            wait_duration = Some(duration);
                        }
                    }
                }
            }

            // Continue avec build1 s'il reste des pixels
            if total_pixels_placed < MAX_PIXELS_PER_BATCH {
                if let Some((pattern, x, y)) = &build1_pattern {
                    let remaining_pixels = MAX_PIXELS_PER_BATCH - total_pixels_placed;
                    let (build1_pixels, build1_wait) = client.process_pattern(
                        &mut auth,
                        pattern,
                        *x,
                        *y,
                        &board,
                        &colors,
                        remaining_pixels
                    ).await?;

                    total_pixels_placed += build1_pixels;
                    if let Some(duration) = build1_wait {
                        if let Some(current) = wait_duration {
                            if duration < current {
                                wait_duration = Some(duration);
                            }
                        } else {
                            wait_duration = Some(duration);
                        }
                    }
                }
            }

            // Continue avec build2 s'il reste des pixels
            if total_pixels_placed < MAX_PIXELS_PER_BATCH {
                if let Some((pattern, x, y)) = &build2_pattern {
                    let remaining_pixels = MAX_PIXELS_PER_BATCH - total_pixels_placed;
                    let (build2_pixels, build2_wait) = client.process_pattern(
                        &mut auth,
                        pattern,
                        *x,
                        *y,
                        &board,
                        &colors,
                        remaining_pixels
                    ).await?;

                    total_pixels_placed += build2_pixels;
                    if let Some(duration) = build2_wait {
                        if let Some(current) = wait_duration {
                            if duration < current {
                                wait_duration = Some(duration);
                            }
                        } else {
                            wait_duration = Some(duration);
                        }
                    }
                }
            }

            // Enfin, continue avec build3 s'il reste des pixels
            if total_pixels_placed < MAX_PIXELS_PER_BATCH {
                if let Some((pattern, x, y)) = &build3_pattern {
                    let remaining_pixels = MAX_PIXELS_PER_BATCH - total_pixels_placed;
                    let (build3_pixels, build3_wait) = client.process_pattern(
                        &mut auth,
                        pattern,
                        *x,
                        *y,
                        &board,
                        &colors,
                        remaining_pixels
                    ).await?;

                    total_pixels_placed += build3_pixels;
                    if let Some(duration) = build3_wait {
                        if let Some(current) = wait_duration {
                            if duration < current {
                                wait_duration = Some(duration);
                            }
                        } else {
                            wait_duration = Some(duration);
                        }
                    }
                }
            }

            info!("Placed {} pixels in total this batch", total_pixels_placed);
            
            // Utilise le plus petit timer reçu ou le délai par défaut
            next_update = Utc::now() + if let Some(duration) = wait_duration {
                chrono::Duration::from_std(duration)?
            } else {
                chrono::Duration::minutes(BATCH_DELAY_MINUTES as i64)
            };
        }

        // Afficher le temps restant
        let wait_time = next_update.signed_duration_since(Utc::now());
        if wait_time.num_seconds() > 0 {
            let mins = wait_time.num_minutes();
            let secs = wait_time.num_seconds() % 60;
            info!("Remaining time: {}m {}s", mins, secs);
            sleep(Duration::from_secs(10)).await;  // Update every 10 seconds
        }
    }
}
