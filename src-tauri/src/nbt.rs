//! NBT (el formato de los datos de Minecraft), lo justo para pasar al servidor el jugador de un
//! mundo individual: en level.dat va en Data › Player y en un servidor va en playerdata/<uuid>.dat.

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use std::io::{Read, Write};

#[derive(Clone, Debug, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    /// Texto tal cual (UTF-8 modificado de Java): se copia sin convertir.
    String(Vec<u8>),
    List(u8, Vec<Tag>),
    Compound(Vec<(Vec<u8>, Tag)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Tag {
    fn id(&self) -> u8 {
        match self {
            Tag::Byte(_) => 1,
            Tag::Short(_) => 2,
            Tag::Int(_) => 3,
            Tag::Long(_) => 4,
            Tag::Float(_) => 5,
            Tag::Double(_) => 6,
            Tag::ByteArray(_) => 7,
            Tag::String(_) => 8,
            Tag::List(..) => 9,
            Tag::Compound(_) => 10,
            Tag::IntArray(_) => 11,
            Tag::LongArray(_) => 12,
        }
    }

    pub fn get(&self, name: &str) -> Option<&Tag> {
        match self {
            Tag::Compound(entries) => entries.iter().find(|(key, _)| key == name.as_bytes()).map(|(_, tag)| tag),
            _ => None,
        }
    }
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, count: usize) -> Option<&[u8]> {
        let slice = self.data.get(self.at..self.at.checked_add(count)?)?;
        self.at += count;
        Some(slice)
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }

    fn length(&mut self) -> Option<usize> {
        let length = i32::from_be_bytes(self.array()?);
        usize::try_from(length).ok()
    }

    fn text(&mut self) -> Option<Vec<u8>> {
        let length = u16::from_be_bytes(self.array()?) as usize;
        Some(self.take(length)?.to_vec())
    }

    fn payload(&mut self, id: u8, depth: u32) -> Option<Tag> {
        if depth > 512 {
            return None;
        }
        Some(match id {
            1 => Tag::Byte(i8::from_be_bytes(self.array()?)),
            2 => Tag::Short(i16::from_be_bytes(self.array()?)),
            3 => Tag::Int(i32::from_be_bytes(self.array()?)),
            4 => Tag::Long(i64::from_be_bytes(self.array()?)),
            5 => Tag::Float(f32::from_be_bytes(self.array()?)),
            6 => Tag::Double(f64::from_be_bytes(self.array()?)),
            7 => {
                let length = self.length()?;
                Tag::ByteArray(self.take(length)?.to_vec())
            }
            8 => Tag::String(self.text()?),
            9 => {
                let kind = self.array::<1>()?[0];
                let length = self.length()?;
                let mut items = Vec::with_capacity(length.min(4096));
                for _ in 0..length {
                    items.push(self.payload(kind, depth + 1)?);
                }
                Tag::List(kind, items)
            }
            10 => {
                let mut entries = Vec::new();
                loop {
                    let kind = self.array::<1>()?[0];
                    if kind == 0 {
                        break;
                    }
                    let name = self.text()?;
                    entries.push((name, self.payload(kind, depth + 1)?));
                }
                Tag::Compound(entries)
            }
            11 => {
                let length = self.length()?;
                (0..length).map(|_| self.array().map(i32::from_be_bytes)).collect::<Option<_>>().map(Tag::IntArray)?
            }
            12 => {
                let length = self.length()?;
                (0..length).map(|_| self.array().map(i64::from_be_bytes)).collect::<Option<_>>().map(Tag::LongArray)?
            }
            _ => return None,
        })
    }
}

fn write_payload(tag: &Tag, out: &mut Vec<u8>) {
    let text = |bytes: &[u8], out: &mut Vec<u8>| {
        out.extend((bytes.len() as u16).to_be_bytes());
        out.extend(bytes);
    };
    match tag {
        Tag::Byte(v) => out.extend(v.to_be_bytes()),
        Tag::Short(v) => out.extend(v.to_be_bytes()),
        Tag::Int(v) => out.extend(v.to_be_bytes()),
        Tag::Long(v) => out.extend(v.to_be_bytes()),
        Tag::Float(v) => out.extend(v.to_be_bytes()),
        Tag::Double(v) => out.extend(v.to_be_bytes()),
        Tag::ByteArray(v) => {
            out.extend((v.len() as i32).to_be_bytes());
            out.extend(v);
        }
        Tag::String(v) => text(v, out),
        Tag::List(kind, items) => {
            out.push(if items.is_empty() { *kind } else { items[0].id() });
            out.extend((items.len() as i32).to_be_bytes());
            for item in items {
                write_payload(item, out);
            }
        }
        Tag::Compound(entries) => {
            for (name, value) in entries {
                out.push(value.id());
                text(name, out);
                write_payload(value, out);
            }
            out.push(0);
        }
        Tag::IntArray(v) => {
            out.extend((v.len() as i32).to_be_bytes());
            for n in v {
                out.extend(n.to_be_bytes());
            }
        }
        Tag::LongArray(v) => {
            out.extend((v.len() as i32).to_be_bytes());
            for n in v {
                out.extend(n.to_be_bytes());
            }
        }
    }
}

/// Lee un archivo NBT comprimido (level.dat, playerdata). Devuelve la etiqueta raíz.
pub fn read_gzip(bytes: &[u8]) -> Option<Tag> {
    let mut data = Vec::new();
    GzDecoder::new(bytes).read_to_end(&mut data).ok()?;
    let mut reader = Reader { data: &data, at: 0 };
    let kind = reader.array::<1>()?[0];
    if kind != 10 {
        return None;
    }
    reader.text()?;
    reader.payload(10, 0)
}

/// Escribe una raíz (compound sin nombre) comprimida, como la guarda Minecraft.
pub fn write_gzip(root: &Tag) -> Vec<u8> {
    let mut data = vec![root.id()];
    data.extend(0u16.to_be_bytes());
    write_payload(root, &mut data);
    let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
    let _ = encoder.write_all(&data);
    encoder.finish().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jugador_de_level_dat() {
        let player = Tag::Compound(vec![
            (b"Health".to_vec(), Tag::Float(17.5)),
            (b"Pos".to_vec(), Tag::List(6, vec![Tag::Double(1.5), Tag::Double(64.0), Tag::Double(-3.25)])),
            (b"Inventory".to_vec(), Tag::List(10, vec![Tag::Compound(vec![(b"id".to_vec(), Tag::String(b"minecraft:diamond".to_vec())), (b"Count".to_vec(), Tag::Byte(3))])])),
            (b"UUID".to_vec(), Tag::IntArray(vec![1, 2, 3, 4])),
            (b"Empty".to_vec(), Tag::List(0, vec![])),
        ]);
        let level = Tag::Compound(vec![(b"Data".to_vec(), Tag::Compound(vec![(b"LevelName".to_vec(), Tag::String("Mundo ñ".as_bytes().to_vec())), (b"Player".to_vec(), player.clone())]))]);
        let bytes = write_gzip(&level);
        let read = read_gzip(&bytes).unwrap();
        assert_eq!(read, level);
        assert_eq!(read.get("Data").and_then(|d| d.get("Player")), Some(&player));
    }
}
