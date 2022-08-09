mod fs;
mod tl;
mod kv;
mod buf;
mod util;
mod track;
mod inode;
mod driver;
mod common;
mod compress;
mod write_buf;
use std::env;
use fuser::MountOption;

fn main() {
    let mountpoint = env::args_os().nth(1).unwrap();
    let fs = fs::filesystem::WondFS::new();
    fuser::mount2(fs, mountpoint, &[MountOption::AutoUnmount]).unwrap();
}