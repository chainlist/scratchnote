//! `vectors.bin`, the file the vectors were saved in before `space.db`, read
//! once when `space.db` is made. Little endian throughout. The header is
//! `SNVB`, a u32 version, the model id, the dims as u32 and the count as
//! u32. Each note follows, in id order, as its id, its body hash and `dims`
//! f32 values. A string is a u32 byte length then UTF-8.

use std::collections::HashMap;
use std::path::Path;

use super::{Unsaved, Vectors};
#[cfg(test)]
use crate::embed::math::to_le_bytes;
use crate::embed::math::{add, from_le_bytes};

pub(super) const MAGIC: &[u8; 4] = b"SNVB";
pub(super) const VERSION: u32 = 1;

impl Vectors {
    /// The vectors `vectors.bin` holds, `None` for anything but a whole,
    /// well-formed file.
    pub fn read_bin(path: &Path) -> Option<Self> {
        Self::decode(&std::fs::read(path).ok()?)
    }

    /// The file `read_bin` reads, as the app saved it before `space.db`.
    #[cfg(test)]
    pub fn write_bin(&self, path: &Path) {
        std::fs::write(path, self.encode()).unwrap();
    }

    #[cfg(test)]
    pub(super) fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.notes.len() * (self.dims * 4 + 48));
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        put_str(&mut out, &self.model_id);
        out.extend_from_slice(&(self.dims as u32).to_le_bytes());
        out.extend_from_slice(&(self.notes.len() as u32).to_le_bytes());
        let mut notes: Vec<_> = self.notes.iter().collect();
        notes.sort_by(|a, b| a.0.cmp(b.0));
        for (id, (hash, vector)) in notes {
            put_str(&mut out, id);
            put_str(&mut out, hash);
            out.extend(to_le_bytes(vector));
        }
        out
    }

    /// `None` for anything but a whole, well-formed file: every length is
    /// checked against what is left, and nothing may trail the last note.
    pub(super) fn decode(bytes: &[u8]) -> Option<Self> {
        let mut reader = Reader(bytes);
        if reader.take(4)? != MAGIC || reader.u32()? != VERSION {
            return None;
        }
        let model_id = reader.string()?;
        let dims = reader.u32()? as usize;
        let count = reader.u32()?;

        // Not sized from `count`: a damaged count must not allocate.
        let mut notes = HashMap::new();
        let mut sum = vec![0.0; dims];
        for _ in 0..count {
            let id = reader.string()?;
            let hash = reader.string()?;
            let vector = from_le_bytes(reader.take(dims.checked_mul(4)?)?);
            add(&mut sum, &vector, 1.0);
            if let Some((_, old)) = notes.insert(id, (hash, vector)) {
                add(&mut sum, &old, -1.0);
            }
        }

        reader.0.is_empty().then_some(Self {
            model_id,
            dims,
            notes,
            sum,
            unsaved: Unsaved::All,
        })
    }
}

#[cfg(test)]
pub(super) fn put_str(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&(text.len() as u32).to_le_bytes());
    out.extend_from_slice(text.as_bytes());
}

/// What is left of the file to decode.
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if n > self.0.len() {
            return None;
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Some(head)
    }

    fn u32(&mut self) -> Option<u32> {
        self.take(4)?.try_into().ok().map(u32::from_le_bytes)
    }

    fn string(&mut self) -> Option<String> {
        let len = self.u32()? as usize;
        String::from_utf8(self.take(len)?.to_vec()).ok()
    }
}
