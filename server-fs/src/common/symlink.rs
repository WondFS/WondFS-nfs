use crate::inode::inode;

pub fn write_symlink(inode: &inode::Inode, path: String) {
    if inode.stat.read().size != 0 {
        return;
    }
    // let mut data = vec![0; 4];
    // data[0] = (ino >> 24) as u8;
    // data[1] = (ino >> 16) as u8;
    // data[2] = (ino >> 8) as u8;
    // data[3] = ino as u8;
    let data = path.as_bytes().to_vec();
    inode.write(0, data.len(), &data);
}

// pub fn read_symlink(inode: &inode::Inode) -> Option<u32> {
//     if inode.stat.read().size != 4 {
//         return None
//     }
//     let mut data = vec![];
//     inode.read_all(&mut data);
//     Some(decode_u32(&data))
// }

// pub fn decode_u32(data: &Vec<u8>) -> u32 {
//     if data.len() != 4 {
//         panic!();
//     }
//     ((data[0] as u32) << 24) | ((data[1] as u32) << 16) | ((data[2] as u32) << 8) | (data[3] as u32)
// }

