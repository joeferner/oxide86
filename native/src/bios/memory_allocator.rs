use emu86_core::cpu::bios::dos_errors;
use std::collections::HashMap;

/// Memory allocation block
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MemoryBlock {
    /// Segment address where block starts
    pub segment: u16,
    /// Size of block in paragraphs (16-byte units)
    pub paragraphs: u16,
}

/// Free memory block
#[derive(Debug, Clone)]
struct FreeBlock {
    /// Segment address where block starts
    segment: u16,
    /// Size of block in paragraphs (16-byte units)
    paragraphs: u16,
}

/// DOS memory allocator with free list
pub struct MemoryAllocator {
    /// Allocated memory blocks, keyed by segment address
    blocks: HashMap<u16, MemoryBlock>,
    /// Free memory blocks in the middle of allocated space
    free_blocks: Vec<FreeBlock>,
    /// Next available segment to allocate from (beyond all allocated blocks)
    next_segment: u16,
    /// Maximum segment address (end of conventional memory)
    max_segment: u16,
}

impl MemoryAllocator {
    /// Create a new memory allocator
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            free_blocks: Vec::new(),
            // Start allocating from segment 0x2000 to avoid:
            // - IVT (0x0000-0x03FF)
            // - BDA (0x0400-0x04FF)
            // - DOS kernel area and typical program load area (0x0500-0x1FFF)
            next_segment: 0x2000,
            // End of conventional memory (640KB = 0xA0000 bytes = segment 0xA000)
            max_segment: 0xA000,
        }
    }

    /// Allocate memory
    /// Returns segment address on success, or (error_code, max_available) on failure
    pub fn allocate(&mut self, paragraphs: u16) -> Result<u16, (u8, u16)> {
        if paragraphs == 0 {
            return Err((dos_errors::INVALID_MEMORY_BLOCK_ADDRESS, 0));
        }

        // First, try to find a free block that fits (first-fit strategy)
        for i in 0..self.free_blocks.len() {
            if self.free_blocks[i].paragraphs >= paragraphs {
                let free_block = self.free_blocks.remove(i);
                let segment = free_block.segment;

                // If the free block is larger than needed, split it
                if free_block.paragraphs > paragraphs {
                    let remaining = FreeBlock {
                        segment: segment.saturating_add(paragraphs),
                        paragraphs: free_block.paragraphs - paragraphs,
                    };
                    self.free_blocks.push(remaining);
                }

                // Allocate the block
                let block = MemoryBlock {
                    segment,
                    paragraphs,
                };
                self.blocks.insert(segment, block);

                return Ok(segment);
            }
        }

        // No suitable free block found, allocate from next_segment
        let available = self.max_segment.saturating_sub(self.next_segment);
        if paragraphs > available {
            // Return the size of the largest available block
            let max_block = self
                .free_blocks
                .iter()
                .map(|b| b.paragraphs)
                .max()
                .unwrap_or(0)
                .max(available);
            return Err((dos_errors::INSUFFICIENT_MEMORY, max_block));
        }

        // Allocate block at next_segment
        let segment = self.next_segment;
        let block = MemoryBlock {
            segment,
            paragraphs,
        };

        self.blocks.insert(segment, block);
        self.next_segment = self.next_segment.saturating_add(paragraphs);

        Ok(segment)
    }

    /// Free memory
    pub fn free(&mut self, segment: u16) -> Result<(), u8> {
        if let Some(block) = self.blocks.remove(&segment) {
            let block_end = segment.saturating_add(block.paragraphs);

            // If this was the last allocated block (at the end), reclaim the space
            if block_end == self.next_segment {
                self.next_segment = segment;

                // Check if we can merge with any free blocks that now touch next_segment
                self.merge_free_blocks_at_end();
            } else {
                // Block is in the middle, add it to free list
                let free_block = FreeBlock {
                    segment,
                    paragraphs: block.paragraphs,
                };
                self.free_blocks.push(free_block);

                // Try to merge adjacent free blocks
                self.merge_adjacent_free_blocks();
            }

            Ok(())
        } else {
            Err(dos_errors::INVALID_MEMORY_BLOCK_ADDRESS)
        }
    }

    /// Merge free blocks that are adjacent to next_segment back into the free space
    fn merge_free_blocks_at_end(&mut self) {
        loop {
            let mut merged = false;
            for i in (0..self.free_blocks.len()).rev() {
                let free_end = self.free_blocks[i]
                    .segment
                    .saturating_add(self.free_blocks[i].paragraphs);
                if free_end == self.next_segment {
                    // This free block touches next_segment, merge it
                    self.next_segment = self.free_blocks[i].segment;
                    self.free_blocks.remove(i);
                    merged = true;
                    break;
                }
            }
            if !merged {
                break;
            }
        }
    }

    /// Merge adjacent free blocks to reduce fragmentation
    fn merge_adjacent_free_blocks(&mut self) {
        if self.free_blocks.len() < 2 {
            return;
        }

        // Sort free blocks by segment address
        self.free_blocks.sort_by_key(|b| b.segment);

        // Merge adjacent blocks
        let mut i = 0;
        while i < self.free_blocks.len() - 1 {
            let current_end = self.free_blocks[i]
                .segment
                .saturating_add(self.free_blocks[i].paragraphs);
            if current_end == self.free_blocks[i + 1].segment {
                // Merge blocks
                self.free_blocks[i].paragraphs = self.free_blocks[i]
                    .paragraphs
                    .saturating_add(self.free_blocks[i + 1].paragraphs);
                self.free_blocks.remove(i + 1);
                // Don't increment i, check if we can merge with the next block too
            } else {
                i += 1;
            }
        }
    }

    /// Resize memory block
    pub fn resize(&mut self, segment: u16, new_paragraphs: u16) -> Result<(), (u8, u16)> {
        // Get the existing block
        let block = self
            .blocks
            .get_mut(&segment)
            .ok_or((dos_errors::INVALID_MEMORY_BLOCK_ADDRESS, 0))?;

        let old_paragraphs = block.paragraphs;

        if new_paragraphs == old_paragraphs {
            // No change needed
            return Ok(());
        }

        if new_paragraphs < old_paragraphs {
            // Shrinking - always succeeds
            let freed_paragraphs = old_paragraphs - new_paragraphs;
            block.paragraphs = new_paragraphs;

            // Add the freed space as a free block or merge with next_segment
            let freed_segment = segment.saturating_add(new_paragraphs);
            let block_end = segment.saturating_add(old_paragraphs);

            if block_end == self.next_segment {
                // This is the last allocated block, reclaim the freed space
                self.next_segment = freed_segment;
            } else {
                // Add freed space to free list
                let free_block = FreeBlock {
                    segment: freed_segment,
                    paragraphs: freed_paragraphs,
                };
                self.free_blocks.push(free_block);
                self.merge_adjacent_free_blocks();
            }

            Ok(())
        } else {
            // Growing - check if we have space
            let additional = new_paragraphs - old_paragraphs;
            let block_end = segment.saturating_add(old_paragraphs);

            if block_end == self.next_segment {
                // This is the last block, we can grow it from next_segment
                let available = self.max_segment.saturating_sub(self.next_segment);

                if additional > available {
                    return Err((dos_errors::INSUFFICIENT_MEMORY, old_paragraphs + available));
                }

                block.paragraphs = new_paragraphs;
                self.next_segment = segment.saturating_add(new_paragraphs);
                Ok(())
            } else {
                // Check if there's a free block immediately after this block
                let mut found_free_index = None;
                for (i, free_block) in self.free_blocks.iter().enumerate() {
                    if free_block.segment == block_end {
                        found_free_index = Some(i);
                        break;
                    }
                }

                if let Some(free_index) = found_free_index {
                    let free_block = &self.free_blocks[free_index];
                    if free_block.paragraphs >= additional {
                        // We can grow into the free block
                        let remaining = free_block.paragraphs - additional;

                        if remaining > 0 {
                            // Update the free block to be smaller
                            self.free_blocks[free_index].segment =
                                block_end.saturating_add(additional);
                            self.free_blocks[free_index].paragraphs = remaining;
                        } else {
                            // Use the entire free block
                            self.free_blocks.remove(free_index);
                        }

                        block.paragraphs = new_paragraphs;
                        return Ok(());
                    }
                }

                // Cannot resize in place - not enough space
                Err((dos_errors::INSUFFICIENT_MEMORY, old_paragraphs))
            }
        }
    }
}
