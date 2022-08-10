use std::collections::HashMap;
use serde::Deserialize;

#[derive(Deserialize)]
struct ReplyData {
    status: u8,
    data: String,
}

pub struct DiskManager {
    pub address: String,
    pub port: u16,
    pub valid: bool,
}

impl DiskManager {
    pub fn new(address: String, port: u16) -> DiskManager {
        DiskManager {
            address,
            port,
            valid: true,
        }
    }
}

impl DiskManager {
    pub fn disk_read(&self, address: u32) -> [u8; 4096] {
        let mut map = HashMap::new();
        map.insert("address", address.to_string());
        let client = reqwest::blocking::Client::new();
        let url = format!("http://{}:{}/read", self.address, self.port);
        let res = client.post(url)
            .json(&map)
            .send().ok().unwrap();
        let data = res.json::<ReplyData>().ok().unwrap();
        if data.status == 0 {
            panic!();
        } else {
            let data = data.data.as_bytes().to_vec();
            let mut res = [0; 4096];
            res.copy_from_slice(&data);
            res
        }
    }

    pub fn disk_write(&mut self, address: u32, data: &[u8; 4096]) {
        let mut map = HashMap::new();
        map.insert("address", address.to_string());
        map.insert("data", std::str::from_utf8(data).unwrap().to_string());
        let client = reqwest::blocking::Client::new();
        let url = format!("http://{}:{}/write", self.address, self.port);
        client.post(url)
            .json(&map)
            .send().ok().unwrap();
    }

    pub fn disk_erase(&mut self, block_no: u32) {
        let mut map = HashMap::new();
        map.insert("address", block_no.to_string());
        let client = reqwest::blocking::Client::new();
        let url = format!("http://{}:{}/erase", self.address, self.port);
        client.post(url)
            .json(&map)
            .send().ok().unwrap();
    }
}