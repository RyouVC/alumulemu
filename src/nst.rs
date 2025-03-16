use regex::Regex;

#[derive(Debug)]
struct ContentMeta {
    title_id: String,
    version: String,
    content_type: u8,
    attributes: u32,
    storage_id: u8,
    content_install_type: u8,
    required_download_system_version: String,
    digest: String,
}

// all of this is just placeholder stuff until we can get our own rust impl going
// make SURE you have prod.keys and title.keys in ~/.switch, you'll need them!
// message to john nintendo: we do not support piracy, our users must dump their own games and
// dump their keys from their own purchased console. trust

pub fn run_nstool(path: &str) -> String {
    // create a temp dir for the files
    let tmp_dir = std::env::temp_dir().join("tmp");
    std::fs::create_dir_all(&tmp_dir).expect("Failed to create temp directory");

    // list the initial nsp files
    let output = std::process::Command::new("tools/nstool")
        .arg("--fstree")
        .arg(path)
        .output()
        .expect("Failed to execute nstool");

    // find the .cnmt.nca
    let output_str = String::from_utf8_lossy(&output.stdout);
    let cnmt_line = output_str
        .lines()
        .find(|line| line.trim().ends_with(".cnmt.nca"))
        .map(|line| line.trim().to_string())
        .unwrap_or_default();
    println!("CNMT line: {:?}", cnmt_line);

    // dump the .cnmt.nca from the nsp
    let output = std::process::Command::new("tools/nstool")
        .arg("-x")
        .arg(cnmt_line)
        .arg(format!("{}/nca", tmp_dir.display()))
        .arg(path)
        .output()
        .expect("Failed to execute nstool");

    // list the nca
    let output = std::process::Command::new("tools/nstool")
        .arg("--fstree")
        .arg(format!("{}/nca", tmp_dir.display()))
        .output()
        .expect("Failed to execute nstool");

    // find the cnmt
    let output_str = String::from_utf8_lossy(&output.stdout);
    let cnmt_line = output_str
        .lines()
        .find(|line| line.trim().ends_with(".cnmt"))
        .map(|line| line.trim().to_string())
        .unwrap_or_default();
    println!("CNMT line 2: {:?}", cnmt_line);

    // dump the cnmt as app.cnmt
    let output = std::process::Command::new("tools/nstool")
        .arg("-x")
        .arg(format!("0/{}", cnmt_line))
        .arg(format!("{}/app.cnmt", tmp_dir.display()))
        .arg(format!("{}/nca", tmp_dir.display()))
        .output()
        .expect("Failed to execute nstool");

    println!("Output: {:?}", output);

    // parse the app.cnmt file
    let output = std::process::Command::new("tools/nstool")
        .arg(format!("{}/app.cnmt", tmp_dir.display()))
        .output()
        .expect("Failed to execute nstool");

    let output_str = String::from_utf8_lossy(&output.stdout).to_string();
    println!("CNMT output: {}", output_str);
    output_str
}

pub fn parse_cnmt_output(output: &str) -> ContentMeta {
    let title_id_re = Regex::new(r"TitleId:\s*0x([0-9a-fA-F]+)").unwrap();
    let version_re = Regex::new(r"Version:.*\(v(\d+)\)").unwrap();
    let type_re = Regex::new(r"Type:\s*\w+\s*\((\d+)\)").unwrap();
    let attributes_re = Regex::new(r"Attributes:\s*0x([0-9a-fA-F]+)").unwrap();
    let storage_id_re = Regex::new(r"StorageId:\s*\w+\s*\((\d+)\)").unwrap();
    let install_type_re = Regex::new(r"ContentInstallType:\s*\w+\s*\((\d+)\)").unwrap();
    let req_download_ver_re = Regex::new(r"RequiredDownloadSystemVersion:.*\(v(\d+)\)").unwrap();
    let digest_re = Regex::new(r"Digest:\s*([0-9a-fA-F]+)").unwrap();

    ContentMeta {
        title_id: title_id_re.captures(output).unwrap()[1].to_string(),
        version: format!("v{}", version_re.captures(output).unwrap()[1].to_string()),
        content_type: type_re.captures(output).unwrap()[1].parse().unwrap(),
        attributes: u32::from_str_radix(&attributes_re.captures(output).unwrap()[1], 16).unwrap(),
        storage_id: storage_id_re.captures(output).unwrap()[1].parse().unwrap(),
        content_install_type: install_type_re.captures(output).unwrap()[1]
            .parse()
            .unwrap(),
        required_download_system_version: format!(
            "v{}",
            req_download_ver_re.captures(output).unwrap()[1].to_string()
        ),
        digest: digest_re.captures(output).unwrap()[1].to_string(),
    }
}

pub fn get_title_id_and_version(cnmt: ContentMeta) -> (String, String) {
    (cnmt.title_id.to_uppercase(), cnmt.version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Read;

    #[test]
    fn test_parse_cnmt_output() {
        let nsp_path = "games/Balatro.nsp";
        let output = run_nstool(nsp_path);
        let result = parse_cnmt_output(&output);
        let (title_id, version) = get_title_id_and_version(result);
        println!("Title ID: {}", title_id);
        println!("Version: {}", version);
    }
    #[test]
    fn test_run_nstool() {
        let nsp_path = "games/Balatro.nsp";
        let cnmt_line = run_nstool(nsp_path);
        println!("{}", cnmt_line);
    }
}
