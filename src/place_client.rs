use reqwest::Client;
use anyhow::{Result, anyhow};
use log::{info, error, debug};
use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use chrono::Utc;
use crate::{
    BATCH_DELAY_MINUTES,
    MAX_RETRIES,
    RETRY_DELAY,
    BOARD_SIZE,
};

#[derive(Deserialize, Debug)]
struct TimerResponse {
    timers: Vec<String>,
    message: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Color {
    id: u8,
    pub(crate) name: String,
    pub(crate) red: u8,
    pub(crate) green: u8,
    pub(crate) blue: u8,
}

pub struct PlaceClient {
    client: Client,
    base_url: String,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct Pixel {
    username: String,
    color_id: u8,
    set_time: String,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct BoardResponse {
    colors: Vec<Color>,
    #[serde(rename = "type")]
    response_type: String,
    board: Vec<Vec<Pixel>>,
}

#[derive(Serialize, Debug)]
struct PlacePixelRequest {
    x: i32,
    y: i32,
    color: String,
}

#[derive(Debug, Clone)]
pub struct Auth {
    pub(crate) refresh_token: String,
    pub(crate) token: String,
}

#[derive(Deserialize, Debug)]
pub struct Pattern {
    pattern: Vec<PatternPixel>,
}

#[derive(Deserialize, Debug)]
struct PatternPixel {
    x: i32,
    y: i32,
    color: u8,
}

impl PlaceClient {
    pub(crate) fn new() -> Result<Self> {
        let client = Client::new();
        info!("HTTP client initialized successfully");

        Ok(PlaceClient {
            client,
            base_url: "https://ftplace.42lwatch.ch".to_string(),
        })
    }

    pub(crate) async fn get_board(&self) -> Result<(HashMap<u8, Color>, Vec<Vec<u8>>)> {
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

    async fn place_pixel(&self, auth: &mut Auth, x: i32, y: i32, color_id: u8) -> Result<(bool, Option<Duration>)> {
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

    pub(crate) async fn process_pattern(&self,
                                        auth: &mut Auth,
                                        pattern: &Pattern,
                                        start_x: i32,
                                        start_y: i32,
                                        board: &Vec<Vec<u8>>,
                                        max_pixels: usize
    ) -> Result<(usize, Option<Duration>)> {
        let mut pixels_placed = 0;
        let mut wait_duration = None;

        for p in &pattern.pattern {
            if pixels_placed >= max_pixels {
                break;
            }

            let target_x: i32 = start_x + p.x;
            let target_y: i32 = start_y + p.y;
            
            if target_x >= BOARD_SIZE as i32 || target_y >= BOARD_SIZE as i32 {
                error!("Pattern point ({}, {}) out of bounds", target_x, target_y);
                continue;
            }

            if board[target_y as usize][target_x as usize] != p.color {
                let mut retries = 0;
                let max_retries = 3;

                while retries < max_retries {
                    match self.place_pixel(auth, target_x, target_y, p.color).await {
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
