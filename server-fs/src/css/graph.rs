use std::collections::HashMap;
use std::collections::BinaryHeap;
use std::collections::HashSet;
use crate::kv::kv::KV;
use rkyv::ser::{Serializer, serializers::AllocSerializer};
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct Node {
    node_id: usize,
    node_ino: Vec<u32>,
    node_hash: Vec<bool>,
    edges: Vec<Edge>,
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct Edge {
    end_node_id: usize,
    weight: usize,
}

pub struct Graph {
    nodes: Vec<Node>,
    map: HashMap<usize, usize>,   // node_id => index
    ino_map: HashMap<u32, usize>, // ino => node_id
    curr_max_node_id: usize,
    kv: KV,
}

impl Graph {
    pub fn add_node(&mut self, node_ino: u32, node_hash: Vec<bool>) {
        let ret = self.contains_node_hash(&node_hash);
        if ret.is_some() {
            let index = ret.unwrap();
            if !self.nodes[index].node_ino.contains(&node_ino) {
                self.ino_map.insert(node_ino, self.nodes[index].node_id);
                self.nodes[index].node_ino.push(node_ino);
            }
            return;
        }
        self.curr_max_node_id += 1;
        self.kv.set_extra_value("max_node_id".to_string(), &Graph::encode_max_node_id(self.curr_max_node_id));
        let node_id = self.curr_max_node_id;
        self.insert_node(node_id, node_ino, node_hash);
    }

    pub fn delete_node_by_ino(&mut self, node_ino: u32) {
        let ret = self.ino_map.get(&node_ino);
        if ret.is_none() {
            return;
        }
        let node_id = *ret.unwrap();
        let index = *self.map.get(&node_id).unwrap();
        if self.nodes[index].node_ino.len() == 1 {
            self.delete_node(index);
        } else {
            for i in 0..self.nodes[index].node_ino.len() {
                if self.nodes[index].node_ino[i] == node_ino {
                    self.nodes[index].node_ino.remove(i);
                    break;
                }
            }
            let data = Graph::encode_node(&self.nodes[index]);
            self.kv.set_extra_value(format!("e:node:{}", self.nodes[index].node_id), &data);
        }
    }

    pub fn delete_node_by_hash(&mut self, node_hash: Vec<bool>) {
        let ret = self.contains_node_hash(&node_hash);
        if ret.is_none() {
            return;
        }
        self.delete_node(ret.unwrap());
    }

    pub fn edit_node(&mut self, node_ino: u32, new_node_hash: Vec<bool>) {
        self.delete_node_by_ino(node_ino);
        self.add_node(node_ino, new_node_hash);
    }

    pub fn query_node_by_ino(&mut self, node_ino: u32, radii: usize) -> Option<Vec<u32>> {
        let ret = self.ino_map.get(&node_ino);
        if ret.is_none() {
            return None;
        }
        let node_id = *ret.unwrap();
        let index = *self.map.get(&node_id).unwrap();
        let ret = self.query_node(index, radii);
        if ret.is_none() {
            return None;
        }
        let mut result = vec![];
        for index in ret.unwrap() {
            for i in 0..self.nodes[index].node_ino.len() {
                result.push(self.nodes[index].node_ino[i]);
            }
        }
        Some(result)
    }

    pub fn query_node_by_node_hash(&mut self, node_hash: Vec<bool>, radii: usize) -> Option<Vec<u32>> {
        let ret = self.contains_node_hash(&node_hash);
        if ret.is_none() {
            return None;
        }
        let ret = self.query_node(ret.unwrap(), radii);
        if ret.is_none() {
            return None;
        }
        let mut result = vec![];
        for index in ret.unwrap() {
            for i in 0..self.nodes[index].node_ino.len() {
                result.push(self.nodes[index].node_ino[i]);
            }
        }
        Some(result)
    }

}

impl Graph {
    pub fn new(kv: KV) -> Graph {
        Graph {
            kv,
            nodes: vec![],
            map: HashMap::new(),
            ino_map: HashMap::new(),
            curr_max_node_id: 0,
        }
    }

    pub fn build(&mut self) {
        let ret = self.kv.get_extra_value("max_node_id".to_string());
        if ret.is_none() {
            self.curr_max_node_id = 0;
        } else {
            self.curr_max_node_id = Graph::decode_max_node_id(ret.unwrap());
        }
        for id in 0..self.curr_max_node_id {
            let data = self.kv.get_extra_value(format!("e:node:{}", id));
            if data.is_some() {
                let node = Graph::decode_node(data.unwrap());
                for node_ino in node.node_ino {
                    self.insert_node(node.node_id, node_ino, node.node_hash.clone());
                }
            }
        }
    }

    pub fn contains_node_hash(&self, node_hash: &Vec<bool>) -> Option<usize> {
        for index in 0..self.nodes.len() {
            if *node_hash == self.nodes[index].node_hash {
                return Some(index);
            }
        }
        None
    }

    pub fn insert_node(&mut self, node_id: usize, node_ino: u32, node_hash: Vec<bool>) {
        let ret = self.contains_node_hash(&node_hash);
        if ret.is_some() {
            let index = ret.unwrap();
            if !self.nodes[index].node_ino.contains(&node_ino) {
                self.ino_map.insert(node_ino, self.nodes[index].node_id);
                self.nodes[index].node_ino.push(node_ino);
            }
            return;
        }
        let mut node = Node {
            node_id,
            node_hash,
            node_ino: vec![node_ino],
            edges: vec![],
        };
        let mut min_weight = usize::MAX;
        let mut min_node_index = vec![];
        let mut potential_node_index = vec![];
        let mut potential_edge_weight = vec![];
        for index in 0..self.nodes.len() {
            let weight = Graph::compute_edge_weight(&node, &self.nodes[index]);
            if weight < min_weight {
                min_weight = weight;
                min_node_index.clear();
                min_node_index.push(index);
            } else if weight == min_weight {
                min_node_index.push(index);
            }
            if weight <= 2 {
                potential_node_index.push(index);
                potential_edge_weight.push(weight);
            }
        }
        if min_weight < 2 {
            for index in 0..potential_node_index.len() {
                let edge = Edge {
                    end_node_id: self.nodes[potential_node_index[index]].node_id,
                    weight: potential_edge_weight[index],
                };
                node.edges.push(edge);
                let edge = Edge {
                    end_node_id: node.node_id,
                    weight: potential_edge_weight[index],
                };
                self.nodes[potential_node_index[index]].edges.push(edge);
            }
        } else {
            for index in 0..min_node_index.len() {
                let edge = Edge {
                    end_node_id: self.nodes[min_node_index[index]].node_id,
                    weight: min_weight,
                };
                node.edges.push(edge);
                let edge = Edge {
                    end_node_id: node.node_id,
                    weight: min_weight,
                };
                self.nodes[min_node_index[index]].edges.push(edge);
            }
        }
        self.map.insert(node.node_id, self.nodes.len());
        self.ino_map.insert(node.node_ino[0], node.node_id);
        self.nodes.push(node.clone());
        let data = Graph::encode_node(&node);
        self.kv.set_extra_value(format!("e:node:{}", node.node_id), &data);
    }

    pub fn delete_node(&mut self, index: usize) {
        let node = self.nodes.remove(index);
        self.map.remove(&node.node_id);
        for ino in node.node_ino {
            self.ino_map.remove(&ino);
        }
        for (k, v) in self.map.clone().iter() {
            if *v > index {
                *self.map.get_mut(&k).unwrap() = *v - 1;
            }
        }
        for egde in node.edges {
            let another_node_id = egde.end_node_id;
            let index = *self.map.get(&another_node_id).unwrap();
            for i in 0..self.nodes[index].edges.len() {
                if node.node_id == self.nodes[index].edges[i].end_node_id {
                    self.nodes[index].edges.remove(i);
                    break;
                }
            }
        }
        self.kv.deleete_extra_value(format!("e:node:{}", node.node_id));
    }

    pub fn query_node(&mut self, index: usize, radii: usize) -> Option<Vec<usize>> {
        let mut queue: BinaryHeap<(usize, usize)> = BinaryHeap::new();
        let mut ret_set = HashSet::new();
        let mut visited_set = HashSet::new();
        for i in 0..self.nodes[index].edges.len() {
            if self.nodes[index].edges[i].weight <= radii {
                queue.push((self.nodes[index].edges[i].end_node_id, self.nodes[index].edges[i].weight));
            }
        }
        visited_set.insert(self.nodes[index].node_id);
        while let Some((node_id, dis)) = queue.pop() {
            let index = *self.map.get(&node_id).unwrap();
            for i in 0..self.nodes[index].edges.len() {
                if self.nodes[index].edges[i].weight + dis <= radii && !visited_set.contains(&self.nodes[index].edges[i].end_node_id) {
                    queue.push((self.nodes[index].edges[i].end_node_id, self.nodes[index].edges[i].weight + dis));
                }
            }
            ret_set.insert(index);
            visited_set.insert(self.nodes[index].node_id);
        }
        if ret_set.len() != 0 {
            let ret = Vec::from_iter(ret_set);
            return Some(ret);
        }
        None
    }

    pub fn compute_edge_weight(node1: &Node, node2: &Node) -> usize {
        let mut weight = 0;
        for index in 0..node1.node_hash.len() {
            if node1.node_hash[index] != node2.node_hash[index] {
                weight += 1;
            }
        }
        weight
    }

    pub fn encode_max_node_id(max_node_id: usize) -> Vec<u8> {
        let mut a;
        let mut b;
        let mut result = vec![];
        b = max_node_id % 256;
        a = max_node_id / 256;
        result.push(b as u8);
        while a != 0 {
            b = a % 256;
            a = a / 256;
            result.push(b as u8);
        }
        result
    }

    pub fn decode_max_node_id(data: Vec<u8>) -> usize {
        let mut weight = 1;
        let mut result = 0;
        for i in 0..data.len() {
            result += data[i] as usize * weight;
            weight *= 256;
        }
        result
    }

    pub fn encode_node(node: &Node) -> Vec<u8> {
        let mut serializer = AllocSerializer::<0>::default();
        serializer.serialize_value(node).unwrap();
        serializer.into_serializer().into_inner().to_vec()
    }

    pub fn decode_node(data: Vec<u8>) -> Node {
        let archived = unsafe { rkyv::archived_root::<Node>(&data) };
        archived.deserialize(&mut rkyv::Infallible).ok().unwrap()
    }
}