use binrw::{BinRead, BinReaderExt};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::str;

#[derive(BinRead)]
#[br(little)]
pub struct PFS0Header {
    magic: [u8; 4],
    num_files: u32,
    string_table_size: u32,
    reserved: u32,
}

#[derive(BinRead)]
#[br(little)]
pub struct PFS0Entry {
    data_offset: u64,
    data_size: u64,
    string_table_offset: u32,
    reserved: u32,
}

/// Represents a file entry in an NSP package
pub struct NspFile {
    name: String,
    offset: u64,
    size: u64,
}

pub struct NspData {
    pub header: PFS0Header,
    pub files: Vec<NspFile>,
}

impl NspData {
    pub fn read_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = File::open(path)?;
        let header: PFS0Header = file.read_le()?;
        if &header.magic != b"PFS0" {
            return Err("Invalid NSP file format".into());
        }
        let mut files = Vec::new();

        let mut entries = Vec::new();
        for _ in 0..header.num_files {
            entries.push(file.read_le::<PFS0Entry>()?);
        }

        // read string table
        let mut string_table = vec![0u8; header.string_table_size as usize];
        file.read_exact(&mut string_table)?;

        for entry in entries {
            let name_start = entry.string_table_offset as usize;
            let name_end = string_table[name_start..]
                .iter()
                .position(|&x| x == 0)
                .map(|p| name_start + p)
                .unwrap_or(string_table.len());

            let name = str::from_utf8(&string_table[name_start..name_end])?.to_string();

            files.push(NspFile {
                name,
                offset: entry.data_offset,
                size: entry.data_size,
            });
        }

        Ok(NspData { header, files })
    }

    pub fn get_title_id(&self) -> Option<String> {
        let ticket_file = self.files.iter().find(|f| f.name.ends_with(".tik"))?;
        let title_id = if ticket_file.name.len() >= 16 {
            ticket_file.name[..16].to_string()
        } else {
            return None;
        };

        Some(title_id.to_uppercase())
    }

    /// Try to read the file data from a given entry?
    /// todo: figure out how to actually return the file, decrypt it, etc.
    pub fn read_file_data(&self, nsp_path: &Path, inner_file_path: &str) -> Option<Vec<u8>> {
        let inner_file = self.files.iter().find(|f| f.name == inner_file_path)?;
        let mut file = File::open(nsp_path).ok()?;
        let mut data = vec![0u8; inner_file.size as usize];
        file.seek(SeekFrom::Start(inner_file.offset)).ok()?;
        file.read_exact(&mut data).ok()?;
        Some(data)
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_get_title_id_from_nsp() {
//         let nsp_data = NspData::read_file(Path::new("games/balatro.nsp")).unwrap();
//         let title_id = nsp_data.get_title_id().unwrap();

//         println!("Title ID: {}", title_id);
//     }

//     #[test]
//     fn list_files_in_nsp() {
//         let nsp_data = NspData::read_file(Path::new("games/balatro.nsp")).unwrap();
//         for file in nsp_data.files {
//             println!("{}", file.name);
//         }
//     }
// }

pub fn get_title_id_from_nsp(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let nsp_data = NspData::read_file(Path::new(path))?;
    let title_id = nsp_data.get_title_id().ok_or("No title ID found in NSP")?;

    // convert to uppercase
    let title_id = title_id.to_uppercase();

    Ok(title_id)
}
