use reqwest::Client;
use serde::Deserialize;
use fuser::FileType;



#[derive(Deserialize)]
pub struct RemoteEntry {
    pub name: String,
    pub kind: FileType, 
    pub ino: u64,
}

// pub async fn list_dir(base: &str, path: &str) -> Result<Vec<RemoteEntry>, reqwest::Error> {
//     let url = format!("{}/list?path={}", base, path);
//     let resp = Client::new().get(&url).send().await?.json::<Vec<RemoteEntry>>().await?;
//     Ok(resp)
// }

// pub async fn read_file(base: &str, path: &str) -> Result<Vec<u8>, reqwest::Error> {
//     let url = format!("{}/files?path={}", base, path);
//     let resp = Client::new().get(&url).send().await?.bytes().await?;
//     Ok(resp.to_vec())
// }
