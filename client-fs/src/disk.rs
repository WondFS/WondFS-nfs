use std::fs::{File, OpenOptions};
use std::os::unix::prelude::FileExt;
use std::path::Path;

pub struct Disk {
    pub file: File,
    pub size: u32,
    pub block_num: u32,
}

impl Disk {
    pub fn new(path: String) -> Disk {
        let file_path = Path::new(&path);
        let ret = OpenOptions::new().read(true).write(true).open(file_path);
        if ret.is_err() {
            panic!();
        }
        let f = ret.ok().unwrap();
        let size = f.metadata().unwrap().len();
        let block_num = size / (128 * 4096);
        let size = size / 4096;
        Disk {
            size: size as u32,
            block_num: block_num as u32,
            file: f,
        }
    }
}

impl Disk {
    pub fn disk_read(&self, address: u32) -> [u8; 4096] {
        if address > self.size - 1 {
            panic!("FakeDisk: read at too big address");
        }
        let offset = address * 4096;
        let mut buf = [0; 4096];
        let ret = self.file.read_at(&mut buf, offset as u64);
        if ret.is_err() {
            panic!();
        }
        buf
    }
    
    pub fn disk_write(&mut self, address: u32, data: &[u8; 4096]) {
        if address > self.size - 1 {
            panic!("FakeDisk: write at too big address");
        }
        let offset = address * 4096;
        let ret = self.file.write_at(data, offset as u64);
        if ret.is_err() {
            panic!();
        }
    }

    pub fn disk_erase(&mut self, block_no: u32) {
        if block_no > self.block_num - 1 {
            panic!("FakeDisk: erase at too big block number");
        }
        let offset = block_no * 128 * 4096;
        let data = [0; 4096 * 128];
        let ret = self.file.write_at(&data, offset as u64);
        if ret.is_err() {
            panic!();
        }
    }
}
