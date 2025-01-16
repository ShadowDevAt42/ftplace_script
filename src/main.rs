use serde::Deserialize;
use reqwest::{Client, header};
use tokio;
use anyhow::{Result, anyhow}; 
use clap::Parser;
use log::{info, warn, error, debug, LevelFilter};
use env_logger::Builder;
use std::{thread, time::Duration};
use std::collections::HashMap;
use image::{ImageBuffer, Rgb};
use std::fs;  // Ajouté
use chrono::Local;  // Ajouté

const BOARD_SIZE: usize = 200;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    refresh_token: String,
    
    #[arg(long)]
    token: String,

    #[arg(long, default_value_t = 3)]
    max_retries: u32,

    #[arg(long, default_value_t = 2000)]
    retry_delay_ms: u64,
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
    auth: Auth,
    base_url: String,
    max_retries: u32,
    retry_delay: Duration,
}

impl PlaceClient {
    fn new(refresh_token: String, token: String, max_retries: u32, retry_delay_ms: u64) -> Result<Self> {
        debug!("Initializing PlaceClient with max_retries={}, retry_delay={}ms", max_retries, retry_delay_ms);
        
        let mut headers = header::HeaderMap::new();
        headers.insert("Accept", "application/json".parse()?);
        headers.insert("Accept-Language", "fr,fr-FR;q=0.8,en-US;q=0.5,en;q=0.3".parse()?);
        headers.insert("Content-Type", "application/json".parse()?);
        
        let client = Client::builder()
            .default_headers(headers)
            .build()?;

        info!("HTTP client initialized successfully");

        Ok(PlaceClient {
            client,
            auth: Auth { refresh_token, token },
            base_url: "https://ftplace.42lwatch.ch".to_string(),
            max_retries,
            retry_delay: Duration::from_millis(retry_delay_ms),
        })
    }

    async fn get_board(&mut self) -> Result<(HashMap<u8, Color>, Vec<Vec<u8>>)> {
        let mut retries = 0;
        
        loop {
            if retries > 0 {
                warn!("Retry attempt {} of {}", retries, self.max_retries);
                thread::sleep(self.retry_delay);
            }

            match self.try_get_board().await {
                Ok(data) => {
                    info!("Successfully retrieved board data");
                    return Ok(data);
                }
                Err(e) => {
                    error!("Error getting board: {}", e);
                    retries += 1;
                    
                    if retries >= self.max_retries {
                        return Err(anyhow!("Max retries ({}) exceeded", self.max_retries));
                    }
                }
            }
        }
    }

    async fn try_get_board(&mut self) -> Result<(HashMap<u8, Color>, Vec<Vec<u8>>)> {
        let url = format!("{}/api/get?type=board", self.base_url);
        debug!("Requesting board from URL: {}", url);
        
        let response = self.client
            .get(&url)
            .header("Cookie", format!(
                "refresh={}; token={}",
                self.auth.refresh_token,
                self.auth.token
            ))
            .header("Origin", &self.base_url)
            .header("Referer", &format!("{}/", self.base_url))
            .send()
            .await?;

        debug!("Response status: {}", response.status());

        if response.status() == 426 {
            info!("Token refresh required");
            self.handle_token_refresh(response.headers())?;
            return Box::pin(self.try_get_board()).await;
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
                if let Some(color) = colors.get(&pixel.color_id) {
                    debug!("Set pixel ({}, {}) to color {} ({})", x, y, color.name, pixel.username);
                }
            }
        }

        info!("Board matrix constructed successfully");
        Ok((colors, board_matrix))
    }

    fn handle_token_refresh(&mut self, headers: &header::HeaderMap) -> Result<()> {
        debug!("Processing token refresh from headers");
        
        let new_token = headers.get("Set-Cookie")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(';').next())
            .and_then(|s| s.strip_prefix("token="))
            .ok_or_else(|| anyhow!("No new token found"))?;

        let new_refresh = headers.get("Set-Cookie")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(';').next())
            .and_then(|s| s.strip_prefix("refresh="))
            .ok_or_else(|| anyhow!("No new refresh token found"))?;

        info!("Tokens successfully refreshed");
        debug!("New token: {}", new_token);
        debug!("New refresh token: {}", new_refresh);

        self.auth.token = new_token.to_string();
        self.auth.refresh_token = new_refresh.to_string();
        
        Ok(())
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
    
    let args = Args::parse();
    let mut client = PlaceClient::new(
        args.refresh_token,
        args.token,
        args.max_retries,
        args.retry_delay_ms,
    )?;
    
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
                            y as u32,
                            x as u32,
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