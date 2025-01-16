use serde::{Deserialize, Serialize};
use reqwest::Client;
use tokio;
use anyhow::{Result, anyhow}; 
use log::{info, error, debug, LevelFilter};
use env_logger::Builder;
use std::collections::HashMap;
use image::{ImageBuffer, Rgb};
use std::fs;
use chrono::Local;
use clap::Parser;
use std::time::Duration;
use tokio::time::sleep;

const BOARD_SIZE: usize = 200;
const MAX_PIXELS_PER_BATCH: usize = 10;
const BATCH_DELAY_MINUTES: u64 = 31;

#[derive(Deserialize, Debug)]
struct Pattern {
    pattern: Vec<PatternPixel>,
}

#[derive(Deserialize, Debug)]
struct PatternPixel {
    x: u32,
    y: u32,
    color: u8,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    refresh_token: String,
    
    #[arg(long)]
    token: String,

    #[arg(long)]
    defensive_x: u32,

    #[arg(long)]
    defensive_y: u32,

    #[arg(long)]
    defensive_pattern: String,

    #[arg(long)]
    build_x: u32,

    #[arg(long)]
    build_y: u32,

    #[arg(long)]
    build_pattern: String,
}

#[derive(Serialize, Debug)]
struct PlacePixelRequest {
    x: u32,
    y: u32,
    color: String,
}

#[derive(Deserialize, Debug)]
struct Color {
    id: u8,
    name: String,
    red: u8,
    green: u8,
    blue: u8,
}

#[derive(Deserialize, Debug)]
struct Pixel {
    username: String,
    color_id: u8,
    set_time: String,
}

#[derive(Deserialize, Debug)]
struct BoardResponse {
    colors: Vec<Color>,
    #[serde(rename = "type")]
    response_type: String,
    board: Vec<Vec<Pixel>>,
}

#[derive(Debug, Clone)]
struct Auth {
    refresh_token: String,
    token: String,
}

struct PlaceClient {
    client: Client,
    base_url: String,
}

impl PlaceClient {
    fn new() -> Result<Self> {
        let client = Client::new();
        info!("HTTP client initialized successfully");

        Ok(PlaceClient {
            client,
            base_url: "https://ftplace.42lwatch.ch".to_string(),
        })
    }

    async fn get_board(&self) -> Result<(HashMap<u8, Color>, Vec<Vec<u8>>)> {
        let url = format!("{}/api/get?type=board", self.base_url);
        debug!("Requesting board from URL: {}", url);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;

        debug!("Response status: {}", response.status());

        if !response.status().is_success() {
            return Err(anyhow!("Request failed with status: {}", response.status()));
        }

        let board_data: BoardResponse = response.json().await?;
        
        let colors: HashMap<u8, Color> = board_data.colors
            .into_iter()
            .map(|c| (c.id, c))
            .collect();
        
        debug!("Loaded {} color definitions", colors.len());

        let mut board_matrix = vec![vec![0u8; BOARD_SIZE]; BOARD_SIZE];
        
        for (y, row) in board_data.board.iter().enumerate() {
            for (x, pixel) in row.iter().enumerate() {
                board_matrix[y][x] = pixel.color_id;
            }
        }

        // Rotation de 90 degrés vers la droite
        let mut rotated_matrix = vec![vec![0u8; BOARD_SIZE]; BOARD_SIZE];
        for y in 0..BOARD_SIZE {
            for x in 0..BOARD_SIZE {
                rotated_matrix[x][BOARD_SIZE - 1 - y] = board_matrix[y][x];
            }
        }

        // Miroir vertical 
        let mut final_matrix = vec![vec![0u8; BOARD_SIZE]; BOARD_SIZE];
        for y in 0..BOARD_SIZE {
            for x in 0..BOARD_SIZE {
                final_matrix[y][BOARD_SIZE - 1 - x] = rotated_matrix[y][x];
            }  
        }

        info!("Board matrix constructed successfully");
        Ok((colors, final_matrix))
    }

    async fn place_pixel(&self, auth: &mut Auth, x: u32, y: u32, color_id: u8, colors: &HashMap<u8, Color>) -> Result<bool> {
        let url = format!("{}/api/set", self.base_url);
        
        let request = PlacePixelRequest {
            x,
            y,
            color: color_id.to_string(),
        };

        debug!("Placing pixel at ({}, {}) with color id {}", x, y, color_id);

        let response = self.client
            .post(&url)
            .header("Accept", "application/json")
            .header("Accept-Encoding", "gzip, deflate, br, zstd")
            .header("Accept-Language", "fr,fr-FR;q=0.8,en-US;q=0.5,en;q=0.3")
            .header("Connection", "keep-alive")
            .header("Content-Type", "application/json")
            .header("Origin", &self.base_url)
            .header("Referer", format!("{}/?x={}&y={}&scale=1", self.base_url, x, y))
            .header("Sec-Fetch-Dest", "empty")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Site", "same-origin")
            .header("Cookie", format!(
                "refresh={}; token={}",
                auth.refresh_token, auth.token
            ))
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let headers = response.headers().clone();
        let response_text = response.text().await?;

        if status == 426 {
            info!("Token refresh required");
            
            for cookie in headers.get_all("set-cookie") {
                let cookie_str = cookie.to_str()?;
                
                if let Some(token_str) = cookie_str.split(';').next() {
                    if token_str.starts_with("token=") {
                        auth.token = token_str["token=".len()..].to_string();
                    } else if token_str.starts_with("refresh=") {
                        auth.refresh_token = token_str["refresh=".len()..].to_string();
                    }
                }
            }

            debug!("New tokens: refresh={}, token={}", auth.refresh_token, auth.token);
            return Ok(true);
        } 

        if !status.is_success() {
            let error_message: serde_json::Value = serde_json::from_str(&response_text)?;
            if let Some(message) = error_message.get("message") {
                if message.as_str() == Some("Too early") {
                    info!("Received 'Too early' error, waiting {} minutes before retrying", BATCH_DELAY_MINUTES);
                    return Err(anyhow!("Too early"));
                }
            }
            return Err(anyhow!("Request failed with status: {} - {}", status, response_text));
        }

        info!("Successfully placed pixel at ({}, {}) with color id {}", x, y, color_id);
        Ok(false)
    }

    async fn process_pattern(&self, 
        auth: &mut Auth,
        pattern: &Pattern,
        start_x: u32,
        start_y: u32,
        board: &Vec<Vec<u8>>,
        colors: &HashMap<u8, Color>,
        max_pixels: usize
    ) -> Result<(usize, bool)> {
        let mut pixels_placed = 0;
        let mut needs_wait = false;

        for p in &pattern.pattern {
            if pixels_placed >= max_pixels {
                break;
            }

            let target_x = start_x + p.x;
            let target_y = start_y + p.y;
            
            if target_x >= BOARD_SIZE as u32 || target_y >= BOARD_SIZE as u32 {
                error!("Pattern point ({}, {}) out of bounds", target_x, target_y);
                continue;
            }

            if board[target_y as usize][target_x as usize] != p.color {
                let mut retries = 0;
                let max_retries = 3;

                while retries < max_retries {
                    match self.place_pixel(auth, target_x, target_y, p.color, colors).await {
                        Ok(needs_refresh) => {
                            if needs_refresh {
                                info!("Retrying with new tokens");
                            } else {
                                info!("Successfully placed pixel at ({}, {})", target_x, target_y);
                                pixels_placed += 1;
                                break;
                            }
                        },
                        Err(e) => {
                            let error_message = e.to_string();
                            if error_message.contains("Too early") {
                                info!("Received 'Too early' error");
                                needs_wait = true;
                                break;
                            } else {
                                error!("Failed to place pixel: {}", e);
                                retries += 1;
                                if retries >= max_retries {
                                    error!("Max retries reached for pixel ({}, {}), skipping", target_x, target_y);
                                    break;
                                }
                            }
                        }
                    }
                    sleep(Duration::from_millis(500)).await;
                }

                if needs_wait {
                    break;
                }
            } else {
                debug!("Pixel at ({}, {}) already has correct color {}", target_x, target_y, p.color);
            }
        }

        Ok((pixels_placed, needs_wait))
    }
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
    
    // Chargement des deux patterns
    let defensive_content = fs::read_to_string(&args.defensive_pattern)?;
    let defensive_pattern: Pattern = serde_json::from_str(&defensive_content)?;
    
    let build_content = fs::read_to_string(&args.build_pattern)?;
    let build_pattern: Pattern = serde_json::from_str(&build_content)?;

    fs::create_dir_all("map")?;

    let client = PlaceClient::new()?;
    let mut auth = Auth {
        refresh_token: args.refresh_token,
        token: args.token,
    };

    loop {
        let now = Local::now();
        let timestamp = now.format("%Y-%m-%d_%H-%M-%S").to_string();

        let (colors, board) = client.get_board().await?;
        save_board_state(&colors, &board, &timestamp)?;

        let mut total_pixels_placed = 0;
        let mut needs_wait = false;

        // Traitement prioritaire du pattern défensif
        let (defensive_pixels, wait_needed) = client.process_pattern(
            &mut auth,
            &defensive_pattern,
            args.defensive_x,
            args.defensive_y,
            &board,
            &colors,
            MAX_PIXELS_PER_BATCH
        ).await?;

        total_pixels_placed += defensive_pixels;
        needs_wait = wait_needed;

        // Si on n'a pas utilisé tous nos pixels sur la défense et qu'on n'a pas besoin d'attendre,
        // on travaille sur le pattern de construction
        if !needs_wait && total_pixels_placed < MAX_PIXELS_PER_BATCH {
            let remaining_pixels = MAX_PIXELS_PER_BATCH - total_pixels_placed;
            let (build_pixels, wait_needed) = client.process_pattern(
                &mut auth,
                &build_pattern,
                args.build_x,
                args.build_y,
                &board,
                &colors,
                remaining_pixels
            ).await?;

            total_pixels_placed += build_pixels;
            needs_wait = wait_needed;
        }

        info!("Placed {} pixels in total this batch", total_pixels_placed);
        info!("Waiting {} minutes before next batch", BATCH_DELAY_MINUTES);
        sleep(Duration::from_secs(BATCH_DELAY_MINUTES * 60)).await;
    }
}