use std::collections::{BTreeMap, VecDeque};

use crate::devices::uart::ComPortDevice;

pub struct MockComDevice {
    /// Simulates hardware delay/asynchronicity
    async_count: u32,
    async_threshold: u32,
    /// Buffer for characters coming FROM the computer
    input_buffer: String,
    /// Queue of characters waiting to go TO the computer
    output_queue: VecDeque<u8>,
    /// Map of "Expected Input" -> "Response to Queue"
    responses: BTreeMap<String, String>,
    /// Tracks which triggers were matched in order
    pub matched_history: Vec<String>,
}

impl MockComDevice {
    pub fn new(threshold: u32) -> Self {
        Self {
            async_count: 0,
            async_threshold: threshold,
            input_buffer: String::new(),
            output_queue: VecDeque::new(),
            responses: BTreeMap::new(),
            matched_history: Vec::new(),
        }
    }

    /// Register a conversation pair
    pub fn add_response(&mut self, trigger: &str, response: &str) {
        self.responses
            .insert(trigger.to_string(), response.to_string());
    }

    /// Helper to check if a specific command was received
    pub fn was_received(&self, trigger: &str) -> bool {
        self.matched_history.iter().any(|h| h == trigger)
    }
}

impl ComPortDevice for MockComDevice {
    fn read(&mut self) -> Option<u8> {
        if self.async_count >= self.async_threshold {
            if let Some(out) = self.output_queue.pop_front() {
                self.async_count = 0;
                return Some(out);
            }
        }
        self.async_count += 1;
        None
    }

    fn write(&mut self, value: u8) -> bool {
        if self.async_count >= self.async_threshold {
            self.async_count = 0;
            self.input_buffer.push(value as char);

            let mut matched_key = None;
            for trigger in self.responses.keys() {
                if self.input_buffer.ends_with(trigger) {
                    matched_key = Some(trigger.clone());
                    break;
                }
            }

            if let Some(trigger) = matched_key {
                let resp = self.responses.get(&trigger).unwrap().clone();
                self.output_queue.extend(resp.as_bytes());

                // Record the "hit" for verification later
                self.matched_history.push(trigger);
                self.input_buffer.clear();
            }
            true
        } else {
            self.async_count += 1;
            false
        }
    }
}
