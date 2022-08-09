use std::collections::BTreeMap;

pub struct WriteCache {
    pub table: BTreeMap<u32, [u8; 4096]>,
    pub sync: bool,
    pub size: usize,
}

impl WriteCache {
    pub fn new() -> WriteCache {
        WriteCache {
            size: 0,
            sync: false,
            table: BTreeMap::new(),
        }
    }

    pub fn get_size(&self) -> u32 {
        self.size as u32
    }

    pub fn contains_address(&self, address: u32) -> bool {
        self.table.contains_key(&address)
    }

    pub fn need_sync(&self) -> bool {
        self.sync
    }

    pub fn sync(&mut self) {
        if self.size < 32 {
            self.sync = false;
        }
    }
}

impl WriteCache {
    pub fn write(&mut self, address: u32, data: [u8; 4096]) {
        self.table.insert(address, data);
        if self.table.len() >= 32 {
            self.sync = true;
        }
    }

    pub fn read(&self, address: u32) -> Option<[u8; 4096]> {
        let data = self.table.get(&address);
        if data.is_some() {
            Some(*data.unwrap())
        } else {
            None
        }
    }

    pub fn get_all(&mut self) -> Vec<(u32, [u8; 4096])> {
        let mut entries: Vec<(u32, [u8; 4096])> = Vec::new();
        for entry in self.table.clone() {
            entries.push((entry.0, entry.1));
            self.table.remove(&entry.0);
            self.size -= 1;
            if entries.len() == 32 {
                break;
            }
        }
        entries
    }

    pub fn recall_write(&mut self, address: u32) {
        if self.table.contains_key(&address) {
            self.table.remove(&address);
        }
    }
}

#[derive(Clone, Copy)]
pub struct WriteBuf {
    pub address: u32,
    pub data: [u8; 4096],
}

impl WriteBuf {
    fn new(address: u32, data: [u8; 4096]) -> WriteBuf {
        WriteBuf {
            address,
            data,
        }
    }
}