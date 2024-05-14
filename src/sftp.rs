use async_recursion::async_recursion;
use async_std::prelude::*;
use async_trait::async_trait;
use russh::*;
use russh_keys::*;
use russh_sftp::{client::SftpSession, protocol::OpenFlags};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, info, trace};

#[derive(Debug)]
pub struct Ssh {
    pub host: String,
    pub username: String,
    pub password: String,
}

impl Ssh {
    pub fn new(host: String, username: String, password: String) -> Self {
        let host = host.trim().trim_start_matches("sftp://").to_string();
        Ssh {
            host,
            username,
            password,
        }
    }
    pub async fn connect(&self) -> Option<SftpSession> {
        let config = russh::client::Config::default();
        let sh = Client {};
        let parts: Vec<&str> = self.host.split(':').collect();

        // Seperate the host and port
        let host = parts[0].to_string();
        let port = parts[1].parse::<u16>().unwrap();
        let mut session = russh::client::connect(Arc::new(config), (host, port), sh)
            .await
            .unwrap();
        if session
            .authenticate_password(self.username.clone(), self.password.clone())
            .await
            .unwrap()
        {
            let channel = session.channel_open_session().await.unwrap();
            channel.request_subsystem(true, "sftp").await.unwrap();
            let sftp = SftpSession::new(channel.into_stream()).await.unwrap();
            return Some(sftp);
        }
        None
    }
    // Copies a directory from the local input path to the output path
    pub async fn transfer(_sftp: SftpSession, input: &String, output: &String) {
        // Load all files in the input directory
        let vfiles = fast_search_directory(input.to_string()).await;
        if vfiles.len() == 0 {
            error!("No files found in input directory: {}", input);
        }
        for file in vfiles {
            let path = file.path();
            let trimmed_input = path
                .to_str()
                .unwrap()
                .trim_start_matches(input)
                .replace("\\", "/");
            let output_path = format!("{}{}", output, trimmed_input);

            trace!("Copying {:?} to {:?}", path, output_path);
            let mut remote_file = _sftp
                .open_with_flags(
                    output_path,
                    OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE | OpenFlags::READ,
                )
                .await
                .unwrap();
            // Read the file
            let mut local_file = tokio::fs::File::open(path).await.unwrap();
            let mut buffer = Vec::new();
            local_file.read_to_end(&mut buffer).await.unwrap();
            // Write the file
            remote_file.write_all(&buffer).await.unwrap();
            match remote_file.flush().await {
                Ok(_) => {}
                Err(e) => {
                    error!("Error copying file: {:?}", e);
                }
            }
        }
        info!("Finished copying files")
    }
}

// Fast directory traversal, returns a list of files
pub async fn fast_search_directory(path: String) -> Vec<async_std::fs::DirEntry> {
    #[async_recursion]
    async fn search_directory(path: String) -> Vec<async_std::fs::DirEntry> {
        let mut files = Vec::new();
        if let Ok(mut entries) = async_std::fs::read_dir(path).await {
            while let Some(res) = entries.next().await {
                match res {
                    Ok(entry) => {
                        let path = entry.path();
                        if path.is_dir().await {
                            files.extend(
                                search_directory(path.to_string_lossy().into_owned()).await,
                            );
                        } else {
                            files.push(entry);
                        }
                    }
                    Err(_) => continue,
                }
            }
        }
        files
    }

    search_directory(path).await
}

pub struct Client;

#[async_trait]
impl client::Handler for Client {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &key::PublicKey,
    ) -> Result<bool, Self::Error> {
        trace!("check_server_key: {:?}", server_public_key);
        Ok(true)
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        trace!("data on channel {:?}: {}", channel, data.len());
        Ok(())
    }
}
