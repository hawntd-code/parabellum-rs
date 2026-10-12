//! Abrir cualquier imagen que Windows sepa abrir (fotos HEIC del celular, TIFF, JPEG XR, ICO…): la
//! ventana solo entiende algunos formatos y con los demás decía «The source image cannot be decoded».
//! Windows (WIC) la decodifica, se reduce si es muy grande y vuelve como PNG (que la ventana sí abre).

use windows::core::GUID;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA, IWICImagingFactory, WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom,
    WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};

/// Más grande no hace falta: el chat la reduce a 1280 px y el icono a 64.
const MAX_SIDE: u32 = 2048;
pub const MAX_BYTES: usize = 25 * 1024 * 1024;

/// La imagen como PNG (RGBA), reducida a MAX_SIDE de lado como mucho.
pub fn to_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let failed = || t!("Windows tampoco pudo abrir esa imagen. Prueba con una captura o un PNG/JPG.", "Windows could not open that image either. Try a screenshot or a PNG/JPG.");
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(t!("La imagen pesa más de 25 MB.", "The image is larger than 25 MB."));
    }
    let (width, height, pixels) = decode(bytes).map_err(|_| failed())?;
    Ok(encode_png(width, height, &pixels))
}

/// Decodifica con WIC: (ancho, alto, píxeles RGBA).
fn decode(bytes: &[u8]) -> windows::core::Result<(u32, u32, Vec<u8>)> {
    // SAFETY: COM se inicia en este hilo (si ya estaba iniciado no pasa nada); todos los objetos son
    // de WIC y viven dentro de esta función; el búfer de píxeles tiene el tamaño exacto pedido.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream = factory.CreateStream()?;
        stream.InitializeFromMemory(bytes)?;
        let decoder = factory.CreateDecoderFromStream(&stream, std::ptr::null::<GUID>(), WICDecodeMetadataCacheOnDemand)?;
        let frame = decoder.GetFrame(0)?;
        let (mut width, mut height) = (0u32, 0u32);
        frame.GetSize(&mut width, &mut height)?;
        if width == 0 || height == 0 {
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(-1)));
        }
        let scale = (MAX_SIDE as f64 / width.max(height) as f64).min(1.0);
        let (out_w, out_h) = (((width as f64 * scale).round() as u32).max(1), ((height as f64 * scale).round() as u32).max(1));
        let scaler = factory.CreateBitmapScaler()?;
        scaler.Initialize(&frame, out_w, out_h, WICBitmapInterpolationModeFant)?;
        let converter = factory.CreateFormatConverter()?;
        converter.Initialize(&scaler, &GUID_WICPixelFormat32bppBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)?;
        let stride = out_w * 4;
        let mut pixels = vec![0u8; (stride * out_h) as usize];
        converter.CopyPixels(std::ptr::null(), stride, &mut pixels)?;
        // BGRA → RGBA
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        Ok((out_w, out_h, pixels))
    }
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
