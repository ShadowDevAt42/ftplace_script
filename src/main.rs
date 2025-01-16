use serde::Deserialize;
use reqwest::Client;
use tokio;
use anyhow::{Result, anyhow}; 
use log::{info, error, debug, LevelFilter};
use env_logger::Builder;
use std::collections::HashMap;
use image::{ImageBuffer, Rgb};
use std::fs;
use chrono::Local;

const BOARD_SIZE: usize = 200;

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

       // Initialiser la matrice initiale
       let mut board_matrix = vec![vec![0u8; BOARD_SIZE]; BOARD_SIZE];
       
       // Remplir la matrice initiale
       for (y, row) in board_data.board.iter().enumerate() {
           for (x, pixel) in row.iter().enumerate() {
               board_matrix[y][x] = pixel.color_id;
               if let Some(color) = colors.get(&pixel.color_id) {
                   debug!("Set pixel ({}, {}) to color {} ({})", x, y, color.name, pixel.username);
               }
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
}

#[tokio::main]
async fn main() -> Result<()> {
   Builder::new()
       .filter_level(LevelFilter::Debug)
       .format_timestamp_millis()
       .init();

   info!("Starting Place client");

   fs::create_dir_all("map")?;
   
   let now = Local::now();
   let timestamp = now.format("%Y-%m-%d_%H-%M-%S");
   
   let client = PlaceClient::new()?;
   
   info!("Attempting to get board data");
   match client.get_board().await {
       Ok((colors, board_matrix)) => {
           info!("Successfully retrieved board data");
           
           let mut color_info = String::new();
           for (id, color) in &colors {
               color_info.push_str(&format!("Color {}: {} (RGB: {},{},{})\n", 
                   id, color.name, color.red, color.green, color.blue));
           }
           fs::write(format!("map/colors_{}.txt", timestamp), color_info)?;
           
           let mut board_output = String::new();
           for row in board_matrix.iter() {
               for color_id in row {
                   board_output.push_str(&format!("{:2} ", color_id));
               }
               board_output.push('\n');
           }
           fs::write(format!("map/board_{}.txt", timestamp), board_output)?;
           
           let mut img = ImageBuffer::new(BOARD_SIZE as u32, BOARD_SIZE as u32);

           for (y, row) in board_matrix.iter().enumerate() {
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
           info!("Board image saved with timestamp {}", timestamp);
           
           info!("Board data, color information and PNG image saved to map folder");
       },
       Err(e) => {
           error!("Failed to get board data: {}", e);
           std::process::exit(1);
       }
   }

   Ok(())
}