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
