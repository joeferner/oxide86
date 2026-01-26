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

/// Simple DOS memory allocator
pub struct MemoryAllocator {
    /// Allocated memory blocks, keyed by segment address
    blocks: HashMap<u16, MemoryBlock>,
    /// Next available segment to allocate from
    next_segment: u16,
    /// Maximum segment address (end of conventional memory)
    max_segment: u16,
}

impl MemoryAllocator {
    /// Create a new memory allocator
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
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

        // Calculate required segment space
        let required_segments = paragraphs;

        // Check if we have enough space
        let available = self.max_segment.saturating_sub(self.next_segment);
        if required_segments > available {
            return Err((dos_errors::INSUFFICIENT_MEMORY, available));
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
            // Successfully freed
            // If this was the last allocated block (at the end), reclaim the space
            let block_end = segment.saturating_add(block.paragraphs);
            if block_end == self.next_segment {
                // This block was at the end, we can reclaim its space
                self.next_segment = segment;

                // Also check if there are any other blocks that end exactly at the new next_segment
                // This handles cases where we free blocks in reverse order
                loop {
                    let mut found_predecessor = false;
                    for (&seg, blk) in self.blocks.iter() {
                        let blk_end = seg.saturating_add(blk.paragraphs);
                        if blk_end == self.next_segment {
                            // This block ends at our current next_segment, so if we free it
                            // we could move next_segment back further. But we're not freeing it yet.
                            // Just note that there's a block here.
                            found_predecessor = true;
                            break;
                        }
                    }
                    if !found_predecessor {
                        break;
                    }
                }
            }
            Ok(())
        } else {
            Err(dos_errors::INVALID_MEMORY_BLOCK_ADDRESS)
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
            block.paragraphs = new_paragraphs;

            // If this is the last allocated block, reclaim the freed space
            let block_end = segment.saturating_add(old_paragraphs);
            if block_end == self.next_segment {
                self.next_segment = segment.saturating_add(new_paragraphs);
            }

            Ok(())
        } else {
            // Growing - check if we have space
            // For simplicity, only allow growing if this is the last allocated block
            let block_end = segment.saturating_add(old_paragraphs);
            if block_end == self.next_segment {
                // This is the last block, we can grow it
                let additional = new_paragraphs - old_paragraphs;
                let available = self.max_segment.saturating_sub(self.next_segment);

                if additional > available {
                    return Err((dos_errors::INSUFFICIENT_MEMORY, old_paragraphs + available));
                }

                block.paragraphs = new_paragraphs;
                self.next_segment = segment.saturating_add(new_paragraphs);
                Ok(())
            } else {
                // Not the last block - cannot resize in place
                Err((dos_errors::INSUFFICIENT_MEMORY, old_paragraphs))
            }
        }
    }
}
