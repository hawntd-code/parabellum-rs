//! Leer un zip que está en internet sin descargarlo entero. Con peticiones de rango (Range) se baja
//! su índice, el "directorio central" que está al final, y después solo los archivos que hacen falta.
//! Así, actualizar un modpack de Drive baja únicamente lo que cambió. Cada archivo se comprueba con
//! su CRC32, igual que al descomprimir un zip normal.

use std::io::Read;

/// Drive (u otro servidor) no acepta rangos: hay que descargar el zip completo.
pub const NO_RANGES: &str = "El servidor no permite descargar por partes.";
/// Cambió casi todo: es más rápido descargar el zip completo de una vez.
pub const PREFER_FULL: &str = "Conviene descargar el paquete completo.";

#[derive(Debug, Clone)]
pub struct RemoteEntry {
    pub name: String,
    pub crc32: u32,
    pub size: u64,
    pub compressed: u64,
    pub method: u16,
    pub offset: u64,
}

pub struct RemoteZip {
    client: reqwest::blocking::Client,
    url: String,
    size: u64,
}

fn u16_at(data: &[u8], at: usize) -> Result<u16, String> {
    data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(damaged)
}

fn u32_at(data: &[u8], at: usize) -> Result<u32, String> {
    data.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(damaged)
}

fn u64_at(data: &[u8], at: usize) -> Result<u64, String> {
    data.get(at..at + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap())).ok_or_else(damaged)
}

fn damaged() -> String {
    "El zip remoto está dañado o incompleto.".into()
}

impl RemoteZip {
    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn new(url: &str, size: u64) -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent(concat!("Parabellum/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .expect("cliente HTTP");
        Self { client, url: url.to_string(), size }
    }

    /// Bytes [start, end] (ambos incluidos).
    fn range(&self, start: u64, end: u64) -> Result<Vec<u8>, String> {
        let response = self
            .client
            .get(&self.url)
            .header(reqwest::header::RANGE, format!("bytes={start}-{end}"))
            .send()
            .map_err(|e| format!("No se pudo conectar: {e}"))?;
        if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(NO_RANGES.into());
        }
        let bytes = response.bytes().map_err(|e| format!("La descarga se cortó: {e}"))?;
        if bytes.len() as u64 != end - start + 1 {
            return Err(damaged());
        }
        Ok(bytes.to_vec())
    }

    /// Lista de archivos del zip (sin descargarlos).
    pub fn entries(&self) -> Result<Vec<RemoteEntry>, String> {
        // El final del zip: registro de cierre (22 bytes + comentario de hasta 64 KB) y, si es zip64, su localizador.
        let tail_len = self.size.min(66_000 + 20 + 56);
        let tail_start = self.size - tail_len;
        let tail = self.range(tail_start, self.size - 1)?;
        let eocd = tail.windows(4).rposition(|w| w == [0x50, 0x4b, 0x05, 0x06]).ok_or_else(damaged)?;
        let mut count = u16_at(&tail, eocd + 10)? as u64;
        let mut cd_size = u32_at(&tail, eocd + 12)? as u64;
        let mut cd_offset = u32_at(&tail, eocd + 16)? as u64;
        if cd_offset == 0xFFFF_FFFF || cd_size == 0xFFFF_FFFF || count == 0xFFFF {
            // zip64: el localizador va justo antes y apunta al registro de cierre de 64 bits.
            let locator = eocd.checked_sub(20).ok_or_else(damaged)?;
            if u32_at(&tail, locator)? != 0x0706_4b50 {
                return Err(damaged());
            }
            let record_offset = u64_at(&tail, locator + 8)?;
            let record = if record_offset >= tail_start {
                tail[(record_offset - tail_start) as usize..].to_vec()
            } else {
                self.range(record_offset, record_offset + 55)?
            };
            if u32_at(&record, 0)? != 0x0606_4b50 {
                return Err(damaged());
            }
            count = u64_at(&record, 32)?;
            cd_size = u64_at(&record, 40)?;
            cd_offset = u64_at(&record, 48)?;
        }
        if cd_offset + cd_size > self.size {
            return Err(damaged());
        }
        let cd = if cd_offset >= tail_start {
            tail[(cd_offset - tail_start) as usize..(cd_offset - tail_start + cd_size) as usize].to_vec()
        } else {
            self.range(cd_offset, cd_offset + cd_size - 1)?
        };

        let mut entries = Vec::with_capacity(count as usize);
        let mut pos = 0usize;
        while pos + 46 <= cd.len() && u32_at(&cd, pos)? == 0x0201_4b50 {
            let method = u16_at(&cd, pos + 10)?;
            let crc32 = u32_at(&cd, pos + 16)?;
            let mut compressed = u32_at(&cd, pos + 20)? as u64;
            let mut size = u32_at(&cd, pos + 24)? as u64;
            let name_len = u16_at(&cd, pos + 28)? as usize;
            let extra_len = u16_at(&cd, pos + 30)? as usize;
            let comment_len = u16_at(&cd, pos + 32)? as usize;
            let mut offset = u32_at(&cd, pos + 42)? as u64;
            let name = String::from_utf8_lossy(cd.get(pos + 46..pos + 46 + name_len).ok_or_else(damaged)?).replace('\\', "/");
            // Campo extra zip64 con los tamaños y la posición de 64 bits.
            let mut extra = cd.get(pos + 46 + name_len..pos + 46 + name_len + extra_len).ok_or_else(damaged)?;
            while extra.len() >= 4 {
                let id = u16_at(extra, 0)?;
                let len = u16_at(extra, 2)? as usize;
                let data = extra.get(4..4 + len).ok_or_else(damaged)?;
                if id == 1 {
                    let mut at = 0;
                    for value in [&mut size, &mut compressed, &mut offset] {
                        if *value == 0xFFFF_FFFF {
                            *value = u64_at(data, at)?;
                            at += 8;
                        }
                    }
                }
                extra = &extra[4 + len..];
            }
            entries.push(RemoteEntry { name, crc32, size, compressed, method, offset });
            pos += 46 + name_len + extra_len + comment_len;
        }
        Ok(entries)
    }

    /// Lo que ocupa un archivo dentro del zip, más un margen para su encabezado local.
    fn span(entry: &RemoteEntry) -> u64 {
        30 + entry.name.len() as u64 + 512 + entry.compressed
    }

    /// Descarga varios archivos del zip. Los que están seguidos se piden juntos (una petición por
    /// bloque en lugar de una por archivo). `on_file` recibe el índice y el contenido de cada uno.
    pub fn read_many(&self, entries: &[&RemoteEntry], mut on_file: impl FnMut(usize, Vec<u8>) -> Result<(), String>) -> Result<(), String> {
        const GAP: u64 = 512 * 1024;
        const MAX_BLOCK: u64 = 48 * 1024 * 1024;
        let mut order: Vec<usize> = (0..entries.len()).collect();
        order.sort_by_key(|&i| entries[i].offset);
        let mut index = 0;
        while index < order.len() {
            let start = entries[order[index]].offset;
            let mut end = (start + Self::span(entries[order[index]])).min(self.size);
            let mut last = index;
            while last + 1 < order.len() {
                let next = entries[order[last + 1]];
                let next_end = (next.offset + Self::span(next)).min(self.size);
                if next.offset > end + GAP || next_end - start > MAX_BLOCK {
                    break;
                }
                end = end.max(next_end);
                last += 1;
            }
            let block = self.range(start, end - 1)?;
            for &i in &order[index..=last] {
                let entry = entries[i];
                let local = &block[(entry.offset - start) as usize..];
                let bytes = match Self::decode(entry, local)? {
                    Some(bytes) => bytes,
                    // El encabezado era más largo de lo calculado: se pide aparte.
                    None => self.read(entry)?,
                };
                on_file(i, bytes)?;
            }
            index = last + 1;
        }
        Ok(())
    }

    /// Descarga y descomprime un archivo del zip, comprobando su CRC32.
    pub fn read(&self, entry: &RemoteEntry) -> Result<Vec<u8>, String> {
        // Encabezado local (30 bytes + nombre + extra) y los datos, en una sola petición casi siempre.
        let end = (entry.offset + Self::span(entry)).min(self.size) - 1;
        let mut data = self.range(entry.offset, end)?;
        if u32_at(&data, 0)? != 0x0403_4b50 {
            return Err(damaged());
        }
        let start = 30 + u16_at(&data, 26)? as usize + u16_at(&data, 28)? as usize;
        let needed = start + entry.compressed as usize;
        if needed > data.len() {
            let more = self.range(entry.offset + data.len() as u64, entry.offset + needed as u64 - 1)?;
            data.extend(more);
        }
        Self::decode(entry, &data)?.ok_or_else(damaged)
    }

    /// Descomprime un archivo a partir de su encabezado local. None si `local` no lo trae completo.
    fn decode(entry: &RemoteEntry, local: &[u8]) -> Result<Option<Vec<u8>>, String> {
        if u32_at(local, 0)? != 0x0403_4b50 {
            return Err(damaged());
        }
        let start = 30 + u16_at(local, 26)? as usize + u16_at(local, 28)? as usize;
        let needed = start + entry.compressed as usize;
        if needed > local.len() {
            return Ok(None);
        }
        let raw = &local[start..needed];
        let bytes = match entry.method {
            0 => raw.to_vec(),
            8 => {
                let mut out = Vec::with_capacity(entry.size as usize);
                flate2::read::DeflateDecoder::new(raw).read_to_end(&mut out).map_err(|_| damaged())?;
                out
            }
            _ => return Err(format!("{} usa una compresión que Parabellum no conoce.", entry.name)),
        };
        if bytes.len() as u64 != entry.size || crc32fast::hash(&bytes) != entry.crc32 {
            return Err(format!("{} llegó dañado.", entry.name));
        }
        Ok(Some(bytes))
    }
}

/// CRC32 de un archivo local (para saber si ya es igual al del zip).
pub fn file_crc32(path: &std::path::Path) -> Option<u32> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = crc32fast::Hasher::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(hasher.finalize())
}
