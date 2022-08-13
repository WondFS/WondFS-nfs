use crate::compress::huffman;
use crate::compress::snappy;

#[derive(Copy, Clone, PartialEq)]
pub enum CompressType {
    Huffman,
    Snappy,
    None,
}

impl From<CompressType> for u8 {
    fn from(kind: CompressType) -> Self {
        match kind {
            CompressType::None => 0,
            CompressType::Snappy => 1,
            CompressType::Huffman => 2,
        }
    }
}

impl From<u8> for CompressType {
    fn from(kind: u8) -> Self {
        match kind {
            0 => CompressType::None,
            1 => CompressType::Snappy,
            2 => CompressType::Huffman,
            _ => panic!(),
        }
    }
}

pub trait Compress {
    fn decode(&mut self, bytes: &[u8]) -> Vec<u8>;
    fn encode(&mut self, bytes: &[u8]) -> Vec<u8>;
}

pub struct CompressManager {
    pub effect_huffman: u8,
    pub effect_snappy: u8,
    pub huffman: huffman::HuffmanCodec,
    pub snappy: snappy::Snappy,
}

impl CompressManager {
    pub fn new() -> Self {
        Self {
            effect_huffman: 0,
            effect_snappy: 0,
            huffman: huffman::HuffmanCodec::new(),
            snappy: snappy::Snappy::new(),
        }
    }

    pub fn encode(&mut self, bytes: &[u8]) -> (Vec<u8>, u8) {
        // let mut result = vec![];
        // let mut compress_type = CompressType::None;
        // let mut flag = false;
        // let mut used_algorithm = vec![];
        // while !flag {
        //     compress_type = self.choose_compress_type_except(&used_algorithm);
        //     match compress_type {
        //         CompressType::Huffman => {
        //             result = self.huffman.encode(bytes);
        //         },
        //         CompressType::Snappy => {
        //             result = self.snappy.encode(bytes);
        //         },
        //         CompressType::None => {
        //             result = bytes.to_owned();
        //         },
        //     }
        //     used_algorithm.push(compress_type);
        //     flag = CompressManager::judge_encode_effect(bytes, &result, compress_type);
        // }
        // let result = self.snappy.encode(bytes);
        // (result, CompressType::Snappy.into())
        (bytes.to_vec(), CompressType::None.into())
    }

    pub fn encode_with_type(&mut self, bytes: &[u8], compress_type: CompressType) -> Vec<u8> {
        match compress_type {
            CompressType::Huffman => {
                self.huffman.encode(bytes)
            },
            CompressType::Snappy => {
                self.snappy.encode(bytes)
            },
            CompressType::None => {
                bytes.to_owned()
            },
        }
    }

    pub fn decode(&mut self, bytes: &[u8], compress_type: u8) -> Vec<u8> {
        // let compress_type = compress_type.into();
        // match compress_type {
        //     CompressType::Huffman => {
        //         self.huffman.decode(bytes)
        //     },
        //     CompressType::Snappy => {
        //         self.snappy.decode(bytes)
        //     },
        //     CompressType::None => {
        //         bytes.to_owned()
        //     },
        // }
        // self.snappy.decode(bytes)
        bytes.to_vec()
    }

    pub fn choose_compress_type_except(&self, except: &Vec<CompressType>) -> CompressType {
        if !except.contains(&CompressType::Huffman) {
            return CompressType::Huffman;
        }
        if !except.contains(&CompressType::Snappy) {
            return CompressType::Snappy;
        }
        CompressType::None
    }

    pub fn get_compress_type_score(&self, compress_type: CompressType) -> u8 {
        match compress_type {
            CompressType::Huffman => {
                self.huffman.coefficient
            },
            CompressType::Snappy => {
                self.snappy.coefficient
            },
            CompressType::None => {
                1
            },
        }
    }

    pub fn judge_encode_effect(o_data: &[u8], data: &Vec<u8>, compress_type: CompressType) -> bool {
        if compress_type == CompressType::None {
            return true;
        }
        if o_data.len() / 4096 == data.len() / 4096 {
            return false;
        }
        let coeffi = o_data.len() * 100 / data.len();
        if coeffi > 90 {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn basics() {
        let data = "fsfjlahuhdwnf.v.sljp;jdqdsjdfhalkshdlhliqjdna,dnlawjdla.jdj.lskd.wnkak".as_bytes();
        let mut manager = CompressManager::new();
        let ret = manager.encode(&data);
        manager.decode(&ret.0, ret.1);
    }
}