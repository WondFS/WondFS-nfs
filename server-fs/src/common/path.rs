extern crate alloc;
use spin::RwLock;
use alloc::sync::Arc;
use crate::inode::inode;
use crate::inode::inode_manager;
use crate::common::directory;

pub fn skip_elem(path: String) -> Option<(String, String)> {
    let path = path.as_str();
    let mut index = 0;
    let temp_index;
    let len;
    for c in path.chars() {
        if c != '/' {
            break;
        }
        index += 1;
    }
    if index == path.len() {
        return None;
    }
    temp_index = index;
    while path[index..index+1] != '/'.to_string() {
        index += 1;
        if index == path.len() {
            break;
        }
    }
    len = index - temp_index;
    for c in path[index..].chars() {
        if c != '/' {
            break;
        }
        index += 1;
    }
    Some((path[index..].to_string(), path[temp_index..temp_index+len].to_string()))
}

pub fn name_x(i_manager: Arc<RwLock<inode_manager::InodeManager>>, path: String, name: &mut String, name_i_parent: bool) -> Option<inode_manager::InodeLink> {
    let path = &mut path.clone();
    let mut ip;
    let mut next;
    if path.len() == 0 {
        return None;
    }
    // if path[0..1] == '/'.to_string() {
    //     ip = i_manager.write().i_get(1).unwrap();
    // } else {
    //     return None;
    // }
    ip = i_manager.write().i_get(1).unwrap();
    loop {
        let res = skip_elem(path.clone());
        if res.is_none() {
            (*path, *name) = ("".to_string(), "".to_string());
            break;
        }
        (*path, *name) = res.unwrap();
        if ip.stat.read().file_type != inode::InodeFileType::Directory {
            return None;
        }
        if name_i_parent && path == "" {
            return Some(ip);
        }
        let res = directory::dir_lookup(&ip, name.clone());
        if res.is_none() {
            return None;
        }
        next = i_manager.write().i_get(res.unwrap().0).unwrap();
        ip = next;
    }
    if name_i_parent {
        return None;
    }
    return Some(ip);
}

pub fn name_i(i_manager: Arc<RwLock<inode_manager::InodeManager>>, path: String) -> Option<inode_manager::InodeLink> {
    let mut name = "".to_string();
    name_x(i_manager, path, &mut name, false)
}

pub fn name_i_parent(i_manager: Arc<RwLock<inode_manager::InodeManager>>, path: String, name: &mut String) -> Option<inode_manager::InodeLink> {
    name_x(i_manager, path, name, true)
}