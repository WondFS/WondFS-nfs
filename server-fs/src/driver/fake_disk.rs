use crate::kv::component::super_block::*;
use rkyv::ser::{Serializer, serializers::AllocSerializer};

pub struct FakeDisk {
    pub size: u32,
    pub block_num: u32,
    pub data: Vec<[u8; 4096]>,
}

impl FakeDisk {
    pub fn new(size: u32) -> FakeDisk {
        let mut data = vec![];
        if size % 128 != 0 {
            panic!("FakeDisk: not available size")
        }
        for _ in 0..size {
            data.push([0; 4096]);
        }
        let block_num = size / 128;
        let kv_ratio = 0.15;
        let main_ratio = 0.6;
        let super_stat = SuperStat {
            magic_code: MAGICNUMBER,
            block_num: block_num as u32,
            super_block_num: 1,
            bit_block_num: 2,
            pit_block_num: 2,
            journal_block_num: 1,
            kv_block_num: (block_num as f64 * kv_ratio) as u32,
            main_area_block_num:(block_num as f64 * main_ratio) as u32,
            reserved_block_num: block_num as u32 - 6 - (block_num as f64 * kv_ratio) as u32 - (block_num as f64 * main_ratio) as u32,
            page_size: 4096,
            page_num_per_block: 128,
        };
        let mut serializer = AllocSerializer::<0>::default();
        serializer.serialize_value(&super_stat).unwrap();
        let mut stat_data = serializer.into_serializer().into_inner().to_vec();
        let len = stat_data.len();
        let mut stat_len: [u8; 4]  = [0; 4];
        stat_len[0] = (len >> 24) as u8;
        stat_len[1] = (len >> 16) as u8;
        stat_len[2] = (len >> 8) as u8;
        stat_len[3] = len as u8;
        data[0][0..4].copy_from_slice(&stat_len);
        data[0][4..4+len].copy_from_slice(&stat_data);
        FakeDisk {
            size,
            data,
            block_num,
        }
    }
}

impl FakeDisk {
    pub fn fake_disk_read(&self, address: u32) -> [u8; 4096] {
        if address > self.size - 1 {
            panic!("FakeDisk: read at too big address");
        }
        self.data[address as usize]
    }

    pub fn fake_disk_read_advanced(&self, address: u32, buf: &mut[u8]) {
        if address > self.size - 1 {
            panic!("FakeDisk: read at too big address");
        }
        buf.copy_from_slice(&self.data[address as usize][..buf.len()]);
    }

    pub fn fake_disk_block_read_advanced(&self, block_no: u32, buf: &mut[u8]) {
        std::thread::sleep(std::time::Duration::from_micros(50));
        if block_no > self.block_num - 1 {
            panic!("FakeDisk: read at too big block_no");
        }
        let start_index = 128 * block_no;
        for i in 0..128 {
            buf[i*4096..(i+1)*4096].copy_from_slice(&self.data[start_index as usize+i]);
        }
    }
    
    pub fn fake_disk_write(&mut self, address: u32, data: &[u8; 4096]) {
        std::thread::sleep(std::time::Duration::from_micros(50));
        if address > self.size - 1 {
            panic!("FakeDisk: write at too big address");
        }
        let o_data = self.data[address as usize];
        if o_data != [0; 4096] {
            panic!("FakeDisk: write at not clean address");
        }
        self.data[address as usize] = *data;
    }

    pub fn fake_disk_erase(&mut self, block_no: u32) {
        std::thread::sleep(std::time::Duration::from_micros(50));
        if block_no > self.block_num - 1 {
            panic!("FakeDisk: erase at too big block number");
        }
        let start_index = block_no * 128;
        let end_index = (block_no + 1) * 128;
        for index in start_index..end_index {
            self.data[index as usize] = [0; 4096];
        }
    }
}