use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct LeaderboardPayload {
    pub tokens: u32,
    pub user: String,
}
