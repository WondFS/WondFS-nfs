use crate::driver::disk;
use crate::driver::fake_disk;

pub struct DiskManager {
    pub is_virtual: bool,
    pub fake_disk: Option<fake_disk::FakeDisk>,
    pub disk: Option<disk::Disk>,
}

impl DiskManager {
    pub fn new(is_virtual: bool) -> DiskManager {
        let fake_disk;
        let disk;
        if is_virtual {
            let block_num = 1224;
            fake_disk = Some(fake_disk::FakeDisk::new(block_num * 128));
            disk = None;
        } else {
            fake_disk = None;
            disk = Some(disk::Disk::new());
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

    pub fn disk_read_advanced(&self, address: u32, buf: &mut[u8]) {
        if self.is_virtual {
            self.fake_disk.as_ref().unwrap().fake_disk_read_advanced(address, buf);
            return;
        }
        self.disk.as_ref().unwrap().disk_read_advanced(address, buf);
    }

    pub fn disk_block_read(&self, block_no: u32, buf: &mut[u8]) {
        if self.is_virtual {
            self.fake_disk.as_ref().unwrap().fake_disk_block_read_advanced(block_no, buf);
            return;
        }
        self.disk.as_ref().unwrap().disk_block_read(block_no, buf);
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn basics() {
        let mut manager = DiskManager::new(true);
        let data = [1; 4096];
        manager.disk_write(100, &data);
        let data = manager.disk_read(100);
        assert_eq!(data, [1; 4096]);
        let data =[2; 4096];
        manager.disk_write(256, &data);
        let data = manager.disk_read(256);
        assert_eq!(data, [2; 4096]);
        manager.disk_erase(2);
        let data = manager.disk_read(1);
        assert_eq!(data, [0; 4096]);
    }
}