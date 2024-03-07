pub trait BiEndian: Sized {
    type Bytes;

    fn to_bi_endian(self) -> Self::Bytes;
    fn from_bi_endian(bytes: Self::Bytes) -> Option<Self>;
}

impl BiEndian for u16 {
    type Bytes = [u8; 4];

    fn to_bi_endian(self) -> [u8; 4] {
        let mut buffer = [0; 4];
        buffer[..2].copy_from_slice(&self.to_le_bytes());
        buffer[2..].copy_from_slice(&self.to_be_bytes());
        buffer
    }

    fn from_bi_endian(bytes: [u8; 4]) -> Option<Self> {
        let le = Self::from_le_bytes(bytes[..2].try_into().unwrap());
        let be = Self::from_be_bytes(bytes[2..].try_into().unwrap());
        (le == be).then_some(le)
    }
}

impl BiEndian for u32 {
    type Bytes = [u8; 8];

    fn to_bi_endian(self) -> [u8; 8] {
        let mut buffer = [0; 8];
        buffer[..4].copy_from_slice(&self.to_le_bytes());
        buffer[4..].copy_from_slice(&self.to_be_bytes());
        buffer
    }

    fn from_bi_endian(bytes: [u8; 8]) -> Option<Self> {
        let le = Self::from_le_bytes(bytes[..4].try_into().unwrap());
        let be = Self::from_be_bytes(bytes[4..].try_into().unwrap());
        (le == be).then_some(le)
    }
}