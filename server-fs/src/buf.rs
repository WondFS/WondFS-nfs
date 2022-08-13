extern crate alloc;
use crate::tl::tl;
use alloc::sync::Arc;
use std::collections::BTreeMap;

pub struct BufCache {
    pub capacity: usize,
    pub table: BTreeMap<u32, [u8; 4096]>,
    pub translation_layer: Arc<tl::TranslationLayer>,
}

impl BufCache {
    pub fn new(tl: Arc<tl::TranslationLayer>) -> BufCache {
        let capacity = 1024;
        BufCache {
            capacity,
            table: BTreeMap::new(),
            translation_layer: tl,
        }
    }
}

impl BufCache {
    pub fn read(&mut self, _: u8, address: u32) -> [u8; 4096] {
        let data = self.table.get(&address);
        if data.is_some() {
            return *data.unwrap();
        }
        let data = self.translation_layer.read(address);
        self.table.insert(address, data);
        data
    }

    pub fn read_advanced(&mut self, _: u8, address: u32, buf: &mut[u8]) {
        let data = self.table.get(&address);
        if data.is_some() {
            let data = data.unwrap();
            buf.copy_from_slice(&data[..buf.len()]);
            return;
        }
        self.translation_layer.read_advanced(address, buf);
        let mut data = [0; 4096];
        data[..buf.len()].copy_from_slice(buf);
        self.table.insert(address, data);
    }

    pub fn write(&mut self, _: u8, address: u32, data: &[u8; 4096]) {
        self.table.insert(address, *data);
        self.translation_layer.write(address, data);
    }

    pub fn erase(&mut self, _: u8, block_no: u32) {
        let start_address = block_no * 128;
        let end_address = (block_no + 1) * 128;
        for address in start_address..end_address {
            self.table.remove(&address);
        }
        self.translation_layer.erase(block_no);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn basics() {
        let mut tl = tl::TranslationLayer::new();
        tl.init();
        let mut cache = BufCache::new(Arc::new(tl));
        let data = [1; 4096];        
        cache.write(0, 100, &data);
        assert_eq!(cache.read(0, 100), [1; 4096]);
        cache.write(0, 100, &data);
        let data = cache.read(0, 100);
        assert_eq!(data, [1; 4096]);
        cache.erase(0, 0);
        let data = cache.read(0, 100);
        assert_eq!(data, [0; 4096]);
    }
}