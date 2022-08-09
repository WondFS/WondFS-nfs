extern crate alloc;
use spin::RwLock;
use alloc::sync::Arc;
use std::cmp::max;
use std::cmp::min;
use crate::buf;
use super::gc::gc_manager;
use super::component::bit;
use super::component::pit;
use super::component::journal;
use super::component::super_block;
use super::lsm_tree::lsm_tree;
// use serde::{Serialize, Deserialize};
use rkyv::ser::{Serializer, serializers::AllocSerializer};
use rkyv::{Archive, Deserialize, Serialize};

pub enum KVOperationsObject {
    MetaObject,
    DataObject,
    ExtraObject,
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, Clone, Copy)]
pub struct DataObjectValueEntry {
    pub len: usize,
    pub offset: usize,
    pub page_pointer: u32,
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
pub struct DataObjectValue {
    pub size: usize,
    pub entries: Vec<DataObjectValueEntry>,
}

pub struct KVManager {
    pub bit: bit::BIT,
    pub pit: pit::PIT,
    pub gc: gc_manager::GCManager,
    pub super_stat: super_block::SuperStat,
    pub journal: journal::Journal,
    pub buf: Arc<RwLock<buf::BufCache>>,
    pub lsm_tree: lsm_tree::LSMTree,
}

impl KVManager {
    pub fn get(&mut self, key: &String, off: usize, len: usize) -> Option<Vec<u8>> {
        let operation_type = KVManager::parse_key(key);
        match operation_type {
            KVOperationsObject::MetaObject => {
                let value = self.lsm_tree.get(&key.as_bytes().to_vec());
                if value.is_none() {
                    return None;
                }
                if len != 0 {
                    Some(value.unwrap()[off..off+len].to_vec())
                } else {
                    value
                }
            },
            KVOperationsObject::DataObject => {
                let value = self.lsm_tree.get(&key.as_bytes().to_vec());
                if value.is_none() {
                    return None;
                }
                let archived = unsafe { rkyv::archived_root::<DataObjectValue>(value.as_ref().unwrap()) };
                let mut data_object: DataObjectValue = archived.deserialize(&mut rkyv::Infallible).unwrap();
                // let mut data_object: DataObjectValue = serde_json::from_slice(&value.unwrap()).unwrap();
                if len != 0 {
                    if off + len > data_object.size {
                        return Some(self.read_data_object_all(&mut data_object)[off..].to_vec());
                    }
                    // KVManager::sort_data_object(&mut data_object);
                    let mut index = 0;
                    // println!("{:?}", data_object.entries);
                    for (i, entry) in data_object.entries.iter().enumerate() {
                        if off < entry.offset {
                            index = i - 1;
                            break;
                        }
                        if i == data_object.entries.len() - 1 {
                            index = data_object.entries.len() - 1;
                        }
                    }
                    let mut result = vec![];
                    let mut remain_num = len;
                    let data = self.read_data_object_entry(&data_object.entries[index]);
                    let read_num = min(data.len(), remain_num);
                    result.append(&mut data[off - data_object.entries[index].offset..off - data_object.entries[index].offset+read_num].to_vec());
                    remain_num -= read_num;
                    index += 1;
                    while remain_num != 0 {
                        let data = self.read_data_object_entry(&data_object.entries[index]);
                        let read_num = min(data.len(), remain_num);
                        result.append(&mut data[..read_num].to_vec());
                        remain_num -= read_num;
                        index += 1;
                    }
                    Some(result)
                } else {
                    Some(self.read_data_object_all(&mut data_object))
                }
            },
            KVOperationsObject::ExtraObject => {
                let value = self.lsm_tree.get(&key.as_bytes().to_vec());
                if value.is_none() {
                    return None;
                }
                if len != 0 {
                    Some(value.unwrap()[off..off+len].to_vec())
                } else {
                    value
                }
            },
        }
    }

    pub fn set(&mut self, key: &String, off: usize, len: usize, value: &Vec<u8>, extra_info: u32) -> Option<usize> {
        let operation_type = KVManager::parse_key(key);
        match operation_type {
            KVOperationsObject::MetaObject => {
                let pre_value = self.lsm_tree.get(&key.as_bytes().to_vec());
                if pre_value.is_none() || len == 0 {
                    self.lsm_tree.put(&key.as_bytes().to_vec(), value);
                } else {
                    let mut pre_value = pre_value.unwrap();
                    if pre_value.len() >= off + len {
                        pre_value[off..off+len].copy_from_slice(value);
                    } else {
                        pre_value.truncate(off);
                        pre_value.append(&mut value.clone());
                    }
                    self.lsm_tree.put(&key.as_bytes().to_vec(), &pre_value);
                }
                None
            },
            KVOperationsObject::DataObject => {
                let pre_value = self.lsm_tree.get(&key.as_bytes().to_vec());
                let mut data_object;
                if pre_value.is_none() {
                    data_object = DataObjectValue {
                        size: 0,
                        entries: vec![],
                    };
                } else {
                    let archived = unsafe { rkyv::archived_root::<DataObjectValue>(pre_value.as_ref().unwrap()) };
                    data_object = archived.deserialize(&mut rkyv::Infallible).unwrap();
                    // data_object = serde_json::from_slice(&pre_value.unwrap()).unwrap();
                }
                if len == 0 {
                    self.recycle_data_obect_all(&mut data_object);
                }
                self.set_data_object(&mut data_object, off, len, value, extra_info);
                let mut serializer = AllocSerializer::<0>::default();
                serializer.serialize_value(&data_object).unwrap();
                let value = serializer.into_serializer().into_inner().to_vec();
                // let value = serde_json::to_vec(&data_object).ok().unwrap();
                self.lsm_tree.put(&key.as_bytes().to_vec(), &value);
                Some(data_object.size)
            },
            KVOperationsObject::ExtraObject => {
                let pre_value = self.lsm_tree.get(&key.as_bytes().to_vec());
                if pre_value.is_none() || len == 0 {
                    self.lsm_tree.put(&key.as_bytes().to_vec(), value);
                } else {
                    let mut pre_value = pre_value.unwrap();
                    if pre_value.len() >= off + len {
                        pre_value[off..off+len].copy_from_slice(value);
                    } else {
                        pre_value.truncate(off);
                        pre_value.append(&mut value.clone());
                    }
                    self.lsm_tree.put(&key.as_bytes().to_vec(), &pre_value);
                }
                None
            },
        }
    }

    pub fn delete(&mut self, key: &String, off: usize, len: usize, extra_info: u32) -> Option<usize> {
        let operation_type = KVManager::parse_key(key);
        match operation_type {
            KVOperationsObject::MetaObject => {
                let pre_value = self.lsm_tree.get(&key.as_bytes().to_vec());
                if pre_value.is_none() {
                    return None;
                }
                if len != 0 {
                    let mut pre_value = pre_value.unwrap();
                    if pre_value.len() > off + len {
                        let mut rest_data = pre_value[off+len..].to_vec();
                        pre_value.truncate(off);
                        pre_value.append(&mut rest_data);
                    } else {
                        pre_value.truncate(off);
                    }
                    self.lsm_tree.put(&key.as_bytes().to_vec(), &pre_value);
                } else {
                    self.lsm_tree.delete(&key.as_bytes().to_vec());
                }
                None
            },
            KVOperationsObject::DataObject => {
                let pre_value = self.lsm_tree.get(&key.as_bytes().to_vec());
                if pre_value.is_none() {
                    return Some(0);
                }
                let archived = unsafe { rkyv::archived_root::<DataObjectValue>(pre_value.as_ref().unwrap()) };
                let mut data_object = archived.deserialize(&mut rkyv::Infallible).unwrap();
                // let mut data_object: DataObjectValue = serde_json::from_slice(&pre_value.unwrap()).unwrap();
                self.delete_data_object(&mut data_object, off, len, extra_info);
                if len != 0 {
                    let mut serializer = AllocSerializer::<0>::default();
                    serializer.serialize_value(&data_object).unwrap();
                    let value = serializer.into_serializer().into_inner().to_vec();
                    // let value = serde_json::to_vec(&data_object).ok().unwrap();
                    self.lsm_tree.put(&key.as_bytes().to_vec(), &value);
                    Some(data_object.size)
                } else {
                    self.lsm_tree.delete(&key.as_bytes().to_vec());
                    Some(0)
                }
            },
            KVOperationsObject::ExtraObject => {
                let pre_value = self.lsm_tree.get(&key.as_bytes().to_vec());
                if pre_value.is_none() {
                    return None;
                }
                if len != 0 {
                    let mut pre_value = pre_value.unwrap();
                    if pre_value.len() > off + len {
                        let mut rest_data = pre_value[off+len..].to_vec();
                        pre_value.truncate(off);
                        pre_value.append(&mut rest_data);
                    } else {
                        pre_value.truncate(off);
                    }
                    self.lsm_tree.put(&key.as_bytes().to_vec(), &pre_value);
                } else {
                    self.lsm_tree.delete(&key.as_bytes().to_vec());
                }
                None
            },
        }
    }

    pub fn parse_key(key: &String) -> KVOperationsObject {
        match &key[0..2] {
            "m:" => KVOperationsObject::MetaObject,
            "d:" => KVOperationsObject::DataObject,
            "e:" => KVOperationsObject::ExtraObject,
            _ => panic!(),
        }
    }
}

impl KVManager {
    pub fn set_data_object(&mut self, object: &mut DataObjectValue, off: usize, len: usize, value: &Vec<u8>, ino: u32) {
        if off > object.size {
            return;
        }
        let size = (len - 1) / 4096 + 1;
        let page_pointer = self.find_write_pos(size);
        let new_entry = DataObjectValueEntry {
            len,
            offset: off,
            page_pointer,
        };
        for i in 0..size {
            let start_index = 4096 * i;
            let end_index = 4096 * (i + 1);
            if i == size - 1 {
                let mut data = value[start_index..].to_vec();
                data.extend(vec![10; 4096 - data.len()]);
                self.write_page(page_pointer + i as u32, &self.trans(data), true);
            } else {
                self.write_page(page_pointer + i as u32, &self.trans(value[start_index..end_index].to_vec()), true);
            }
            self.update_bit(page_pointer + i as u32, true);
            self.update_pit(page_pointer + i as u32, ino);
        }
        let mut remove_list = vec![];
        let mut insert_index = -1;
        let mut second_entry = None;
        for (index, entry) in object.entries.clone().iter().enumerate() {
            if entry.offset + entry.len <= new_entry.offset {
                continue;
            } else if entry.offset >= new_entry.offset + new_entry.len {
                continue;
            }
            let valid_prev = max(0, new_entry.offset as i32 - entry.offset as i32) as usize;
            let valid_suffix = max(0, entry.offset as i32 + entry.len as i32 - new_entry.offset as i32 - new_entry.len as i32) as usize;
            if valid_prev == 0 {
                let size = (entry.len - 1) / 4096 + 1;
                for i in 0..size as u32 {
                    self.dirty_pit(entry.page_pointer + i);
                }
                remove_list.push(object.entries[index].page_pointer);
                if insert_index == -1 {
                    insert_index = index as i32;
                }
            } else {
                let size = (entry.len - 1) / 4096 + 1;
                let o_size = (valid_prev - 1) / 4096 + 1;
                for i in o_size as u32..size as u32 {
                    self.dirty_pit(entry.page_pointer + i);
                }
                object.entries[index].len = valid_prev;
                if insert_index == -1 {
                    insert_index = (index + 1) as i32;
                }
            }
            if valid_suffix > 0 {
                let data = self.read_data_object_entry(entry);
                let data = data[data.len()-valid_suffix..].to_vec();
                let size = (valid_suffix - 1) / 4096 + 1;
                let page_pointer = self.find_write_pos(size);
                let new_entry = DataObjectValueEntry {
                    len: valid_suffix,
                    offset: entry.offset + entry.len - valid_suffix,
                    page_pointer,
                };
                for i in 0..size {
                    let start_index = 4096 * i;
                    let end_index = 4096 * (i + 1);
                    if i == size - 1 {
                        let mut data = data[start_index..].to_vec();
                        data.extend(vec![10; 4096 - data.len()]);
                        self.write_page(page_pointer + i as u32, &self.trans(data), true);
                    } else {
                        self.write_page(page_pointer + i as u32, &self.trans(data[start_index..end_index].to_vec()), true);
                    }
                    self.update_bit(page_pointer + i as u32, true);
                    self.update_pit(page_pointer + i as u32, ino);
                }
                second_entry = Some(new_entry);
            }
        }
        for pointer in remove_list.iter() {
            for i in 0..object.entries.len() {
                if object.entries[i].page_pointer == *pointer {
                    object.entries.remove(i);
                    break;
                }
            }
        }
        if insert_index == -1 {
            insert_index = object.entries.len() as i32;
        }
        // if insert_index == 0 {
        //     if object.entries.len() == 0 {
        //         object.entries.push(new_entry);
        //     } else {
        //         object.entries.insert(0, object.entries[0]);
        //         object.entries[0] = new_entry;
        //     }
        // } else {
        //     object.entries.insert((insert_index - 1) as usize, new_entry);
        // }
        object.entries.insert(insert_index as usize, new_entry);
        if second_entry.is_some() {
            object.entries.insert(insert_index as usize + 1, second_entry.unwrap());
        }
        let mut len = 0;
        for entry in object.entries.iter() {
            len += entry.len;
        }
        object.size = len;
        // KVManager::sort_data_object(object);
    }

    pub fn delete_data_object(&mut self, object: &mut DataObjectValue, off: usize, len: usize, ino: u32) {
        if off >= object.size {
            return;
        }
        let mut remove_list = vec![];
        let mut insert_index = -1;
        let mut second_entry = None;
        for (index, entry) in object.entries.clone().iter().enumerate() {
            if entry.offset + entry.len <= off {
                continue
            } else if entry.offset >= off + len {
                object.entries[index].offset = entry.offset - len;
                continue
            } else {
                let valid_prev = max(0, off as i32 - entry.offset as i32) as usize;
                let valid_suffix = max(0, entry.offset as i32 + entry.len as i32 - off as i32 - len as i32) as usize;
                if valid_prev == 0 {
                    let size = (entry.len - 1) / 4096 + 1;
                    for i in 0..size as u32 {
                        self.dirty_pit(entry.page_pointer + i);
                    }
                    remove_list.push(object.entries[index].page_pointer);
                    if insert_index == -1 {
                        insert_index = index as i32;
                    }
                } else {
                    let size = (entry.len - 1) / 4096 + 1;
                    let o_size = (valid_prev - 1) / 4096 + 1;
                    for i in o_size as u32..size as u32 {
                        self.dirty_pit(entry.page_pointer + i);
                    }
                    object.entries[index].len = valid_prev;
                    if insert_index == -1 {
                        insert_index = (index + 1) as i32;
                    }
                }
                if valid_suffix > 0 {
                    let data = self.read_data_object_entry(entry);
                    let data = data[data.len()-valid_suffix..].to_vec();
                    let size = (valid_suffix - 1) / 4096 + 1;
                    let page_pointer = self.find_write_pos(size);
                    let new_entry = DataObjectValueEntry {
                        len: valid_suffix,
                        offset: entry.offset + entry.len - valid_suffix,
                        page_pointer,
                    };
                    for i in 0..size {
                        let start_index = 4096 * i;
                        let end_index = 4096 * (i + 1);
                        if i == size - 1 {
                            let mut data = data[start_index..].to_vec();
                            data.extend(vec![10; 4096 - data.len()]);
                            self.write_page(page_pointer + i as u32, &self.trans(data), true);
                        } else {
                            self.write_page(page_pointer + i as u32, &self.trans(data[start_index..end_index].to_vec()), true);
                        }
                        self.update_bit(page_pointer + i as u32, true);
                        self.update_pit(page_pointer + i as u32, ino);
                    }
                    second_entry = Some(new_entry);
                }
            }
        }
        for pointer in remove_list.iter() {
            for i in 0..object.entries.len() {
                if object.entries[i].page_pointer == *pointer {
                    object.entries.remove(i);
                    break;
                }
            }
        }
        // if insert_index == 0 {
        //     if object.entries.len() == 0 {
        //         object.entries.push(second_entry.unwrap());
        //     } else {
        //         object.entries.insert(0, object.entries[0]);
        //         object.entries[0] = second_entry.unwrap();
        //     }
        // } else {
        //     object.entries.insert((insert_index - 1) as usize, second_entry.unwrap());
        // }
        if second_entry.is_some() {
            object.entries.insert(insert_index as usize, second_entry.unwrap());
        }
        let mut len = 0;
        for entry in object.entries.iter() {
            len += entry.len;
        }
        object.size = len;
        // KVManager::sort_data_object(object);
    }
}

impl KVManager {
    pub fn read_data_object_all(&mut self, object: &mut DataObjectValue) -> Vec<u8> {
        // KVManager::sort_data_object(object);
        let mut result = vec![];
        for entry in object.entries.iter() {
            result.append(&mut self.read_data_object_entry(entry));
        }
        result
    }

    pub fn read_data_object_entry(&mut self, entry: &DataObjectValueEntry) -> Vec<u8> {
        let mut data = vec![0; entry.len];
        let mut size = 0;
        for i in 0..(entry.len-1)/4096+1 {
            // let page_data = self.read_page(entry.page_pointer + i as u32, true);
            if i == (entry.len-1)/4096 {
                let remain_num = entry.len - size;
                // for j in 0..remain_num {
                //     data[size + j] = page_data[j];
                // }
                self.read_page_advanced(entry.page_pointer + i as u32, true, &mut data[size..size+remain_num]);
            } else {
                // for j in 0..4096 {
                //     data[size + j] = page_data[j];
                // }
                self.read_page_advanced(entry.page_pointer + i as u32, true, &mut data[size..size+4096]);
                size += 4096;
            }
        }
        data
    }

    pub fn recycle_data_obect_all(&mut self, object: &mut DataObjectValue) {
        for entry in object.entries.iter() {
            let size = (entry.len - 1) / 4096 + 1;
            for i in 0..size as u32 {
                self.dirty_pit(entry.page_pointer + i);
            }
        }
        object.size = 0;
        object.entries.clear();
    }
}

impl KVManager {
    pub fn sort_data_object(object: &mut DataObjectValue) {
        let len = object.entries.len();
        for i in 0..len {
            for j in 0..len - 1 - i {
                let index_1 = object.entries[j].offset;
                let index_2 = object.entries[j+1].offset;
                if index_1 > index_2 {
                    let temp = object.entries[j];
                    object.entries[j] = object.entries[j+1];
                    object.entries[j+1] = temp;
                }
            }
        }
    }

    pub fn trans<T, const N: usize>(&self, v: Vec<T>) -> [T; N] {
        v.try_into()
            .unwrap_or_else(|v: Vec<T>| panic!("Expected a Vec of length{} but it was {}", N, v.len()))
    }
}