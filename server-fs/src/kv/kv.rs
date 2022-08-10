extern crate alloc;
use spin::RwLock;
use crate::tl::tl;
use alloc::sync::Arc;
use super::kv_manager::KVManager;
use rkyv::ser::{Serializer, serializers::AllocSerializer};
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
pub struct InodeMetadata {
    pub file_type: u8,
    pub ino: u32,
    pub size: u32,
    pub n_link: u8,
    pub last_accessed: u32,
    pub last_modified: u32,
    pub last_metadata_changed: u32,
}

pub struct KV {
    pub manager: Arc<RwLock<KVManager>>,
    pub max_ino: Arc<RwLock<u32>>,
}

impl KV {
    pub fn new(tl: Arc<tl::TranslationLayer>) -> KV {
        KV {
            manager: Arc::new(RwLock::new(KVManager::new(tl))),
            max_ino: Arc::new(RwLock::new(0)),
        }
    }

    pub fn mount(&self) {
        self.manager.write().mount();
    }

    pub fn allocate_indoe(&self, metadata: &mut InodeMetadata) -> u32 {
        *self.max_ino.write() += 1;
        metadata.ino = *self.max_ino.read();
        let key = format!("m:{}", *self.max_ino.read());
        let mut serializer = AllocSerializer::<0>::default();
        serializer.serialize_value(metadata).unwrap();
        let data = serializer.into_serializer().into_inner().to_vec();
        self.manager.write().set(&key, 0, 0, &data, 0);
        *self.max_ino.read()
    }

    pub fn delete_inode(&self, ino: u32) {
        let meta_key = format!("m:{}", ino);
        let data_key = format!("d:{}", ino);
        self.manager.write().delete(&meta_key, 0, 0, 0);
        self.manager.write().delete(&data_key, 0, 0, 0);
    }

    pub fn get_inode_metadata(&self, ino: u32) -> Option<InodeMetadata> {
        let key = format!("m:{}", ino);
        let data = self.manager.write().get(&key, 0, 0)?;
        let archived = unsafe { rkyv::archived_root::<InodeMetadata>(&data) };
        archived.deserialize(&mut rkyv::Infallible).ok()
    }

    pub fn set_inode_metadata(&self, ino: u32, metadata: &InodeMetadata) {
        let key = format!("m:{}", ino);
        let mut serializer = AllocSerializer::<0>::default();
        serializer.serialize_value(metadata).unwrap();
        let data = serializer.into_serializer().into_inner().to_vec();
        self.manager.write().set(&key, 0, 0, &data, 0);
    }

    pub fn get_inode_data(&self, ino: u32, off: usize, len: usize) -> Option<Vec<u8>> {
        let key = format!("d:{}", ino);
        let data = self.manager.write().get(&key, off, len);
        if data.is_none() {
            return Some(vec![]);
        }
        data
    }

    pub fn set_inode_data(&self, ino: u32, off: usize, len: usize, value: &Vec<u8>) -> usize {
        let mut metadata = self.get_inode_metadata(ino).unwrap();
        let key = format!("d:{}", ino);
        let size = self.manager.write().set(&key, off, len, value, metadata.ino).unwrap();
        metadata.size = size as u32;
        self.set_inode_metadata(ino, &metadata);
        size
    }

    pub fn delete_inode_data(&self, ino: u32, off: usize, len: usize) -> usize {
        let mut metadata = self.get_inode_metadata(ino).unwrap();
        let key = format!("d:{}", ino);
        let size = self.manager.write().delete(&key, off, len, metadata.ino).unwrap();
        metadata.size = size as u32;
        self.set_inode_metadata(ino, &metadata);
        size
    }

    pub fn get_extra_value(&self, key: String) -> Option<Vec<u8>> {
        let key = format!("e:{}", key);
        self.manager.write().get(&key, 0, 0)
    }

    pub fn set_extra_value(&mut self, key: String, value: &Vec<u8>) {
        let key = format!("e:{}", key);
        self.manager.write().set(&key, 0, 0, value, 0);
    }

    pub fn deleete_extra_value(&self, key: String) {
        let key = format!("e:{}", key);
        self.manager.write().delete(&key, 0, 0, 0);
    }

    pub fn background_gc(&self) {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(10));
            self.manager.write().background_gc();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        #[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
        struct Test {
            int: u8,
            string: String,
            option: Option<Vec<i32>>,
        }
        let value = Test {
            int: 42,
            string: "hello world".to_string(),
            option: Some(vec![1, 2, 3, 4]),
        };
        use rkyv::ser::{Serializer, serializers::AllocSerializer};
        let mut serializer = AllocSerializer::<0>::default();
        serializer.serialize_value(&value).unwrap();
        let bytes = serializer.into_serializer().into_inner().to_vec();
        let archived = unsafe { rkyv::archived_root::<Test>(&bytes) };
        let deserialized: Test = archived.deserialize(&mut rkyv::Infallible).unwrap();
        assert_eq!(deserialized, value);
    }
}