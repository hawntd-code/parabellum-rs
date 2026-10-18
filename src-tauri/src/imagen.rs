//! Abrir cualquier imagen que el sistema sepa abrir (fotos HEIC del celular, TIFF, JPEG XR, ICO…): la
//! ventana solo entiende algunos formatos y con los demás decía «The source image cannot be decoded».
//! El sistema la decodifica (plataforma::decode_image), se reduce si es muy grande y vuelve como PNG
//! (que la ventana sí abre).

/// Más grande no hace falta: el chat la reduce a 1280 px y el icono a 64.
const MAX_SIDE: u32 = 2048;
pub const MAX_BYTES: usize = 25 * 1024 * 1024;

/// La imagen como PNG (RGBA), reducida a MAX_SIDE de lado como mucho.
/// Si los primeros bytes son de una imagen (PNG, JPEG, GIF, WebP, BMP, TIFF, ICO, HEIC/AVIF, JPEG XR o
/// DDS) y no solo el nombre. Lo mismo que hace Mac al pegar (PR #15, is_image_file).
pub fn is_image(bytes: &[u8]) -> bool {
    let starts = |magic: &[u8]| bytes.starts_with(magic);
    starts(b"\x89PNG\r\n\x1a\n")
        || starts(&[0xFF, 0xD8, 0xFF])
        || starts(b"GIF87a")
        || starts(b"GIF89a")
        || (starts(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"))
        || starts(b"BM")
        || starts(b"II*\0")
        || starts(b"MM\0*")
        || starts(&[0x49, 0x49, 0xBC])
        || starts(&[0, 0, 1, 0])
        || starts(b"DDS ")
        || bytes.get(4..8) == Some(b"ftyp")
}

pub fn to_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let failed = || t!("{} tampoco pudo abrir esa imagen. Prueba con una captura o un PNG/JPG.", "{} could not open that image either. Try a screenshot or a PNG/JPG.", crate::plataforma::NAME);
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(t!("La imagen pesa más de 25 MB.", "The image is larger than 25 MB."));
    }
    let (width, height, pixels) = decode(bytes).map_err(|_| failed())?;
    Ok(encode_png(width, height, &pixels))
}

/// Decodifica con el sistema (WIC en Windows, sips en Mac): (ancho, alto, píxeles RGBA).
fn decode(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    crate::plataforma::decode_image(bytes, MAX_SIDE)
}

/// PNG sin pérdida (RGBA de 8 bits), con zlib de flate2.
fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    use std::io::Write;
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend(kind);
        out.extend(data);
        let crc = crc32fast::hash(&out[start..]);
        out.extend(crc.to_be_bytes());
    }
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::with_capacity(13);
    header.extend(width.to_be_bytes());
    header.extend(height.to_be_bytes());
    header.extend([8, 6, 0, 0, 0]); // 8 bits, RGBA, deflate, sin filtro adaptativo, sin entrelazado
    chunk(&mut png, b"IHDR", &header);
    let row = (width * 4) as usize;
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    for line in rgba.chunks_exact(row) {
        let _ = encoder.write_all(&[0]);
        let _ = encoder.write_all(line);
    }
    chunk(&mut png, b"IDAT", &encoder.finish().unwrap_or_default());
    chunk(&mut png, b"IEND", &[]);
    png
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imagen_por_dentro() {
        assert!(is_image(b"\x89PNG\r\n\x1a\n...."));
        assert!(is_image(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(is_image(b"RIFF\0\0\0\0WEBPVP8 "));
        assert!(is_image(b"\0\0\0\x18ftypheic"));
        assert!(!is_image(b"MZ\x90\0 un programa"));
        assert!(!is_image(b"hola, soy texto"));
    }

    #[test]
    fn png_valido_y_windows_lo_abre() {
        // Un PNG hecho aquí, abierto de vuelta por Windows (como si fuera un formato raro).
        let pixels: Vec<u8> = (0..3 * 2).flat_map(|i| [i as u8 * 40, 200, 30, 255]).collect();
        let png = encode_png(3, 2, &pixels);
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        let (w, h, back) = decode(&png).unwrap();
        assert_eq!((w, h), (3, 2));
        assert_eq!(back, pixels);
        assert!(to_png(b"esto no es una imagen").is_err());
    }
}
