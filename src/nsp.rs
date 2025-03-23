use nx_archive::{
    formats::{Keyset, TitleKeys, cnmt::Cnmt, pfs0::Pfs0, xci::Xci},
    util::TitleDataExt,
};
use once_cell::sync::Lazy;
use std::fs::File;
use std::path::Path;
use std::str;

// Lazy-loaded static keyset and title keys
static KEYSET: Lazy<Result<Keyset, color_eyre::eyre::Error>> = Lazy::new(|| {
    let config = crate::config::config();
    Keyset::from_file(&config.backend_config.prod_keys).map_err(|e| e.into())
});

static TITLE_KEYS: Lazy<Result<TitleKeys, color_eyre::eyre::Error>> = Lazy::new(|| {
    let config = crate::config::config();
    TitleKeys::load_from_file(&config.backend_config.title_keys).map_err(|e| e.into())
});

const NSP_EXTENSIONS: &[&str] = &["nsp", "nsz"];
const XCI_EXTENSIONS: &[&str] = &["xci", "xcz"];

pub fn read_cnmts(path: &str) -> color_eyre::Result<Vec<Cnmt>> {
    let keyset = KEYSET
        .as_ref()
        .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;
    let title_keyset = TITLE_KEYS
        .as_ref()
        .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;
    let path = Path::new(path);
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .ok_or_else(|| color_eyre::eyre::eyre!("Invalid file extension"))?;

    let file = File::open(path)?;
    let shared_reader = nx_archive::io::SharedReader::new(&file);

    let cnmt = if NSP_EXTENSIONS.contains(&extension.as_str()) {
        let mut nsp = Pfs0::from_reader(shared_reader)?;
        nsp.get_cnmts(keyset, Some(title_keyset))?
    } else if XCI_EXTENSIONS.contains(&extension.as_str()) {
        let mut xci = Xci::new(shared_reader)?;
        xci.get_cnmts(keyset, Some(title_keyset))?
    } else {
        return Err(color_eyre::eyre::eyre!("Unsupported file extension"));
    };

    println!("{:?}", cnmt.len());
    Ok(cnmt)
}

pub fn read_cnmt_merged(path: &str) -> color_eyre::Result<Cnmt> {
    tracing::info!("Reading CNMT using nx-archive from {}", path);
    let cnmts_list = read_cnmts(path)?;

    // We're gonna be merging the cnmts into one, getting the base cnmt and the update cnmt with the latest version

    if cnmts_list.len() == 1 {
        return Ok(cnmts_list[0].clone());
    }

    let cnmt_base = cnmts_list
        .iter()
        .find(|cnmt| cnmt.get_title_id_string().ends_with("000"));

    let cnmt_latest = cnmts_list
        .iter()
        .max_by_key(|cnmt| cnmt.header.title_version);

    match (cnmt_base, cnmt_latest) {
        (Some(base), Some(latest)) => {
            // get the base cnmt, but modify the version to be the latest version
            let mut base = base.clone();
            base.header.title_version = latest.header.title_version;
            Ok(base)
        }
        (Some(base), None) => Ok(base.clone()),
        (None, Some(latest)) => Ok(latest.clone()),
        (None, None) => Err(color_eyre::eyre::eyre!("No valid CNMT found")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_cnmt() {
        read_cnmts(
            "/media/nas/media/games/ROMs/switch/Ace Attorney Investigations Collection/DLC - Ace Attorney Investigations Collection[010005501E68D001][v0][US].nsp",
        ).unwrap();
    }
}
