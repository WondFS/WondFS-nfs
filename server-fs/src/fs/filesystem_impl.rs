extern crate fuser;
extern crate libc;
use fuser::*;
use std::cmp::min;
use std::ffi::OsStr;
use std::os::unix::prelude::OsStrExt;
use std::time::Duration;
use std::sync::atomic::Ordering;
use libc::ENOENT;
use crate::inode::inode;
use crate::common::directory;
use crate::common::symlink;
use crate::common::path;
use super::fuse_helper::*;
use super::filesystem::*;

pub const FILE_HANDLE_READ_BIT: u64 = 1 << 63;
pub const FILE_HANDLE_WRITE_BIT: u64 = 1 << 62;

const TTL: Duration = Duration::new(1, 0);

impl WondFS {
    fn allocate_next_file_handle(&self, read: bool, write: bool) -> u64 {
        let mut fh = self.next_file_handle.fetch_add(1, Ordering::SeqCst);
        assert!(fh < FILE_HANDLE_WRITE_BIT && fh < FILE_HANDLE_READ_BIT);
        if read {
            fh |= FILE_HANDLE_READ_BIT;
        }
        if write {
            fh |= FILE_HANDLE_WRITE_BIT;
        }
        fh
    }
}

impl Filesystem for WondFS {
    fn init(&mut self, _req: &Request<'_>, _config: &mut KernelConfig) -> Result<(), libc::c_int> {
        let mut inode = self.inode_manager.as_ref().unwrap().write().i_alloc().unwrap();
        assert!(inode.stat.read().ino == FUSE_ROOT_ID as u32);
        let mut stat = inode.get_stat();
        stat.file_type = inode::InodeFileType::Directory;
        stat.size = 0;
        stat.ref_cnt = 0;
        stat.n_link = 2;
        stat.last_accessed = time_now();
        stat.last_modified = time_now();
        stat.last_metadata_changed = time_now();
        inode.modify_stat(stat);
        directory::dir_link(&mut inode, FUSE_ROOT_ID as u32, ".".to_string());
        self.inode_manager.as_ref().unwrap().write().i_put(inode);
        Ok(())
    }

    fn lookup(&mut self, _req: &Request<'_>, _parent: u64, _name: &std::ffi::OsStr, reply: ReplyEntry) {
        let parent = _parent as u32;
        let name = _name.to_str().unwrap().to_string();
        if name == "tls" {
            reply.error(ENOENT);
            return;
        }
        if name == "haswell" {
            reply.error(ENOENT);
            return;
        }
        if name == "x86_64" {
            reply.error(ENOENT);
            return;
        }
        if name[0..3] == "lib".to_string() {
            reply.error(ENOENT);
            return;
        }
        println!("lookup {} {}", parent, name);
        let parent_inode = self.get_inode(parent);
        if parent_inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        let ino = directory::dir_lookup(parent_inode.as_ref().unwrap(), name);
        if ino.is_none() {
            reply.error(ENOENT);
            return;
        }
        let inode = self.get_inode(ino.unwrap().0 as u32);
        if inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        let stat = inode.as_ref().unwrap().get_stat();
        let attr = transfer_stat_to_attr(stat);
        self.inode_manager.as_ref().unwrap().write().i_put(parent_inode.unwrap());
        self.inode_manager.as_ref().unwrap().write().i_put(inode.unwrap());
        reply.entry(&TTL, &attr, 0);
    }

    fn forget(&mut self, _req: &Request, _ino: u64, _nlookup: u64) {}

    fn getattr(&mut self, _req: &Request<'_>, _ino: u64, reply: ReplyAttr) {
        let ino = _ino as u32;
        let inode = self.get_inode(ino);
        println!("getattr {}", ino);
        match inode {
            Some(inode) => {
                let stat = inode.get_stat();
                let attr = transfer_stat_to_attr(stat);
                self.inode_manager.as_ref().unwrap().write().i_put(inode);
                reply.attr(&TTL, &attr);
            },
            None => {
                reply.error(ENOENT);
            },
        }
    }

    fn setattr(&mut self, _req: &Request<'_>, _ino: u64, _mode: Option<u32>, _uid: Option<u32>, _gid: Option<u32>, _size: Option<u64>, _atime: Option<TimeOrNow>, _mtime: Option<TimeOrNow>, _ctime: Option<std::time::SystemTime>, _fh: Option<u64>, _crtime: Option<std::time::SystemTime>, _chgtime: Option<std::time::SystemTime>, _bkuptime: Option<std::time::SystemTime>, _flags: Option<u32>, reply: ReplyAttr) {
        let ino = _ino as u32;
        let inode = self.get_inode(ino);
        println!("setattr {}", ino);
        if inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        let inode = inode.unwrap();
        if _mode.is_some() {
            reply.error(ENOENT);
            return;
        }
        if _uid.is_some() || _gid.is_some() {
            reply.error(ENOENT);
            return;
        }
        if let Some(size) = _size {
            let len = (inode.stat.read().size - size as u32) as usize;
            inode.truncate(size as usize, len);
        }
        let now = time_now();
        if let Some(atime) = _atime {
            let mut stat = inode.get_stat();
            stat.last_accessed = match atime {
                TimeOrNow::SpecificTime(time) => time_from_system_time(&time).0 as u32,
                TimeOrNow::Now => now,
            };
            stat.last_metadata_changed = now;
            inode.modify_stat(stat);
        }
        if let Some(mtime) = _mtime {
            let mut stat = inode.get_stat();
            stat.last_modified = match mtime {
                TimeOrNow::SpecificTime(time) => time_from_system_time(&time).0 as u32,
                TimeOrNow::Now => now,
            };
            stat.last_metadata_changed = now;
            inode.modify_stat(stat);
        }
        let stat = inode.get_stat();
        let attr = transfer_stat_to_attr(stat);
        self.inode_manager.as_ref().unwrap().write().i_put(inode);
        reply.attr(&TTL, &attr);
    }

    fn readlink(&mut self, _req: &Request<'_>, _ino: u64, reply: ReplyData) {
        let ino =  _ino as u32;
        println!("readlink {}", ino);
        let inode = self.get_inode(ino);
        match inode {
            Some(inode) => {
                // let exact_ino = symlink::read_symlink(&inode);
                // println!("exact_ino: {:?}", exact_ino);
                // if exact_ino.is_none() {
                    // reply.error(ENOENT);
                    // return;
                // }
                // let exact_inode = self.get_inode(exact_ino.unwrap());
                // if exact_inode.is_none() {
                    // reply.error(ENOENT);
                    // return;
                // }
                let mut data = vec![];
                inode.read_all(&mut data);
                println!("aa {:?}", data);
                println!("bb {:?}", std::str::from_utf8(&data).ok().unwrap().to_string());
                self.inode_manager.as_ref().unwrap().write().i_put(inode);
                // self.inode_manager.as_ref().unwrap().write().i_put(exact_inode.unwrap());
                reply.data(&data);
            },
            None => {
                reply.error(ENOENT);
            },
        }
    }

    fn mknod(&mut self, _req: &Request<'_>, _parent: u64, _name: &std::ffi::OsStr, mut _mode: u32, _umask: u32, _rdev: u32, reply: ReplyEntry) {
        let file_type = _mode & libc::S_IFMT as u32;
        if file_type != libc::S_IFREG as u32 && file_type != libc::S_IFDIR as u32 {
            reply.error(ENOENT);
            return;
        }
        let parent = _parent as u32;
        let name = _name.to_str().unwrap().to_string();
        println!("mknod {} {}", parent, name);
        let mut parent_inode = self.get_inode(parent);
        if parent_inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        if directory::dir_lookup(parent_inode.as_ref().unwrap(), name.clone()).is_some() {
            reply.error(libc::ENOENT);
            return;
        }
        let mut stat = parent_inode.as_ref().unwrap().get_stat();
        stat.last_modified = time_now();
        stat.last_metadata_changed = time_now();
        parent_inode.as_ref().unwrap().modify_stat(stat);
        let mut inode = self.new_inode_file();
        if inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        let ino = inode.as_ref().unwrap().stat.read().ino;
        let mut stat = inode.as_ref().unwrap().get_stat();
        stat.file_type = as_file_kind(_mode);
        stat.size = 0;
        stat.ref_cnt = 0;
        stat.last_accessed = time_now();
        stat.last_modified = time_now();
        stat.last_metadata_changed = time_now();
        if stat.file_type == inode::InodeFileType::File {
            stat.n_link = 1;
        }
        if stat.file_type == inode::InodeFileType::Directory {
            stat.n_link = 2;
        }
        inode.as_ref().unwrap().modify_stat(stat);
        if inode.as_ref().unwrap().stat.read().file_type == inode::InodeFileType::Directory {
            directory::dir_link(inode.as_mut().unwrap(), ino, ".".to_string());
            directory::dir_link(inode.as_mut().unwrap(), parent, "..".to_string());
        }
        directory::dir_link(parent_inode.as_mut().unwrap(), ino, name);
        let stat = inode.as_ref().unwrap().get_stat();
        let attr = transfer_stat_to_attr(stat);
        self.inode_manager.as_ref().unwrap().write().i_put(parent_inode.unwrap());
        self.inode_manager.as_ref().unwrap().write().i_put(inode.unwrap());
        reply.entry(&TTL, &attr, 0);
    }

    fn mkdir(&mut self, _req: &Request<'_>, _parent: u64, _name: &OsStr, mut _mode: u32, _umask: u32, reply: ReplyEntry) {
        let parent = _parent as u32;
        let name = _name.to_str().unwrap().to_string();
        println!("mkdir {} {}", parent, name);
        let mut parent_inode = self.get_inode(parent);
        if parent_inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        if directory::dir_lookup(parent_inode.as_ref().unwrap(), name.clone()).is_some() {
            reply.error(ENOENT);
            return;
        }
        let mut stat = parent_inode.as_ref().unwrap().get_stat();
        stat.last_modified = time_now();
        stat.last_metadata_changed = time_now();
        parent_inode.as_ref().unwrap().modify_stat(stat);
        let mut inode = self.new_inode_file();
        if inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        let ino = inode.as_ref().unwrap().stat.read().ino;
        let mut stat = inode.as_ref().unwrap().get_stat();
        stat.file_type = inode::InodeFileType::Directory;
        stat.size = 0;
        stat.ref_cnt = 0;
        stat.n_link = 2;
        stat.last_accessed = time_now();
        stat.last_modified = time_now();
        stat.last_metadata_changed = time_now();
        inode.as_ref().unwrap().modify_stat(stat);
        directory::dir_link(inode.as_mut().unwrap(), ino, ".".to_string());
        directory::dir_link(inode.as_mut().unwrap(), parent, "..".to_string());
        directory::dir_link(parent_inode.as_mut().unwrap(), ino, name);
        let stat = inode.as_ref().unwrap().get_stat();
        let attr = transfer_stat_to_attr(stat);
        self.inode_manager.as_ref().unwrap().write().i_put(parent_inode.unwrap());
        self.inode_manager.as_ref().unwrap().write().i_put(inode.unwrap());
        reply.entry(&TTL, &attr, 0);
    }

    fn unlink(&mut self, _req: &Request<'_>, _parent: u64, _name: &std::ffi::OsStr, reply: ReplyEmpty) {
        let parent = _parent as u32;
        let name = _name.to_str().unwrap().to_string();
        println!("unlink {} {}", parent, name);
        let mut parent_inode = self.get_inode(parent);
        if parent_inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        let ino = directory::dir_lookup(parent_inode.as_ref().unwrap(), name.clone());
        if ino.is_none() {
            reply.error(ENOENT);
            return;
        }
        let ino = ino.unwrap().0 as u32;
        let inode = self.get_inode(ino);
        if inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        let mut stat = parent_inode.as_ref().unwrap().get_stat();
        stat.last_modified = time_now();
        stat.last_metadata_changed = time_now();
        parent_inode.as_ref().unwrap().modify_stat(stat);
        directory::dir_unlink(parent_inode.as_mut().unwrap(), ino, name);
        let mut stat = inode.as_ref().unwrap().get_stat();
        stat.n_link -= 1;
        stat.last_metadata_changed = time_now();
        inode.as_ref().unwrap().modify_stat(stat);
        if inode.as_ref().unwrap().get_stat().n_link == 0 {
            inode.as_ref().unwrap().delete();
        }
        self.inode_manager.as_ref().unwrap().write().i_put(parent_inode.unwrap());
        self.inode_manager.as_ref().unwrap().write().i_put(inode.unwrap());
        reply.ok();
    }

    fn rmdir(&mut self, _req: &Request<'_>, _parent: u64, _name: &std::ffi::OsStr, reply: ReplyEmpty) {
        let parent = _parent as u32;
        let name = _name.to_str().unwrap().to_string();
        println!("rmdir {} {}", parent, name);
        let mut parent_inode = self.get_inode(parent);
        if parent_inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        let ino = directory::dir_lookup(parent_inode.as_ref().unwrap(), name.clone());
        if ino.is_none() {
            reply.error(ENOENT);
            return;
        }
        let ino = ino.unwrap().0 as u32;
        let inode = self.get_inode(ino);
        if inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        if inode.as_ref().unwrap().stat.read().size > 259 * 2 {
            reply.error(ENOENT);
            return;
        }
        let mut stat = parent_inode.as_ref().unwrap().get_stat();
        stat.last_modified = time_now();
        stat.last_metadata_changed = time_now();
        parent_inode.as_ref().unwrap().modify_stat(stat);
        directory::dir_unlink(parent_inode.as_mut().unwrap(), ino, name);
        let mut stat = inode.as_ref().unwrap().get_stat();
        stat.n_link = 0;
        stat.last_metadata_changed = time_now();
        inode.as_ref().unwrap().modify_stat(stat);
        inode.as_ref().unwrap().delete();
        self.inode_manager.as_ref().unwrap().write().i_put(parent_inode.unwrap());
        self.inode_manager.as_ref().unwrap().write().i_put(inode.unwrap());
        reply.ok();
    }
    
    fn rename(&mut self, _req: &Request<'_>, _parent: u64, _name: &OsStr, _newparent: u64, _newname: &OsStr, _flags: u32, reply: ReplyEmpty) {
        reply.error(ENOENT);
    }

    fn link(&mut self, _req: &Request<'_>, _ino: u64, _newparent: u64, _newname: &std::ffi::OsStr, reply: ReplyEntry) {
        let ino = _ino as u32;
        let newparent = _newparent as u32;
        let newname = _newname.to_str().unwrap().to_string();
        println!("link {} {} {}", ino, newparent, newname);
        let mut parent_inode = self.get_inode(newparent);
        if parent_inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        let inode = self.get_inode(ino);
        if inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        directory::dir_link(parent_inode.as_mut().unwrap(), ino, newname);
        let mut stat = inode.as_ref().unwrap().get_stat();
        stat.n_link += 1;
        stat.last_metadata_changed = time_now();
        inode.as_ref().unwrap().modify_stat(stat);
        let attr = transfer_stat_to_attr(stat);
        self.inode_manager.as_ref().unwrap().write().i_put(parent_inode.unwrap());
        self.inode_manager.as_ref().unwrap().write().i_put(inode.unwrap());
        reply.entry(&TTL, &attr, 0);
    }

    fn open(&mut self, _req: &Request<'_>, _ino: u64, _flags: i32, reply: ReplyOpen) {
        let ino = _ino as u32;
        let inode = self.get_inode(ino);
        println!("open {}", ino);
        match inode {
            Some(inode) => {
                inode.stat.write().ref_cnt += 1;
                self.inode_manager.as_ref().unwrap().write().i_put(inode);
                reply.opened(self.allocate_next_file_handle(true, true), 0);
            },
            None => {
                reply.error(ENOENT);
            },
        }
    }

    fn read(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _offset: i64, _size: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyData) {
        let ino =  _ino as u32;
        let offset = _offset as u32;
        let size = _size as u32;
        println!("read {} {} {}", ino, offset, size);
        let inode = self.get_inode(ino);
        match inode {
            Some(inode) => {
                if offset >= inode.get_stat().size {
                    reply.error(ENOENT);
                    return;
                }
                let mut data = vec![];
                let read_size = min(size, inode.get_stat().size - offset);
                inode.read(offset as usize, read_size as usize, &mut data);
                self.inode_manager.as_ref().unwrap().write().i_put(inode);
                reply.data(&data);
            },
            None => {
                reply.error(ENOENT);
            },
        }
    }

    fn write(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _offset: i64, _data: &[u8], _write_flags: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyWrite) {
        let ino = _ino as u32;
        let offset = _offset as u32;
        let data = _data;
        let inode = self.get_inode(ino);
        println!("write {} {} {}", ino, offset, data.len());
        match inode {
            Some(inode) => {
                if offset > inode.get_stat().size {
                    reply.error(ENOENT);
                    return;
                }
                inode.write(offset as usize, data.len(), &data.to_vec());
                self.inode_manager.as_ref().unwrap().write().i_put(inode);
                reply.written(data.len() as u32);
            },
            None => {
                reply.error(ENOENT);
            },
        }
    }

    fn release(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _flags: i32, _lock_owner: Option<u64>, _flush: bool, reply: ReplyEmpty) {
        let ino = _ino as u32;
        let inode = self.get_inode(ino);
        println!("release {}", ino);
        if inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        inode.as_ref().unwrap().stat.write().ref_cnt -= 1;
        self.inode_manager.as_ref().unwrap().write().i_put(inode.unwrap());
        reply.ok();
    }

    fn opendir(&mut self, _req: &Request<'_>, _ino: u64, _flags: i32, reply: ReplyOpen) {
        let ino = _ino as u32;
        let inode = self.get_inode(ino);
        println!("opendir {}", ino);
        match inode {
            Some(inode) => {
                inode.stat.write().ref_cnt += 1;
                self.inode_manager.as_ref().unwrap().write().i_put(inode);
                reply.opened(self.allocate_next_file_handle(true, true), 1);
            },
            None => {
                reply.error(ENOENT);
            },
        }   
    }  

    fn readdir(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _offset: i64, mut reply: ReplyDirectory) {
        let ino = _ino as u32;
        let offset = _offset as i32;
        let inode = self.get_inode(ino);
        println!("readdir {} {}", ino, offset);
        if inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        let mut data = vec![];
        inode.as_ref().unwrap().read_all(&mut data);
        let iter = directory::DirectoryParser::new(&data);
        for (index, entry) in iter.skip(offset as usize).enumerate() {
            // println!("{}", entry.ino);
            let file_type = self.get_inode(entry.ino).unwrap().stat.read().file_type;
            println!("{:?}", file_type);
            let buffer_full: bool = reply.add(
                entry.ino as u64,
                offset as i64 + index as i64 + 1,
                file_type.into(),
                OsStr::from_bytes(entry.file_name.as_bytes()),
            );
            if buffer_full {
                break;
            }
        }
        self.inode_manager.as_ref().unwrap().write().i_put(inode.unwrap());
        reply.ok()
    }

    fn releasedir(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _flags: i32, reply: ReplyEmpty) {
        let ino = _ino as u32;
        let inode = self.get_inode(ino);
        println!("release {}", ino);
        if inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        inode.as_ref().unwrap().stat.write().ref_cnt -= 1;
        self.inode_manager.as_ref().unwrap().write().i_put(inode.unwrap());
        reply.ok();
    }

    // fn statfs(&mut self, _req: &Request, _ino: u64, reply: ReplyStatfs) {
    //     warn!("statfs() implementation is a stub");
    //     // TODO: real implementation of this
    //     reply.statfs(
    //         10_000,
    //         10_000,
    //         10_000,
    //         1,
    //         10_000,
    //         BLOCK_SIZE as u32,
    //         MAX_NAME_LENGTH,
    //         BLOCK_SIZE as u32,
    //     );
    // }

    fn access(&mut self, _req: &Request<'_>, _ino: u64, _mask: i32, reply: ReplyEmpty) {
        let ino = _ino as u32;
        println!("access {}", ino);
        let inode = self.get_inode(ino);
        if inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        self.inode_manager.as_ref().unwrap().write().i_put(inode.unwrap());
        reply.ok();
    }

    fn create(&mut self, _req: &Request<'_>, _parent: u64, _name: &std::ffi::OsStr, mut _mode: u32, _umask: u32, _flags: i32, reply: ReplyCreate) {
        let parent = _parent as u32;
        let name = _name.to_str().unwrap().to_string();
        println!("create {} {}", parent, name);
        let mut parent_inode = self.get_inode(parent);
        if parent_inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        let ino = directory::dir_lookup(parent_inode.as_ref().unwrap(), name.clone());
        if ino.is_some() {
            reply.error(ENOENT);
            return;
        }
        let mut stat = parent_inode.as_ref().unwrap().get_stat();
        stat.last_modified = time_now();
        stat.last_metadata_changed = time_now();
        parent_inode.as_ref().unwrap().modify_stat(stat);
        let mut inode = self.new_inode_file();
        if inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        let ino = inode.as_ref().unwrap().stat.read().ino;
        let mut stat = inode.as_ref().unwrap().get_stat();
        stat.file_type = as_file_kind(_mode);
        stat.size = 0;
        stat.ref_cnt = 1;
        stat.last_accessed = time_now();
        stat.last_modified = time_now();
        stat.last_metadata_changed = time_now();
        if stat.file_type == inode::InodeFileType::File {
            stat.n_link = 1;
        }
        if stat.file_type == inode::InodeFileType::Directory {
            stat.n_link = 2;
        }
        inode.as_ref().unwrap().modify_stat(stat);
        if inode.as_ref().unwrap().stat.read().file_type == inode::InodeFileType::Directory {
            directory::dir_link(inode.as_mut().unwrap(), ino, ".".to_string());
            directory::dir_link(inode.as_mut().unwrap(), parent, "..".to_string());
        }
        directory::dir_link(parent_inode.as_mut().unwrap(), ino, name);
        // println!("{}", parent_inode.as_ref().unwrap().stat.read().size);
        let stat = inode.as_ref().unwrap().get_stat();
        let attr = transfer_stat_to_attr(stat);
        self.inode_manager.as_ref().unwrap().write().i_put(parent_inode.unwrap());
        self.inode_manager.as_ref().unwrap().write().i_put(inode.unwrap());
        reply.created(
            &TTL,
            &attr,
            0,
            self.allocate_next_file_handle(true, true),
            0,
        );
    }

    fn symlink(&mut self, _req: &Request<'_>, _parent: u64, _name: &OsStr, _link: &std::path::Path, reply: ReplyEntry) {
        let parent = _parent as u32;
        let name = _name.to_str().unwrap().to_string();
        let path = _link.to_str().unwrap().to_string();
        // let inode = path::name_i(self.inode_manager.as_ref().unwrap().clone(), path);
        // if inode.is_none() {
            // reply.error(ENOENT);
            // return;
        // }
        // let exact_ino = inode.unwrap().stat.read().ino;
        // println!("{}", exact_ino);
        let mut parent_inode = self.get_inode(parent);
        if parent_inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        if directory::dir_lookup(parent_inode.as_ref().unwrap(), name.clone()).is_some() {
            reply.error(libc::ENOENT);
            return;
        }
        let mut stat = parent_inode.as_ref().unwrap().get_stat();
        stat.last_modified = time_now();
        stat.last_metadata_changed = time_now();
        parent_inode.as_ref().unwrap().modify_stat(stat);
        let inode = self.new_inode_file();
        if inode.is_none() {
            reply.error(ENOENT);
            return;
        }
        let ino = inode.as_ref().unwrap().stat.read().ino;
        let mut stat = inode.as_ref().unwrap().get_stat();
        stat.file_type = inode::InodeFileType::Symlink;
        stat.size = 0;
        stat.ref_cnt = 0;
        stat.last_accessed = time_now();
        stat.last_modified = time_now();
        stat.last_metadata_changed = time_now();
        inode.as_ref().unwrap().modify_stat(stat);
        directory::dir_link(parent_inode.as_mut().unwrap(), ino, name);
        symlink::write_symlink(&inode.as_ref().unwrap().clone(), path);
        let stat = inode.as_ref().unwrap().get_stat();
        let attr = transfer_stat_to_attr(stat);
        self.inode_manager.as_ref().unwrap().write().i_put(parent_inode.unwrap());
        self.inode_manager.as_ref().unwrap().write().i_put(inode.unwrap());
        reply.entry(&TTL, &attr, 0);
    }
}

pub fn as_file_kind(mut mode: u32) -> inode::InodeFileType {
    mode &= libc::S_IFMT as u32;
    if mode == libc::S_IFREG as u32 {
        return inode::InodeFileType::File;
    } else if mode == libc::S_IFLNK as u32 {
        return inode::InodeFileType::Symlink;
    } else if mode == libc::S_IFDIR as u32 {
        return inode::InodeFileType::Directory;
    } else {
        unimplemented!("{}", mode);
    }
}