//! The smallest PNG writer `--snap` needs: straight-alpha RGBA in stored (uncompressed)
//! deflate blocks, so snapshots need no dependency and no `golden` feature.

use crate::Image;

/// `image` (premultiplied BGRA) as PNG bytes.
pub(crate) fn encode(image: &Image) -> Vec<u8> {
    let (w, h) = (image.width as usize, image.height as usize);
    let mut raw = Vec::with_capacity((w * 4 + 1) * h);
    for row in image.pixels.chunks_exact(w * 4).take(h) {
        raw.push(0); // no filter
        for &[b, g, r, a] in row.as_chunks::<4>().0 {
            let straight = |c: u8| match a {
                0 => 0,
                a => ((u32::from(c) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8,
            };
            raw.extend_from_slice(&[straight(r), straight(g), straight(b), a]);
        }
    }
    let mut zlib = vec![0x78, 0x01];
    let mut blocks = raw.chunks(0xFFFF).peekable();
    while let Some(block) = blocks.next() {
        let len = block.len() as u16;
        zlib.push(u8::from(blocks.peek().is_none()));
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    if raw.is_empty() {
        zlib.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&(w as u32).to_be_bytes());
    header.extend_from_slice(&(h as u32).to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &zlib);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let at = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[at..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    !bytes.iter().fold(!0u32, |crc, &byte| {
        (0..8).fold(crc ^ u32::from(byte), |c, _| {
            if c & 1 == 1 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 }
        })
    })
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in bytes.chunks(5552) {
        for &byte in chunk {
            a += u32::from(byte);
            b += a;
        }
        (a, b) = (a % 65521, b % 65521);
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_match_the_reference_values() {
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[cfg(feature = "golden")]
    #[test]
    fn a_snapshot_reads_back_as_the_same_pixels() {
        let image = Image {
            width: 2,
            height: 1,
            // Opaque red, half-transparent premultiplied green.
            pixels: vec![0, 0, 255, 255, 0, 64, 0, 128],
        };
        let path = std::env::temp_dir().join("stillkit-el-png-test.png");
        std::fs::write(&path, encode(&image)).unwrap();
        let back = crate::golden::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!((back.width, back.height), (2, 1));
        assert_eq!(back.pixel(0, 0), [0, 0, 255, 255]);
        assert_eq!(back.pixel(1, 0), [0, 64, 0, 128]);
    }
}
