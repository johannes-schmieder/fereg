#![allow(dead_code)]

pub fn strings(data: &[u8], max_items: usize, max_bytes: usize) -> Vec<String> {
    data.split(|byte| *byte == 0)
        .take(max_items)
        .map(|part| String::from_utf8_lossy(&part[..part.len().min(max_bytes)]).into_owned())
        .collect()
}

pub struct Cursor<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    pub fn byte(&mut self) -> u8 {
        let value = self.data.get(self.position).copied().unwrap_or(0);
        self.position = self.position.saturating_add(1);
        value
    }

    pub fn u32(&mut self) -> u32 {
        let bytes = [self.byte(), self.byte(), self.byte(), self.byte()];
        u32::from_le_bytes(bytes)
    }

    pub fn i64(&mut self) -> i64 {
        let bytes = [
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
        ];
        i64::from_le_bytes(bytes)
    }

    pub fn f64(&mut self) -> f64 {
        f64::from_bits(self.i64() as u64)
    }

    pub fn string(&mut self, max_bytes: usize) -> String {
        let length = usize::from(self.byte()).min(max_bytes);
        let mut bytes = Vec::with_capacity(length);
        for _ in 0..length {
            bytes.push(self.byte());
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }
}
