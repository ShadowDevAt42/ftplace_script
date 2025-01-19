use clap::Parser;
use serde::{Deserialize, Serialize};

// TODO probably should implement instead of making everything public
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ArgSpecs {
    pub pattern_path: String,
    pub x: i32,
    pub y: i32,
    pub priority: u32, // priority lower = higher
}

impl Ord for ArgSpecs {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}

impl PartialOrd for ArgSpecs {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(long)]
    pub refresh_token: String,
    
    #[arg(long)]
    pub token: String,

    #[arg(long = "pattern")]
    pub patterns: Vec<String>,
}

pub fn parse_patterns(pattern: &str) -> Result<ArgSpecs, String> {
    let parts: Vec<&str> = pattern.split(" ").collect();
    if parts.len() != 4 {
        return Err(format!("Invalid pattern arguments {}", pattern));
    }

    let x = parts[1].parse::<i32>()
        .map_err(|_| format!("Invalid x coordinate: {}", parts[1]))?;
    let y = parts[2].parse::<i32>()
        .map_err(|_| format!("Invalid y coordinate: {}", parts[2]))?;
    let priority = parts[3].parse::<u32>()
        .map_err(|_| format!("Invalid y priority: {}", parts[3]))?;

    Ok(ArgSpecs {
        pattern_path: parts[0].try_into().unwrap(),
        x,
        y,
        priority,
    })
}
