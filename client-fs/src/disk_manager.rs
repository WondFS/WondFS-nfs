use crate::disk;
use crate::fake_disk;

pub struct DiskManager {
    pub is_virtual: bool,
    pub fake_disk: Option<fake_disk::FakeDisk>,
    pub disk: Option<disk::Disk>,
}

impl DiskManager {
    pub fn new(is_virtual: bool, path: String) -> DiskManager {
        let fake_disk;
        let disk;
        if is_virtual {
            let block_num = 1224;
            fake_disk = Some(fake_disk::FakeDisk::new(block_num * 128));
            disk = None;
        } else {
            fake_disk = None;
            disk = Some(disk::Disk::new(path));
        }
        DiskManager {
            is_virtual,
            fake_disk,
            disk,
        }
    }
}

impl DiskManager {
    pub fn disk_read(&self, address: u32) -> [u8; 4096] {
        if self.is_virtual {
            return self.fake_disk.as_ref().unwrap().fake_disk_read(address);
        }
        return self.disk.as_ref().unwrap().disk_read(address);
    }

    pub fn disk_write(&mut self, address: u32, data: &[u8; 4096]) {
        if self.is_virtual {
            return self.fake_disk.as_mut().unwrap().fake_disk_write(address, data);
        }
        self.disk.as_mut().unwrap().disk_write(address, data);
    }

    pub fn disk_erase(&mut self, block_no: u32) {
        if self.is_virtual {
            return self.fake_disk.as_mut().unwrap().fake_disk_erase(block_no);
        }
        self.disk.as_mut().unwrap().disk_erase(block_no);
    }
}