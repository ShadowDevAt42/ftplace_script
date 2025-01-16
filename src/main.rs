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
   start_x: u32,

   #[arg(long)]
   start_y: u32,

   #[arg(long)]
   pattern_file: String,
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

   info!("Starting Place client");

   let args = Args::parse();
   let pattern_content = fs::read_to_string(&args.pattern_file)?;
   let pattern: Pattern = serde_json::from_str(&pattern_content)?;

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

       let mut pixels_placed = 0;

       for p in &pattern.pattern {
           if pixels_placed >= MAX_PIXELS_PER_BATCH {
               break;
           }

           let target_x = args.start_x + p.x;
           let target_y = args.start_y + p.y;
           
           if target_x >= BOARD_SIZE as u32 || target_y >= BOARD_SIZE as u32 {
               error!("Pattern point ({}, {}) out of bounds", target_x, target_y);
               continue;
           }

           if board[target_y as usize][target_x as usize] != p.color {
               let mut retries = 0;
               let max_retries = 3;

               while retries < max_retries {
                   match client.place_pixel(&mut auth, target_x, target_y, p.color, &colors).await {
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
                               info!("Received 'Too early' error, waiting {} minutes before retrying", BATCH_DELAY_MINUTES);
                               sleep(Duration::from_secs(BATCH_DELAY_MINUTES * 60)).await;
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
                   sleep(Duration::from_secs(1)).await;
               }
           } else {
               debug!("Pixel at ({}, {}) already has correct color {}", target_x, target_y, p.color);
           }

           sleep(Duration::from_secs(1)).await;
       }

       info!("Placed {} pixels in this batch", pixels_placed);
       info!("Waiting {} minutes before next batch", BATCH_DELAY_MINUTES);
       sleep(Duration::from_secs(BATCH_DELAY_MINUTES * 60)).await;
   }
}