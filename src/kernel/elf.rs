/// AegisOS — Minimal ELF64 Parser
///
/// Parses ELF64 executables from a `&[u8]` byte slice. No heap, no_std.
/// Only supports: ELF64, little-endian, ET_EXEC, EM_AARCH64 (183).
/// Extracts entry point + up to 4 PT_LOAD segments.

// ─── Constants ─────────────────────────────────────────────────────

/// ELF magic bytes: 0x7F 'E' 'L' 'F'
const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

/// ELF64 header size (minimum file size)
const ELF64_HEADER_SIZE: usize = 64;

/// ELF class: 64-bit
const ELFCLASS64: u8 = 2;

/// ELF data encoding: little-endian
const ELFDATA2LSB: u8 = 1;

/// ELF type: executable
const ET_EXEC: u16 = 2;

/// ELF machine: AArch64
const EM_AARCH64: u16 = 183;

/// Program header type: loadable segment
const PT_LOAD: u32 = 1;

/// Maximum PT_LOAD segments we support (static array, no heap)
pub const MAX_SEGMENTS: usize = 4;

/// Program header segment permission flags
pub const PF_X: u32 = 1; // Execute
pub const PF_W: u32 = 2; // Write
pub const PF_R: u32 = 4; // Read

// ─── Data Types ────────────────────────────────────────────────────

/// A single PT_LOAD segment parsed from the ELF program header table.
#[derive(Debug, Clone, Copy)]
pub struct ElfSegment {
    /// Virtual address where this segment should be loaded
    pub vaddr: u64,
    /// Offset of segment data within the ELF file
    pub offset: u64,
    /// Number of bytes to copy from the file (file size)
    pub filesz: u64,
    /// Total memory size (memsz >= filesz; excess is zero-filled BSS)
    pub memsz: u64,
    /// Permission flags: PF_R=4, PF_W=2, PF_X=1
    pub flags: u32,
}

/// Result of parsing an ELF64 file.
#[derive(Debug)]
pub struct ElfInfo {
    /// Entry point virtual address
    pub entry: u64,
    /// PT_LOAD segments (up to MAX_SEGMENTS)
    pub segments: [Option<ElfSegment>; MAX_SEGMENTS],
    /// Number of valid segments in the array
    pub num_segments: usize,
}

/// ELF parse error.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ElfError {
    /// File smaller than ELF64 header (64 bytes)
    TooSmall,
    /// Magic bytes don't match 0x7F 'E' 'L' 'F'
    BadMagic,
    /// Not a 64-bit ELF (Class != 2)
    Not64Bit,
    /// Not little-endian (Data != 1)
    NotLittleEndian,
    /// Not an executable (Type != ET_EXEC)
    NotExecutable,
    /// Wrong architecture (Machine != EM_AARCH64)
    WrongArch,
    /// More than MAX_SEGMENTS PT_LOAD segments
    TooManySegments,
    /// Segment offset+filesz exceeds file bounds
    SegmentOutOfBounds,
    /// Program header table extends past file end
    ProgramHeaderOutOfBounds,
}

// ─── Byte-reading helpers (little-endian, no FP) ───────────────────

/// Read a u16 from `data` at `offset` (little-endian).
#[inline]
fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    (data[offset] as u16) | ((data[offset + 1] as u16) << 8)
}

/// Read a u32 from `data` at `offset` (little-endian).
#[inline]
fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    (data[offset] as u32)
        | ((data[offset + 1] as u32) << 8)
        | ((data[offset + 2] as u32) << 16)
        | ((data[offset + 3] as u32) << 24)
}

/// Read a u64 from `data` at `offset` (little-endian).
#[inline]
fn read_u64_le(data: &[u8], offset: usize) -> u64 {
    (data[offset] as u64)
        | ((data[offset + 1] as u64) << 8)
        | ((data[offset + 2] as u64) << 16)
        | ((data[offset + 3] as u64) << 24)
        | ((data[offset + 4] as u64) << 32)
        | ((data[offset + 5] as u64) << 40)
        | ((data[offset + 6] as u64) << 48)
        | ((data[offset + 7] as u64) << 56)
}

// ─── Main Parser ───────────────────────────────────────────────────

/// Parse an ELF64 executable from a byte slice.
///
/// Returns `ElfInfo` with entry point and up to 4 PT_LOAD segments.
/// No heap allocation — all data returned in stack/static structures.
///
/// # Errors
/// Returns `ElfError` if the file is malformed or unsupported.
pub fn parse_elf64(data: &[u8]) -> Result<ElfInfo, ElfError> {
    // 1. Minimum size check
    if data.len() < ELF64_HEADER_SIZE {
        return Err(ElfError::TooSmall);
    }

    // 2. Magic bytes
    if data[0] != ELF_MAGIC[0]
        || data[1] != ELF_MAGIC[1]
        || data[2] != ELF_MAGIC[2]
        || data[3] != ELF_MAGIC[3]
    {
        return Err(ElfError::BadMagic);
    }

    // 3. Class = 64-bit
    if data[4] != ELFCLASS64 {
        return Err(ElfError::Not64Bit);
    }

    // 4. Data encoding = little-endian
    if data[5] != ELFDATA2LSB {
        return Err(ElfError::NotLittleEndian);
    }

    // 5. Type = ET_EXEC
    let e_type = read_u16_le(data, 16);
    if e_type != ET_EXEC {
        return Err(ElfError::NotExecutable);
    }

    // 6. Machine = EM_AARCH64
    let e_machine = read_u16_le(data, 18);
    if e_machine != EM_AARCH64 {
        return Err(ElfError::WrongArch);
    }

    // 7. Entry point
    let e_entry = read_u64_le(data, 24);

    // 8. Program header table
    let e_phoff = read_u64_le(data, 32) as usize;
    let e_phentsize = read_u16_le(data, 54) as usize;
    let e_phnum = read_u16_le(data, 56) as usize;

    // Validate program header table fits in file
    let ph_table_end = e_phoff.checked_add(e_phnum.checked_mul(e_phentsize)
        .ok_or(ElfError::ProgramHeaderOutOfBounds)?)
        .ok_or(ElfError::ProgramHeaderOutOfBounds)?;
    if ph_table_end > data.len() {
        return Err(ElfError::ProgramHeaderOutOfBounds);
    }

    // 9. Iterate program headers, collect PT_LOAD segments
    let mut segments: [Option<ElfSegment>; MAX_SEGMENTS] = [None; MAX_SEGMENTS];
    let mut num_segments: usize = 0;

    let mut i = 0;
    while i < e_phnum {
        let ph_offset = e_phoff + i * e_phentsize;

        // Each program header entry must be at least 56 bytes (ELF64 Phdr size)
        if ph_offset + 56 > data.len() {
            return Err(ElfError::ProgramHeaderOutOfBounds);
        }

        let p_type = read_u32_le(data, ph_offset);

        if p_type == PT_LOAD {
            if num_segments >= MAX_SEGMENTS {
                return Err(ElfError::TooManySegments);
            }

            let p_flags = read_u32_le(data, ph_offset + 4);
            let p_offset = read_u64_le(data, ph_offset + 8);
            let p_vaddr = read_u64_le(data, ph_offset + 16);
            // p_paddr at ph_offset + 24 (ignored — we use vaddr)
            let p_filesz = read_u64_le(data, ph_offset + 32);
            let p_memsz = read_u64_le(data, ph_offset + 40);

            // Validate segment data fits within file
            let seg_end = (p_offset as usize).checked_add(p_filesz as usize)
                .ok_or(ElfError::SegmentOutOfBounds)?;
            if seg_end > data.len() {
                return Err(ElfError::SegmentOutOfBounds);
            }

            segments[num_segments] = Some(ElfSegment {
                vaddr: p_vaddr,
                offset: p_offset,
                filesz: p_filesz,
                memsz: p_memsz,
                flags: p_flags,
            });
            num_segments += 1;
        }

        i += 1;
    }

    Ok(ElfInfo {
        entry: e_entry,
        segments,
        num_segments,
    })
}

// ─── ELF Loader (Phase L4) ─────────────────────────────────────────

/// ELF load error (returned by loader functions).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ElfLoadError {
    /// Segment vaddr is below the load region or entry outside region
    VaddrOutOfRange,
    /// Segment memsz extends past the end of the load region
    SegmentTooLarge,
    /// Segment has both Write and Execute flags (W^X violation)
    WxViolation,
    /// No loadable segments found in ELF
    NoSegments,
}

/// Validate that an ElfInfo can be loaded into [load_base, load_base + load_size).
///
/// Checks:
///   1. At least one segment present
///   2. No segment has both PF_W and PF_X (W^X enforcement)
///   3. All segment vaddrs within the load region
///   4. Entry point within the load region
pub fn validate_elf_for_load(
    info: &ElfInfo,
    load_base: u64,
    load_size: u64,
) -> Result<(), ElfLoadError> {
    if info.num_segments == 0 {
        return Err(ElfLoadError::NoSegments);
    }

    let load_end = load_base + load_size;

    for i in 0..info.num_segments {
        if let Some(seg) = info.segments[i] {
            // W^X check: never allow both Write and Execute
            if (seg.flags & PF_W != 0) && (seg.flags & PF_X != 0) {
                return Err(ElfLoadError::WxViolation);
            }
            // Vaddr must be within load region
            if seg.vaddr < load_base {
                return Err(ElfLoadError::VaddrOutOfRange);
            }
            // Segment end must not exceed load region
            let seg_end = seg.vaddr.checked_add(seg.memsz)
                .ok_or(ElfLoadError::SegmentTooLarge)?;
            if seg_end > load_end {
                return Err(ElfLoadError::SegmentTooLarge);
            }
        }
    }

    // Entry point must be within the load region
    if info.entry < load_base || info.entry >= load_end {
        return Err(ElfLoadError::VaddrOutOfRange);
    }

    Ok(())
}

/// Load ELF segments into memory.
///
/// Copies each segment's `filesz` bytes from ELF data to the destination
/// buffer, then zeros the BSS portion (`memsz - filesz`). Validates the
/// load configuration before copying.
///
/// # Parameters
/// - `elf_data`: raw ELF binary (source for segment data)
/// - `info`: parsed ELF info from `parse_elf64`
/// - `dest`: pointer to writable memory at `dest_vaddr`
/// - `dest_vaddr`: virtual address corresponding to `dest` (for offset calculation)
/// - `dest_size`: available bytes at `dest`
///
/// # Returns
/// Entry point address on success.
///
/// # Safety
/// `dest` must point to at least `dest_size` bytes of writable memory.
pub unsafe fn load_elf_segments(
    elf_data: &[u8],
    info: &ElfInfo,
    dest: *mut u8,
    dest_vaddr: u64,
    dest_size: usize,
) -> Result<u64, ElfLoadError> {
    // Validate before touching memory
    validate_elf_for_load(info, dest_vaddr, dest_size as u64)?;

    // SAFETY: caller guarantees dest points to dest_size bytes of writable memory;
    // validate_elf_for_load ensures all segments fit within that region
    unsafe {
        for i in 0..info.num_segments {
            if let Some(seg) = info.segments[i] {
                let offset_in_dest = (seg.vaddr - dest_vaddr) as usize;

                // Copy filesz bytes from ELF data → destination
                if seg.filesz > 0 {
                    let src = elf_data.as_ptr().add(seg.offset as usize);
                    let dst = dest.add(offset_in_dest);
                    core::ptr::copy_nonoverlapping(src, dst, seg.filesz as usize);
                }

                // Zero BSS (memsz - filesz)
                if seg.memsz > seg.filesz {
                    let bss_start = dest.add(offset_in_dest + seg.filesz as usize);
                    core::ptr::write_bytes(bss_start, 0, (seg.memsz - seg.filesz) as usize);
                }
            }
        }
    }

    Ok(info.entry)
}
