//! Shared memory reader for SYSVAD audio bridge
//!
//! Reads interleaved i16 PCM audio samples from a Windows named shared memory section
//! (Global\InfernoAudioShm) written by the SYSVAD virtual audio device driver.

use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::System::Memory::{
    OpenFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::core::PCWSTR;

pub const SHM_NAME: &str = "Global\\InfernoAudioShm";

/// Header at offset 0 of the shared memory (16 bytes total)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ShmHeader {
    pub write_index: u32,
    pub channel_count: u32,
    pub sample_rate: u32,
    pub buffer_frames: u32,
}

pub struct ShmReader {
    ptr: MEMORY_MAPPED_VIEW_ADDRESS,
    size: usize,
    handle: HANDLE,
    last_write_index: u32,
    last_active_time: u64,
}

unsafe impl Send for ShmReader {}
unsafe impl Sync for ShmReader {}

impl ShmReader {
    pub fn open() -> Option<Self> {
        let name: Vec<u16> = SHM_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let handle = unsafe {
            match OpenFileMappingW(FILE_MAP_READ.0, false, PCWSTR(name.as_ptr())) {
                Ok(h) => h,
                Err(_) => {
                    tracing::warn!("Failed to open shared memory '{}': driver not running", SHM_NAME);
                    return None;
                }
            }
        };

        let ptr = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0) };
        if ptr.Value.is_null() {
            tracing::warn!("Failed to map view of shared memory");
            unsafe { CloseHandle(handle).ok(); }
            return None;
        }

        let header = unsafe {
            *(ptr.Value as *const ShmHeader)
        };

        if header.channel_count == 0 || header.channel_count > 64 {
            tracing::warn!("Invalid channel count in shared memory: {}", header.channel_count);
            unsafe {
                UnmapViewOfFile(ptr).ok();
                CloseHandle(handle).ok();
            }
            return None;
        }

        if header.sample_rate == 0 || header.sample_rate > 192000 {
            tracing::warn!("Invalid sample rate in shared memory: {}", header.sample_rate);
            unsafe {
                UnmapViewOfFile(ptr).ok();
                CloseHandle(handle).ok();
            }
            return None;
        }

        if header.buffer_frames == 0 {
            tracing::warn!("Invalid buffer frames in shared memory");
            unsafe {
                UnmapViewOfFile(ptr).ok();
                CloseHandle(handle).ok();
            }
            return None;
        }

        let samples_bytes = header.channel_count as usize * header.buffer_frames as usize * 2;
        let expected_size = 16 + samples_bytes;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Some(Self {
            ptr,
            size: expected_size,
            handle,
            last_write_index: header.write_index,
            last_active_time: now_ms,
        })
    }

    fn header(&self) -> ShmHeader {
        unsafe {
            *(self.ptr.Value as *const ShmHeader)
        }
    }

    pub fn read_samples(&mut self) -> Option<Vec<i16>> {
        let header = self.header();
        let current_write_index = header.write_index;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        if current_write_index == self.last_write_index {
            if now_ms.saturating_sub(self.last_active_time) > 200 {
                tracing::debug!("Driver inactive for >200ms");
                return None;
            }
            return None;
        }

        self.last_active_time = now_ms;

        let buffer_frames = header.buffer_frames as usize;
        let channels = header.channel_count as usize;

        let start_frame = self.last_write_index as usize % buffer_frames;
        let end_frame = current_write_index as usize % buffer_frames;

        let frames_to_read = if end_frame > start_frame {
            end_frame - start_frame
        } else if end_frame < start_frame {
            buffer_frames - start_frame + end_frame
        } else {
            0
        };

        if frames_to_read == 0 {
            return None;
        }

        let samples_per_frame = channels;
        let total_samples_to_read = frames_to_read * samples_per_frame;
        let mut samples = Vec::with_capacity(total_samples_to_read);

        if end_frame > start_frame {
            unsafe {
                let src = (self.ptr.Value as *const i16).add(start_frame * samples_per_frame);
                std::ptr::copy_nonoverlapping(src, samples.as_mut_ptr(), total_samples_to_read);
                samples.set_len(total_samples_to_read);
            }
        } else {
            let first_part_frames = buffer_frames - start_frame;
            let first_part_samples = first_part_frames * samples_per_frame;

            unsafe {
                let src1 = (self.ptr.Value as *const i16).add(start_frame * samples_per_frame);
                std::ptr::copy_nonoverlapping(src1, samples.as_mut_ptr(), first_part_samples);

                let src2 = (self.ptr.Value as *const i16).add(16 / 2);
                std::ptr::copy_nonoverlapping(
                    src2,
                    samples.as_mut_ptr().add(first_part_samples),
                    end_frame * samples_per_frame,
                );

                samples.set_len(total_samples_to_read);
            }
        }

        self.last_write_index = current_write_index;
        Some(samples)
    }

    pub fn channel_count(&self) -> u32 {
        self.header().channel_count
    }

    pub fn sample_rate(&self) -> u32 {
        self.header().sample_rate
    }

    pub fn buffer_frames(&self) -> u32 {
        self.header().buffer_frames
    }

    pub fn is_driver_active(&self) -> bool {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        now_ms.saturating_sub(self.last_active_time) <= 200
    }
}

impl Drop for ShmReader {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.Value.is_null() {
                UnmapViewOfFile(self.ptr).ok();
            }
            if !self.handle.is_invalid() {
                CloseHandle(self.handle).ok();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shm_name_encoding() {
        let _: Vec<u16> = SHM_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
    }
}
