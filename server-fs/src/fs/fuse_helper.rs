extern crate fuser;
use fuser::*;
use std::time::{UNIX_EPOCH, SystemTime, Duration};
use crate::inode::inode;

impl From<inode::InodeFileType> for fuser::FileType {
    fn from(kind: inode::InodeFileType) -> Self {
        match kind {
            inode::InodeFileType::File => fuser::FileType::RegularFile,
            inode::InodeFileType::Directory => fuser::FileType::Directory,
            inode::InodeFileType::Symlink => fuser::FileType::Symlink,
        }
    }
}

pub fn transfer_stat_to_attr(stat: inode::InodeStat) -> FileAttr {
    let size;
    if stat.size == 0 {
        size = 1;
    } else {
        size = stat.size;
    }
    FileAttr {
        ino: stat.ino as u64,
        size: stat.size as u64,
        blocks: ((size - 1) / 512 + 1) as u64,
        atime: system_time_from_time(stat.last_accessed as i64, 0),
        mtime: system_time_from_time(stat.last_modified as i64, 0),
        ctime: system_time_from_time(stat.last_metadata_changed as i64, 0),
        kind: stat.file_type.into(),
        perm: 0o777,
        nlink: stat.n_link as u32,
        uid: 0,
        gid: 0,
        rdev: 0,
        flags: 0,
        blksize: 512,
        padding: 0,
        crtime: UNIX_EPOCH,
    }
}

pub fn time_now() -> u32 {
    time_from_system_time(&SystemTime::now()).0 as u32
}

pub fn system_time_from_time(secs: i64, nsecs: u32) -> SystemTime {
    if secs >= 0 {
        UNIX_EPOCH + Duration::new(secs as u64, nsecs)
    } else {
        UNIX_EPOCH - Duration::new((-secs) as u64, nsecs)
    }
}

pub fn time_from_system_time(system_time: &SystemTime) -> (i64, u32) {
    match system_time.duration_since(UNIX_EPOCH) {
        Ok(duration) => (duration.as_secs() as i64, duration.subsec_nanos()),
        Err(before_epoch_error) => (
            -(before_epoch_error.duration().as_secs() as i64),
            before_epoch_error.duration().subsec_nanos(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let time = time_now();
        let systime = system_time_from_time(time as i64, 0);
        assert_eq!(time, time_from_system_time(&systime).0 as u32);
    }
}