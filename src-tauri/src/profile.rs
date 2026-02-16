use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileFileInfo {
    pub filename: String,
    pub content: String,
    pub modified_at: String,
    pub size_bytes: u64,
}

pub fn get_profile_dir(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("profile")
}

pub fn read_all_profiles(app_data_dir: &PathBuf) -> Result<HashMap<String, String>, String> {
    let dir = get_profile_dir(app_data_dir);
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        return Ok(HashMap::new());
    }
    let mut files = HashMap::new();
    let entries = fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            files.insert(filename, content);
        }
    }
    Ok(files)
}

pub fn write_profile_file(app_data_dir: &PathBuf, filename: &str, content: &str) -> Result<String, String> {
    let dir = get_profile_dir(app_data_dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(filename);
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(format!("Successfully wrote {}", filename))
}

pub fn delete_profile_file(app_data_dir: &PathBuf, filename: &str) -> Result<String, String> {
    let dir = get_profile_dir(app_data_dir);
    let path = dir.join(filename);
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
        Ok(format!("Successfully deleted {}", filename))
    } else {
        Ok(format!("File {} does not exist", filename))
    }
}

pub fn read_all_profiles_detailed(app_data_dir: &PathBuf) -> Result<Vec<ProfileFileInfo>, String> {
    let dir = get_profile_dir(app_data_dir);
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let metadata = fs::metadata(&path).map_err(|e| e.to_string())?;
            let modified = metadata.modified().map_err(|e| e.to_string())?;
            let modified_at = chrono::DateTime::<chrono::Utc>::from(modified)
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string();
            let size_bytes = metadata.len();
            files.push(ProfileFileInfo {
                filename,
                content,
                modified_at,
                size_bytes,
            });
        }
    }
    files.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn integration_writes_reads_and_sorts_profile_files() {
        let dir = tempdir().expect("temp directory should exist");
        let app_data_dir = dir.path().to_path_buf();

        write_profile_file(&app_data_dir, "career.md", "# Career\n- Engineer")
            .expect("career profile should save");
        write_profile_file(&app_data_dir, "values.md", "# Values\n- Family")
            .expect("values profile should save");

        let profiles = read_all_profiles(&app_data_dir).expect("profiles should load");
        assert_eq!(
            profiles.get("career.md").map(String::as_str),
            Some("# Career\n- Engineer")
        );
        assert_eq!(
            profiles.get("values.md").map(String::as_str),
            Some("# Values\n- Family")
        );

        let detailed = read_all_profiles_detailed(&app_data_dir).expect("detailed profiles should load");
        assert_eq!(detailed.len(), 2);
        assert_eq!(detailed[0].filename, "career.md");
        assert_eq!(detailed[1].filename, "values.md");
        assert!(detailed[0].size_bytes > 0);
    }

    #[test]
    fn unit_delete_profile_file_is_idempotent() {
        let dir = tempdir().expect("temp directory should exist");
        let app_data_dir = dir.path().to_path_buf();

        let first = delete_profile_file(&app_data_dir, "missing.md")
            .expect("deleting missing file should not fail");
        assert_eq!(first, "File missing.md does not exist");

        write_profile_file(&app_data_dir, "notes.md", "content").expect("file should save");
        let deleted = delete_profile_file(&app_data_dir, "notes.md").expect("file should delete");
        assert_eq!(deleted, "Successfully deleted notes.md");
    }
}
