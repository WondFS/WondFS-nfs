extern crate alloc;
use spin::RwLock;
use alloc::sync::Arc;
use std::collections::HashMap;
use std::time::SystemTime;
use crate::util::array::array;
use crate::write_buf;
use crate::tl::check_center;
use crate::driver::disk_manager;

use std::sync::mpsc;

const MAGIC_NUMBER_1: u32 = 0x2222ffff;
const MAGIC_NUMBER_2: u32 = 0x3333aaaa;

#[derive(PartialEq)]
pub enum MessageType {
    Read,
    Write,
    Erase,
    WriteImme,
    EraseImme,
}

pub struct Message {
    method: MessageType,
    pointer: u32,
    value: Option<[u8; 4096]>,
    channel1: Option<mpsc::Sender<[u8; 4096]>>,
    channel2: Option<mpsc::Sender<Vec<u8>>>,
}

pub enum BlockType {
    MappingTable,
    Signature,
    Used,
    Unused,
    Unknown,
}

pub fn judge_block_type(data: &Vec<u8>) -> BlockType {
    if data[0] == 0x22 && data[1] == 0x22 && data[2] == 0xff && data[3] == 0xff {
        return BlockType::MappingTable;
    }
    if data[119] == 0x33 && data[120] == 0x33 && data[121] == 0xaa && data[122] == 0xaa {
        return BlockType::Signature;
    }
    BlockType::Unknown
}

pub struct TranslationLayer {
    pub disk_manager: Arc<RwLock<disk_manager::DiskManager>>,
    pub write_cache: Arc<RwLock<write_buf::WriteCache>>,
    pub used_table: Arc<RwLock<HashMap<u32, bool>>>,
    pub map_v_table: Arc<RwLock<HashMap<u32, u32>>>,
    pub sign_block_map: Arc<RwLock<HashMap<u32, u32>>>,
    pub sign_offset_map: Arc<RwLock<HashMap<u32, u32>>>,
    pub sign_block_no: Arc<RwLock<u32>>,
    pub sign_block_offset: Arc<RwLock<u32>>,
    pub write_speed: Arc<RwLock<u32>>,
    pub read_speed: Arc<RwLock<u32>>,
    pub err_block_num: Arc<RwLock<u32>>,
    pub last_err_time: Arc<RwLock<SystemTime>>,
    pub block_num: u32,
    pub use_max_block_no: u32,
    pub max_block_no: u32,
    pub table_block_no: u32,
}

impl TranslationLayer {
    pub fn write_loop(&self) {
        loop {
            if !self.write_cache.read().need_sync() {
                return;
            }
            let data = self.write_cache.write().get_all();
            // self.write_sign(&data);
            let start_time = SystemTime::now();
            for (address, data) in data.into_iter() {
                let  block_no = address / 128;
                let offset = address % 128;
                let map_block_no = self.transfer(block_no);
                let map_address = map_block_no * 128 + offset;
                self.disk_manager.write().disk_write(map_address, &data); 
            }
            let end_time = SystemTime::now();
            let duration = end_time.duration_since(start_time).ok().unwrap().as_micros();
            self.update_write_speed(32 * 4, duration);
            self.write_cache.write().sync();
        }
    }
}

impl TranslationLayer {
    pub fn new() -> TranslationLayer {
        TranslationLayer {
            disk_manager: Arc::new(RwLock::new(disk_manager::DiskManager::new(true))),
            write_cache: Arc::new(RwLock::new(write_buf::WriteCache::new())),
            map_v_table: Arc::new(RwLock::new(HashMap::new())),
            used_table: Arc::new(RwLock::new(HashMap::new())),
            sign_block_map: Arc::new(RwLock::new(HashMap::new())),
            sign_offset_map: Arc::new(RwLock::new(HashMap::new())),
            sign_block_no: Arc::new(RwLock::new(1025)),
            sign_block_offset: Arc::new(RwLock::new(0)),
            block_num: 1224,
            use_max_block_no: 1023,
            max_block_no: 1223,
            table_block_no: 1024,
            write_speed: Arc::new(RwLock::new(0)),
            read_speed: Arc::new(RwLock::new(0)),
            err_block_num: Arc::new(RwLock::new(0)),
            last_err_time: Arc::new(RwLock::new(SystemTime::UNIX_EPOCH)),
        }
    }

    pub fn init(&mut self) {
        for block_no in self.use_max_block_no + 1..=self.max_block_no {
            let mut data = vec![0; 4096 * 128];
            self.disk_manager.read().disk_block_read(block_no, &mut data);
            self.init_with_block(block_no, &data);
        }
    }
    
    pub fn get_disk_speed(&self) -> (u32, u32) {
        (*self.read_speed.read(), *self.write_speed.read())
    }

    pub fn set_block_num(&mut self, block_num: u32) {
        self.block_num = block_num;
    }

    pub fn set_use_max_block_no(&mut self, use_max_block_no: u32) {
        self.use_max_block_no = use_max_block_no;
    }

    pub fn set_max_block_no(&mut self, max_block_no: u32) {
        self.max_block_no = max_block_no;
    }

    pub fn set_table_block_no(&mut self, table_block_no: u32) {
        self.table_block_no = table_block_no;
    }

    pub fn set_sign_block_no(&self, sign_block_no: u32) {
        *self.sign_block_no.write() = sign_block_no;
    }
}

impl TranslationLayer {
    pub fn read(&self, address: u32) -> [u8; 4096] {
        if self.write_cache.read().contains_address(address) {
            let data = self.write_cache.read().read(address).unwrap();
            return data;
        }
        self.disk_manager.read().disk_read(address)
    }

    pub fn read_advanced(&self, address: u32, buf: &mut [u8]) {
        if self.write_cache.read().contains_address(address) {
            let data = self.write_cache.read().read(address).unwrap();
            buf.copy_from_slice(&data[..buf.len()]);
            return;
        }
        self.disk_manager.read().disk_read_advanced(address, buf);
    }

    pub fn write(&self, address: u32, data: &[u8; 4096]) {
        self.write_cache.write().write(address, *data);
    }

    pub fn erase(&self, block_no: u32) {
        let start_index = block_no * 128;
        let end_index = (block_no + 1) * 128;
        for index in start_index..end_index {
            self.write_cache.write().recall_write(index);
            if self.sign_block_map.read().contains_key(&index) {
                self.sign_block_map.write().remove(&index);
                self.sign_offset_map.write().remove(&index);
            }
        }
        let map_block_no = self.transfer(block_no);
        self.disk_manager.write().disk_erase(map_block_no);
    }
}

impl TranslationLayer {
    fn init_with_block(&mut self, block_no: u32, data: &Vec<u8>) {
        let block_type = judge_block_type(&data);
        match block_type {
            BlockType::MappingTable => {
                let iter = MapDataRegion::new(&data);
                for entry in iter {
                    self.map_v_table.write().insert(entry.0, entry.1);
                    self.used_table.write().insert(entry.1, true);
                }
                self.used_table.write().insert(block_no, true);
                self.table_block_no = block_no;
            },
            BlockType::Signature => {
                let iter = SignDataRegion::new(&data);
                let mut len = 0;
                for (index, entry) in iter.enumerate() {
                    let address = check_center::CheckCenter::extract_address(&entry.to_vec());
                    if self.sign_block_map.read().contains_key(&address) {
                        *self.sign_block_map.write().get_mut(&address).unwrap() = block_no;
                        *self.sign_offset_map.write().get_mut(&address).unwrap() = index as u32;
                    } else {
                        self.sign_block_map.write().insert(address, block_no);
                        self.sign_offset_map.write().insert(address, index as u32);
                    }
                    len += 1;
                }
                self.used_table.write().insert(block_no, true);
                *self.sign_block_no.write() = block_no;
                *self.sign_block_offset.write() = len;
            },
            _ => (),
        }
    }

    fn check_block(&self, block_no: u32, data: &mut Vec<u8>, should_check: &Vec<bool>) -> bool {
        let mut flag = true;
        for index in 0..128 {
            let page = &data[index * 4096..(index + 1) * 4096];
            if !should_check[index] {
                continue;
            }
            let address = block_no * 128 + index as u32;
            let signature = self.get_address_sign(address);
            if signature.is_none() {
                continue;
            }
            if page == [0; 4096] {
                continue;
            }
            let ret = check_center::CheckCenter::check(page, &signature.as_ref().unwrap());
            if ret.0 == false {
                if ret.2 == None {
                    println!("{:?} {}", signature.unwrap(), index);
                    flag = false;
                    break;
                } else {
                    data[index * 4096..(index + 1) * 4096].copy_from_slice(&ret.2.unwrap());
                }
            }
        }
        if !flag {
            let new_block_no = self.find_next_block();
            self.used_table.write().insert(new_block_no, true);
            self.map_v_table.write().insert(block_no, new_block_no);
            *self.err_block_num.write() += 1;
            self.sync_map_v_table();
            return false;
        }
        true
    }

    fn check_page(&self, address: u32, data: &mut [u8]) -> bool {
        let signature = self.get_address_sign(address);
        if signature.is_none() {
            return true;
        }
        if *data == [0; 4096] {
            return true;
        }
        let ret = check_center::CheckCenter::check(data, &signature.as_ref().unwrap());
        if ret.0 == false {
            if ret.2 == None {
                return false;
            } else {
                data.copy_from_slice(&ret.2.unwrap());
            }
        }
        true
    }

    fn write_sign(&self, data: &Vec<(u32, [u8;4096])>) {
        if data.len() != 32 {
            panic!("TranslationLayer: write sign no available size");
        }
        let mut page_data = [0; 4096];
        if *self.sign_block_offset.read() / 32 == 127 {
            if !self.used_table.read().contains_key(&self.sign_block_no.read()) {
                self.used_table.write().insert(*self.sign_block_no.read(), true);
            }
            *self.sign_block_no.write() = self.find_next_block();
            *self.sign_block_offset.write() = 0;
        }
        let address = *self.sign_block_no.read() * 128 + *self.sign_block_offset.read() / 32;
        for (index, data) in data.iter().enumerate() {
            let signature = self.set_address_sign(&data.1, data.0);
            let start_index = index * 128;
            for (index, byte) in signature.iter().enumerate() {
                page_data[start_index + index] = *byte;
            }
            if self.sign_block_map.read().contains_key(&data.0) {
                *self.sign_block_map.write().get_mut(&data.0).unwrap() = *self.sign_block_no.read();
                *self.sign_offset_map.write().get_mut(&data.0).unwrap() = *self.sign_block_offset.read() + index as u32;
            } else {
                self.sign_block_map.write().insert(data.0, *self.sign_block_no.read());
                self.sign_offset_map.write().insert(data.0,  *self.sign_block_offset.read() + index as u32);
            }
        }
        *self.sign_block_offset.write() += 32;
        self.disk_manager.write().disk_write(address, &page_data);
    }

    fn transfer(&self, pla: u32) -> u32 {
        if self.map_v_table.read().contains_key(&pla) {
            *self.map_v_table.read().get(&pla).unwrap()
        } else {
            pla
        }
    }

    fn get_address_sign(&self, address: u32) -> Option<Vec<u8>> {
        let sign_block_map = self.sign_block_map.read();
        let sign_address = *sign_block_map.get(&address)?;
        let sign_offset_map = self.sign_offset_map.read();
        let offset = *sign_offset_map.get(&address)?;
        let mut ret = vec![0; 128];
        let mut data = vec![0; 4096];
        self.disk_manager.read().disk_read_advanced(sign_address*128+offset/32, &mut data);
        ret.copy_from_slice(&data[(offset%32*128) as usize..(offset%32*128+128) as usize]);
        Some(ret)
    }

    fn set_address_sign(&self, data: &[u8; 4096], address: u32) -> Vec<u8> {
        let sign_type = self.choose_sign_type();
        let sign = check_center::CheckCenter::sign(data, address, sign_type);
        sign
    }

    fn choose_sign_type(&self) -> check_center::CheckType {
        let err_ratio = *self.err_block_num.read() as f32 / self.block_num as f32;
        if err_ratio > 0.02 {
            return check_center::CheckType::Ecc;
        }
        let time = SystemTime::now();
        let duration = time.duration_since(*self.last_err_time.read()).ok().unwrap().as_secs();
        if duration < 60 * 60 * 12 {
            return check_center::CheckType::Ecc;
        }
        check_center::CheckType::Crc32
    }

    fn find_next_block(&self) -> u32 {
        for block_no in self.use_max_block_no+1..self.max_block_no {
            if block_no == self.table_block_no || block_no == *self.sign_block_no.read() {
                continue;
            }
            if self.used_table.read().contains_key(&block_no) {
                continue;
            }
            return block_no;
        }
        panic!("TranslationLayer: No available block to map")
    }

    fn sync_map_v_table(&self) {
        let mut data = array::Array1::<u8>::new(128 * 4096, 0);
        data.set(0, 0x22);
        data.set(1, 0x22);
        data.set(2, 0xff);
        data.set(3, 0xff);
        let mut index = 0;
        for (key, value) in self.map_v_table.read().iter() {
            let start_index = 8 + index * 8;
            let byte_1 = (*key >> 24) as u8;
            let byte_2 = (*key >> 16) as u8;
            let byte_3 = (*key >> 8) as u8;
            let byte_4 = *key as u8;
            data.set(start_index, byte_1);
            data.set(start_index + 1, byte_2);
            data.set(start_index + 2, byte_3);
            data.set(start_index + 3, byte_4);
            let byte_1 = (*value >> 24) as u8;
            let byte_2 = (*value >> 16) as u8;
            let byte_3 = (*value >> 8) as u8;
            let byte_4 = *value as u8;
            data.set(start_index + 4, byte_1);
            data.set(start_index + 5, byte_2);
            data.set(start_index + 6, byte_3);
            data.set(start_index + 7, byte_4);
            index += 1;
        }
        self.write_table_block(&data);
    }

    pub fn write_table_block(&self, data: &array::Array1::<u8>) {
        self.disk_manager.write().disk_erase(self.table_block_no);
        let mut index = 0;
        while index < 128 {
            let start_index = 4096 * index;
            let end_index = (index + 1) * 4096;
            let mut page = [0; 4096];
            for index in start_index..end_index {
                page[index - start_index] = data.get(index as u32);
            }
            self.disk_manager.write().disk_write(self.table_block_no * 128 + index as u32, &page);
            index += 1;
        }
    }

    fn update_read_speed(&self, size: u32, duration: u128) {
        let len = size * 1000000 / 1024;
        let duration = duration as u32;
        let speed = len / duration;
        let new_speed = 6 * speed / 10 + 4 * *self.read_speed.read() / 10;
        *self.read_speed.write() = new_speed;
    }

    fn update_write_speed(&self, size: u32, duration: u128) {
        let len = size * 1000000 / 1024;
        let duration = duration as u32;
        let speed = len / duration;
        let new_speed = 6 * speed / 10 + 4 * *self.write_speed.read() / 10;
        *self.write_speed.write() = new_speed;
    }
}

struct MapDataRegion<'a> {
    count: usize,
    data: &'a Vec<u8>,
}

impl MapDataRegion<'_> {
    fn new(data: &Vec<u8>) -> MapDataRegion {
        if data.len() != 128 {
            panic!("MapDataRegion: new not matched size");
        }
        MapDataRegion {
            count: 8,
            data,
        }
    }
}

impl Iterator for MapDataRegion<'_> {
    type Item = (u32, u32);
    fn next(&mut self) -> Option<Self::Item> {
        if self.count < 128 * 4096 {
            let byte_1 = (self.data[self.count] as u32) << 24;
            let byte_2 = (self.data[self.count + 1] as u32 as u32) << 16;
            let byte_3 = (self.data[self.count + 2] as u32) << 8;
            let byte_4 = self.data[self.count + 3] as u32;
            let lba = byte_1 + byte_2 + byte_3 + byte_4;
            let byte_1 = (self.data[self.count + 4] as u32) << 24;
            let byte_2 = (self.data[self.count + 5] as u32) << 16;
            let byte_3 = (self.data[self.count + 6] as u32) << 8;
            let byte_4 = self.data[self.count + 7] as u32;
            let pba = byte_1 + byte_2 + byte_3 + byte_4;
            self.count += 8;
            if lba == 0 && pba == 0 {
                None
            } else {
                Some((lba, pba))
            }
        } else {
            None
        }
    }
}
struct SignDataRegion<'a> {
    count: usize,
    data: &'a Vec<u8>,
}

impl SignDataRegion<'_> {
    fn new(data: &Vec<u8>) -> SignDataRegion {
        if data.len() != 128 {
            panic!("SignDataRegion: new not matched size");
        }
        SignDataRegion {
            count: 0,
            data,
        }
    }
}

impl Iterator for SignDataRegion<'_> {
    type Item = [u8; 128];
    fn next(&mut self) -> Option<Self::Item> {
        if self.count < 128 * 4096 {
            let mut data = [0; 128];
            data.copy_from_slice(&self.data[self.count..self.count + 128]);
            self.count += 128;
            if data == [0; 128] {
                None
            } else {
                Some(data)
            }
        } else {
            None
        }
    }
}