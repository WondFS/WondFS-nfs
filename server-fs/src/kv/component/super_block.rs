use rkyv::{Archive, Deserialize, Serialize};

pub const MAGICNUMBER: u32 = 0x3bf7444d;

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
pub struct SuperStat {
    pub magic_code: u32,
    pub block_num: u32,
    pub super_block_num: u32,
    pub bit_block_num: u32,
    pub pit_block_num: u32,
    pub journal_block_num: u32,
    pub kv_block_num: u32,
    pub main_area_block_num: u32,
    pub reserved_block_num: u32,
    pub page_size: u32,
    pub page_num_per_block: u32,
}

impl SuperStat {
    pub fn new() -> SuperStat {
        SuperStat {
            magic_code: 0,
            block_num: 0,
            super_block_num: 0,
            bit_block_num: 0,
            pit_block_num: 0,
            journal_block_num: 0,
            kv_block_num: 0,
            main_area_block_num: 0,
            reserved_block_num: 0,
            page_size: 0,
            page_num_per_block: 0,
        }
    }

    pub fn get_bit_offset(&self) -> u32 {
        self.super_block_num
    }

    pub fn get_bit_size(&self) -> u32 {
        self.bit_block_num
    }

    pub fn get_pit_offset(&self) -> u32 {
        self.super_block_num + self.bit_block_num
    }

    pub fn get_pit_size(&self) -> u32 {
        self.pit_block_num
    }

    pub fn get_journal_offset(&self) -> u32 {
        self.super_block_num + self.bit_block_num + self.pit_block_num
    }

    pub fn get_journal_size(&self) -> u32 {
        self.journal_block_num
    }

    pub fn get_kv_offset(&self) -> u32 {
        self.super_block_num + self.bit_block_num + self.pit_block_num + self.journal_block_num
    }

    pub fn get_kv_size(&self) -> u32 {
        self.kv_block_num
    }

    pub fn get_main_offset(&self) -> u32 {
        self.super_block_num + self.bit_block_num + self.pit_block_num + self.journal_block_num + self.kv_block_num
    }

    pub fn get_main_size(&self) -> u32 {
        self.main_area_block_num
    }

    pub fn get_reserved_offset(&self) -> u32 {
        self.super_block_num + self.bit_block_num + self.pit_block_num + self.journal_block_num + self.kv_block_num + self.main_area_block_num
    }

    pub fn get_reserved_size(&self) -> u32 {
        self.reserved_block_num
    }

    pub fn get_page_size(&self) -> u32 {
        self.page_size
    }

    pub fn get_page_num_per_block(&self) -> u32 {
        self.page_num_per_block
    }

    pub fn get_block_num(&self) -> u32 {
        self.block_num
    }
}