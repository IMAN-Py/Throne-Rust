//! Protobuf and TLV wire framing codec for Throne Named Pipe IPC.
//! Encodes and decodes Varint, fixed-size integers, and length-delimited payloads.

pub const WIRE_TYPE_VARINT: u8 = 0;
pub const WIRE_TYPE_FIXED64: u8 = 1;
pub const WIRE_TYPE_LENGTH_DELIMITED: u8 = 2;
pub const WIRE_TYPE_FIXED32: u8 = 5;

pub struct VarintCodec;

impl VarintCodec {
    #[inline]
    pub fn encode_u64(mut val: u64, buf: &mut Vec<u8>) {
        while val >= 0x80 {
            buf.push(((val & 0x7F) as u8) | 0x80);
            val >>= 7;
        }
        buf.push(val as u8);
    }

    #[inline]
    pub fn decode_u64(data: &[u8]) -> Option<(u64, usize)> {
        let mut result = 0u64;
        let mut shift = 0;

        for (i, &byte) in data.iter().enumerate() {
            if shift >= 64 {
                return None; // Overflow
            }
            result |= ((byte & 0x7F) as u64) << shift;
            shift += 7;
            if (byte & 0x80) == 0 {
                return Some((result, i + 1));
            }
        }
        None
    }
}

pub struct SimpleProtobufWriter<'a> {
    buffer: &'a mut Vec<u8>,
}

impl<'a> SimpleProtobufWriter<'a> {
    pub fn new(buffer: &'a mut Vec<u8>) -> Self {
        Self { buffer }
    }

    pub fn write_tag(&mut self, field_number: u32, wire_type: u8) {
        let tag = ((field_number as u64) << 3) | ((wire_type as u64) & 0x07);
        VarintCodec::encode_u64(tag, self.buffer);
    }

    pub fn write_uint64(&mut self, field_number: u32, val: u64) {
        self.write_tag(field_number, WIRE_TYPE_VARINT);
        VarintCodec::encode_u64(val, self.buffer);
    }

    pub fn write_bytes(&mut self, field_number: u32, bytes: &[u8]) {
        self.write_tag(field_number, WIRE_TYPE_LENGTH_DELIMITED);
        VarintCodec::encode_u64(bytes.len() as u64, self.buffer);
        self.buffer.extend_from_slice(bytes);
    }

    pub fn write_string(&mut self, field_number: u32, s: &str) {
        self.write_bytes(field_number, s.as_bytes());
    }
}

pub struct SimpleProtobufReader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> SimpleProtobufReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    pub fn has_more(&self) -> bool {
        self.offset < self.data.len()
    }

    pub fn next_field(&mut self) -> Option<(u32, u8)> {
        if !self.has_more() {
            return None;
        }
        let (tag, consumed) = VarintCodec::decode_u64(&self.data[self.offset..])?;
        self.offset += consumed;
        let field_num = (tag >> 3) as u32;
        let wire_type = (tag & 0x07) as u8;
        Some((field_num, wire_type))
    }

    pub fn read_uint64(&mut self) -> Option<u64> {
        let (val, consumed) = VarintCodec::decode_u64(&self.data[self.offset..])?;
        self.offset += consumed;
        Some(val)
    }

    pub fn read_bytes(&mut self) -> Option<&'a [u8]> {
        let (len, consumed) = VarintCodec::decode_u64(&self.data[self.offset..])?;
        self.offset += consumed;
        let len_usize = len as usize;
        if self.offset + len_usize > self.data.len() {
            return None;
        }
        let slice = &self.data[self.offset..self.offset + len_usize];
        self.offset += len_usize;
        Some(slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_roundtrip() {
        let mut buf = Vec::new();
        VarintCodec::encode_u64(300, &mut buf);
        let (val, consumed) = VarintCodec::decode_u64(&buf).unwrap();
        assert_eq!(val, 300);
        assert_eq!(consumed, buf.len());
    }

    #[test]
    fn test_protobuf_encoding() {
        let mut buf = Vec::new();
        {
            let mut writer = SimpleProtobufWriter::new(&mut buf);
            writer.write_uint64(1, 12345);
            writer.write_string(2, "throne-proxy");
        }

        let mut reader = SimpleProtobufReader::new(&buf);
        let (f1, w1) = reader.next_field().unwrap();
        assert_eq!(f1, 1);
        assert_eq!(w1, WIRE_TYPE_VARINT);
        assert_eq!(reader.read_uint64().unwrap(), 12345);

        let (f2, w2) = reader.next_field().unwrap();
        assert_eq!(f2, 2);
        assert_eq!(w2, WIRE_TYPE_LENGTH_DELIMITED);
        assert_eq!(reader.read_bytes().unwrap(), b"throne-proxy");
    }
}
