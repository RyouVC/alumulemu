use binrw::{BinRead, BinReaderExt};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::str;

#[derive(BinRead)]
#[br(little)]
struct PFS0Header {
    magic: [u8; 4],
    num_files: u32,
    string_table_size: u32,
    reserved: u32,
}

#[derive(BinRead)]
#[br(little)]
struct PFS0Entry {
    data_offset: u64,
    data_size: u64,
    string_table_offset: u32,
    reserved: u32,
}

struct NspFile {
    name: String,
    offset: u64,
    size: u64,
}

pub fn get_title_id_from_nsp(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;

    // read PFS0 header
    let header: PFS0Header = file.read_le()?;

    // every nsp file includes this "PFS0" header, so if it doesnt have it, its not a valid nsp
    if &header.magic != b"PFS0" {
        return Err("Invalid NSP file format".into());
    }

    // read file entries
    let mut entries = Vec::new();
    for _ in 0..header.num_files {
        entries.push(file.read_le::<PFS0Entry>()?);
    }

    // read string table
    let mut string_table = vec![0u8; header.string_table_size as usize];
    file.read_exact(&mut string_table)?;

    // parse file entries and string table into NspFile structures
    let mut nsp_files = Vec::new();
    for entry in entries {
        let name_start = entry.string_table_offset as usize;
        let name_end = string_table[name_start..]
            .iter()
            .position(|&x| x == 0)
            .map(|p| name_start + p)
            .unwrap_or(string_table.len());

        let name = str::from_utf8(&string_table[name_start..name_end])?.to_string();

        nsp_files.push(NspFile {
            name,
            offset: entry.data_offset,
            size: entry.data_size,
        });
    }

    // find the ticket file (.tik)
    let ticket_file = nsp_files
        .iter()
        .find(|f| f.name.ends_with(".tik"))
        .ok_or("No ticket file found")?;

    println!("Found ticket file: {}", ticket_file.name);

    // the first 16 characters of the tik file are actually just our title id
    // (albeit in lowercase), so instead of attempting to parse the tik file
    // we can just use the filename

    let title_id = if ticket_file.name.len() >= 16 {
        ticket_file.name[..16].to_string()
    } else {
        return Err("Ticket filename too short".into());
    };

    println!("Extracted title ID: {}", title_id);

    // convert to uppercase
    let title_id = title_id.to_uppercase();

    Ok(title_id)
}
