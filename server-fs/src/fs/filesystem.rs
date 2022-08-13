extern crate alloc;
use std::sync::atomic::AtomicU64;
use spin::RwLock;
use alloc::sync::Arc;
use std::thread;
use crate::kv::kv::KV;
use crate::tl::tl::TranslationLayer;
use crate::inode::inode_manager::InodeManager;
use crate::inode::inode;
use crate::common::directory;

pub struct WondFS {
    pub is_virtual: bool,
    pub kv: Arc<KV>,
    pub inode_manager: Option<Arc<RwLock<InodeManager>>>,
    pub tl: Arc<TranslationLayer>,
    pub next_file_handle: AtomicU64,
}

impl WondFS {
    pub fn new() -> Self {
        let mut tl = TranslationLayer::new();
        tl.init();
        let tl = Arc::new(tl);
        let a_tl = tl.clone();
        thread::spawn( move || {
            a_tl.write_loop();
        });
        let kv = KV::new(Arc::clone(&tl));
        kv.mount();
        let kv = Arc::new(kv);
        // let a_kv = kv.clone();
        // thread::spawn( move || {
        //     a_kv.background_gc();
        // });
        let inode_manager = InodeManager::new(Arc::clone(&kv));
        WondFS {
            tl,
            kv,
            is_virtual: false,
            inode_manager: Some(Arc::new(RwLock::new(inode_manager))),
            next_file_handle: AtomicU64::new(1),
        }
    }
}

impl WondFS {
    pub fn new_inode_file(&self) -> Option<Arc<inode::Inode>> {
        self.inode_manager.as_ref().unwrap().write().i_alloc()
    }

    pub fn new_inode_dir(&self, parent: u32) -> Option<Arc<inode::Inode>> {
        let inode = self.inode_manager.as_ref().unwrap().write().i_alloc()?;
        let mut stat = inode.get_stat();
        stat.file_type = inode::InodeFileType::Directory;
        inode.modify_stat(stat);
        directory::dir_link(&inode, inode.stat.read().ino, ".".to_string());
        directory::dir_link(&inode, parent, "..".to_string());
        Some(inode)
    }

    pub fn get_inode(&self, ino: u32) -> Option<Arc<inode::Inode>> {
        self.inode_manager.as_ref().unwrap().write().i_get(ino)
    }
}