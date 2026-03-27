use serde::{Deserialize, Serialize};

/// Project data stored in state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub goal_amount: u64,
    pub deadline_timestamp: u64,
    pub creator_wallet: String,
    pub raised_amount: u64,
    pub backers: Vec<String>, // List of backer addresses
}

impl Project {
    pub fn new(
        project_id: String,
        name: String,
        description: String,
        goal_amount: u64,
        deadline_timestamp: u64,
        creator_wallet: String,
    ) -> Self {
        Project {
            project_id,
            name,
            description,
            goal_amount,
            deadline_timestamp,
            creator_wallet,
            raised_amount: 0,
            backers: Vec::new(),
        }
    }

    /// Check if project can accept donations at given time
    pub fn can_accept_donations(&self, current_time: u64) -> bool {
        current_time < self.deadline_timestamp
    }
}
