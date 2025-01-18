use serde::{Deserialize, Serialize};
use reqwest::Client;
use tokio;
use anyhow::{Result, anyhow}; 
use log::{info, error, debug, LevelFilter};
use env_logger::Builder;
use std::collections::HashMap;
use image::{ImageBuffer, Rgb};
use std::fs;
use chrono::{Local, Utc};
use clap::Parser;
use std::time::Duration;
use tokio::time::sleep;

const BOARD_SIZE: usize = 200;
const MAX_PIXELS_PER_BATCH: usize = 10;
const BATCH_DELAY_MINUTES: u64 = 31;
const MAX_RETRIES: u32 = 10;
const RETRY_DELAY: Duration = Duration::from_secs(120); // 2 minutes

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

#[derive(Deserialize, Debug)]
struct TimerResponse {
    timers: Vec<String>,
    message: Option<String>,
}

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
        let mut retries = 0;

        loop {
            debug!("Requesting board from URL: {}", url);
            
            match self.client.get(&url).send().await {
                Ok(response) => {
                    debug!("Response status: {}", response.status());

                    if response.status() == reqwest::StatusCode::BAD_GATEWAY {
                        if retries >= MAX_RETRIES {
                            error!("Max retries ({}) reached for 502 error, stopping script", MAX_RETRIES);
                            return Err(anyhow!("Failed to connect after {} retries", MAX_RETRIES));
                        }

                        retries += 1;
                        info!("Received 502 Bad Gateway (attempt {}/{}), waiting {} seconds before retry", 
                            retries, MAX_RETRIES, RETRY_DELAY.as_secs());
                        sleep(RETRY_DELAY).await;
                        continue;
                    }

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

                    let mut rotated_matrix = vec![vec![0u8; BOARD_SIZE]; BOARD_SIZE];
                    for y in 0..BOARD_SIZE {
                        for x in 0..BOARD_SIZE {
                            rotated_matrix[x][BOARD_SIZE - 1 - y] = board_matrix[y][x];
                        }
                    }

                    let mut final_matrix = vec![vec![0u8; BOARD_SIZE]; BOARD_SIZE];
                    for y in 0..BOARD_SIZE {
                        for x in 0..BOARD_SIZE {
                            final_matrix[y][BOARD_SIZE - 1 - x] = rotated_matrix[y][x];
                        }  
                    }

                    info!("Board matrix constructed successfully");
                    return Ok((colors, final_matrix));
                },
                Err(e) => {
                    if retries >= MAX_RETRIES {
                        error!("Max retries ({}) reached for connection error, stopping script", MAX_RETRIES);
                        return Err(anyhow!("Failed to connect after {} retries: {}", MAX_RETRIES, e));
                    }

                    retries += 1;
                    error!("Connection error (attempt {}/{}): {}", retries, MAX_RETRIES, e);
                    info!("Waiting {} seconds before retry", RETRY_DELAY.as_secs());
                    sleep(RETRY_DELAY).await;
                    continue;
                }
            }
        }
    }

    fn calculate_wait_interval(&self, response: &str) -> Result<Duration> {
        let timer_response: TimerResponse = serde_json::from_str(response)?;
        let mut earliest_available = None;

        println!("=====================================================");
        println!("{:?}", timer_response.timers);
        println!("=====================================================");

        for timer in timer_response.timers {
            if let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(&timer) {
                let utc_timestamp = timestamp.with_timezone(&chrono::Utc);

                info!("Pixel will be available at: {}", utc_timestamp.format("%H:%M:%S"));

                if let Some(current_earliest) = earliest_available {
                    if utc_timestamp < current_earliest {
                        earliest_available = Some(utc_timestamp);
                    }
                } else {
                    earliest_available = Some(utc_timestamp);
                }
            }
        }

        if let Some(available_time) = earliest_available {
            let now = Utc::now();
            if available_time > now {
                let wait_duration = available_time.signed_duration_since(now);
                let total_seconds = wait_duration.num_seconds() as u64;
                let minutes = total_seconds / 60;
                let seconds = total_seconds % 60;

                info!("Current time: {}", now.format("%H:%M:%S"));
                info!("Target time: {}", available_time.format("%H:%M:%S"));
                info!("Need to wait: {}m {}s until first pixel is available", minutes, seconds);

                // Add 1 second buffer to ensure we're past the timeout
                return Ok(Duration::from_secs(total_seconds + 1));
            }
        }

        // If it goes wrong try the old method and wait 31 minutes
        Ok(Duration::from_secs(BATCH_DELAY_MINUTES * 60))
    }

    async fn place_pixel(&self, auth: &mut Auth, x: u32, y: u32, color_id: u8, colors: &HashMap<u8, Color>) -> Result<(bool, Option<Duration>)> {
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
            .header("Cookie", format!("refresh={}; token={}", auth.refresh_token, auth.token))
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
            return Ok((true, None));
        } 

        if !status.is_success() {
            let timer_response: Result<TimerResponse, _> = serde_json::from_str(&response_text);
            
            if let Ok(timer_response) = timer_response {
                if timer_response.message.as_deref() == Some("Too early") {
                    let wait_duration = self.calculate_wait_interval(&response_text)?;
                    info!("Waiting for {:?} before retrying", wait_duration);
                    return Ok((false, Some(wait_duration)));
                }
            }
            return Err(anyhow!("Request failed with status: {} - {}", status, response_text));
        }

        // Pour les réponses réussies, on extrait aussi les timers
        let timer_response: Result<TimerResponse, _> = serde_json::from_str(&response_text);
        let mut wait_duration = None;
        if let Ok(timer_response) = timer_response {
            if !timer_response.timers.is_empty() {
                wait_duration = Some(self.calculate_wait_interval(&response_text)?);
                info!("Next pixel available in {:?}", wait_duration);
            }
        }

        info!("Successfully placed pixel at ({}, {}) with color id {}", x, y, color_id);
        Ok((false, wait_duration))
    }

    async fn process_pattern(&self, 
        auth: &mut Auth,
        pattern: &Pattern,
        start_x: u32,
        start_y: u32,
        board: &Vec<Vec<u8>>,
        colors: &HashMap<u8, Color>,
        max_pixels: usize
    ) -> Result<(usize, Option<Duration>)> {
        let mut pixels_placed = 0;
        let mut wait_duration = None;

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
                        Ok((needs_refresh, new_wait_duration)) => {
                            if needs_refresh {
                                info!("Retrying with new tokens");
                                continue;
                            }
                            
                            // Mise à jour du temps d'attente si besoin
                            if let Some(duration) = new_wait_duration {
                                match wait_duration {
                                    None => wait_duration = Some(duration),
                                    Some(current) => {
                                        if duration < current {
                                            wait_duration = Some(duration);
                                        }
                                    }
                                }
                            }
                            
                            info!("Successfully placed pixel at ({}, {})", target_x, target_y);
                            pixels_placed += 1;
                            break;
                        },
                        Err(e) => {
                            error!("Failed to place pixel: {}", e);
                            retries += 1;
                            if retries >= max_retries {
                                error!("Max retries reached for pixel ({}, {}), skipping", target_x, target_y);
                                break;
                            }
                        }
                    }
                    sleep(Duration::from_millis(500)).await;
                }
            } else {
                debug!("Pixel at ({}, {}) already has correct color {}", target_x, target_y, p.color);
            }
        }

        Ok((pixels_placed, wait_duration))
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