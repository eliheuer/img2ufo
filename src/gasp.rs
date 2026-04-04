/// Insert a `gasp` table into a compiled TrueType font file.
///
/// For unhinted fonts, Google Fonts requires a gasp table with a single range:
///   rangeMaxPPEM = 0xFFFF, flags = SYMMETRIC_SMOOTHING | SYMMETRIC_GRIDFIT (0x000A)
///
/// This avoids requiring Python/gftools as a post-build dependency.

use anyhow::{bail, Context, Result};
use std::path::Path;

/// Add the standard unhinted gasp table to a TTF file in-place.
pub fn fix_gasp(ttf_path: &Path) -> Result<()> {
    let data = std::fs::read(ttf_path)
        .with_context(|| format!("Cannot read {:?}", ttf_path))?;

    if data.len() < 12 {
        bail!("File too small to be a valid font");
    }

    // Check if gasp already exists.
    let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;
    for i in 0..num_tables {
        let offset = 12 + i * 16;
        if offset + 16 > data.len() {
            bail!("Truncated table directory");
        }
        if &data[offset..offset + 4] == b"gasp" {
            return Ok(()); // Already has gasp, nothing to do.
        }
    }

    // Build the gasp table: version=0, 1 range, maxPPEM=0xFFFF, flags=0x000A
    let gasp_data: [u8; 8] = [0x00, 0x00, 0x00, 0x01, 0xFF, 0xFF, 0x00, 0x0A];
    let gasp_checksum = table_checksum(&gasp_data);

    // New number of tables.
    let new_num = (num_tables + 1) as u16;
    let search_range = (1u16 << (15 - new_num.leading_zeros())).wrapping_mul(16);
    let entry_selector = 15u16.saturating_sub(new_num.leading_zeros() as u16);
    let range_shift = new_num * 16 - search_range;

    // Build new file:
    // 1. Updated offset table (12 bytes)
    // 2. Old table records + new gasp record (sorted by tag)
    // 3. All original table data
    // 4. gasp data (appended)

    let old_header_end = 12 + num_tables * 16;
    let new_header_end = 12 + (num_tables + 1) * 16;
    // Everything shifts by 16 bytes (one new table record).
    let shift = 16u32;

    let mut out = Vec::with_capacity(data.len() + 16 + 8);

    // Updated offset table.
    out.extend_from_slice(&data[0..4]); // sfVersion
    out.extend_from_slice(&new_num.to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&range_shift.to_be_bytes());

    // Collect old records, shift their offsets, insert gasp in sorted position.
    let mut records: Vec<[u8; 16]> = Vec::with_capacity(num_tables + 1);
    for i in 0..num_tables {
        let off = 12 + i * 16;
        let mut rec = [0u8; 16];
        rec.copy_from_slice(&data[off..off + 16]);
        // Shift the offset field (bytes 8..12) by `shift`.
        let old_off = u32::from_be_bytes([rec[8], rec[9], rec[10], rec[11]]);
        let new_off = old_off + shift;
        rec[8..12].copy_from_slice(&new_off.to_be_bytes());
        records.push(rec);
    }

    // gasp record: tag + checksum + offset (end of file) + length.
    let gasp_offset = (data.len() as u32 - old_header_end as u32) + new_header_end as u32;
    let mut gasp_rec = [0u8; 16];
    gasp_rec[0..4].copy_from_slice(b"gasp");
    gasp_rec[4..8].copy_from_slice(&gasp_checksum.to_be_bytes());
    gasp_rec[8..12].copy_from_slice(&gasp_offset.to_be_bytes());
    gasp_rec[12..16].copy_from_slice(&(gasp_data.len() as u32).to_be_bytes());
    records.push(gasp_rec);

    // Sort records by tag (OpenType requires sorted table directory).
    records.sort_by(|a, b| a[0..4].cmp(&b[0..4]));

    for rec in &records {
        out.extend_from_slice(rec);
    }

    // Original table data (everything after the old table directory).
    out.extend_from_slice(&data[old_header_end..]);

    // Append gasp data.
    out.extend_from_slice(&gasp_data);

    // Recalculate head.checksumAdjustment.
    // Find the head table record to get its offset.
    for rec in &records {
        if &rec[0..4] == b"head" {
            let head_off = u32::from_be_bytes([rec[8], rec[9], rec[10], rec[11]]) as usize;
            if head_off + 12 <= out.len() {
                // Zero out checksumAdjustment (bytes 8..12 in head table) before computing.
                out[head_off + 8] = 0;
                out[head_off + 9] = 0;
                out[head_off + 10] = 0;
                out[head_off + 11] = 0;
                let whole = whole_file_checksum(&out);
                let adj = 0xB1B0AFBAu32.wrapping_sub(whole);
                out[head_off + 8..head_off + 12].copy_from_slice(&adj.to_be_bytes());
            }
            break;
        }
    }

    std::fs::write(ttf_path, &out)
        .with_context(|| format!("Cannot write {:?}", ttf_path))?;

    Ok(())
}

fn table_checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    let mut i = 0;
    while i + 4 <= data.len() {
        sum = sum.wrapping_add(u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]));
        i += 4;
    }
    // Pad final partial word with zeros.
    if i < data.len() {
        let mut last = [0u8; 4];
        for (j, &b) in data[i..].iter().enumerate() {
            last[j] = b;
        }
        sum = sum.wrapping_add(u32::from_be_bytes(last));
    }
    sum
}

fn whole_file_checksum(data: &[u8]) -> u32 {
    table_checksum(data)
}
