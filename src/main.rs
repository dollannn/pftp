use config::ServerInfo;
use tracing::{error, info};

mod config;
mod sftp;

#[tokio::main]
async fn main() {
    // Load env
    dotenvy::dotenv().ok();
    // Setup tracing
    let format = tracing_subscriber::fmt::format()
        .with_target(false)
        .with_level(true)
        .with_thread_names(false);

    tracing_subscriber::fmt()
        .event_format(format)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Load the config
    let config = config::Config::new();

    info!("PFTP {}", env!("CARGO_PKG_VERSION"));

    // Get cmd args
    let server_group = match std::env::args()
        .collect::<Vec<String>>()
        .into_iter()
        .skip(1)
        .next()
    {
        Some(group) => group,
        None => {
            error!("No server group specified.");
            return;
        }
    };
    // Get servers in group
    let servers: &Vec<i32> = match config.server_groups.get(&server_group) {
        Some(servers) => {
            if servers.len() == 0 {
                error!("Server group is empty.");
                return;
            }
            servers
        }
        None => {
            error!("Server group not found.");
            return;
        }
    };
    // Get server info
    let mut server_infos: Vec<ServerInfo> = Vec::new();
    for server_id in servers {
        let server = match config.servers.get(&server_id.to_string()) {
            Some(server) => server.clone(),
            None => {
                error!("Server not found. ID: {}", server_id);
                return;
            }
        };
        server_infos.push(server.clone());
    }

    // Ensure host is defined
    for server_info in &mut server_infos {
        // Get the host
        let host = server_info.host.clone();
        config.hosts.get(&host).unwrap_or_else(|| {
            error!("Host not found. Host: {} ID: {}", host, server_info.name);
            std::process::exit(1);
        });
        // Replace the host with the value from the config
        server_info.host = config.hosts.get(&host).unwrap().clone();
    }

    let mut task_queue = Vec::new();
    for server_info in server_infos {
        let ssh = sftp::Ssh::new(
            server_info.host.clone(),
            server_info.username.clone(),
            std::env::var("PFTP_PASSWORD").unwrap(),
        );
        let input_dir = config.input.input_dir.clone();
        let default_output_dir = config.input.default_output_dir.clone();
        let task = tokio::spawn(async move {
            let sftp = ssh.connect().await;
            match sftp {
                Some(sftp) => {
                    info!("Connected to server: {}", server_info.name);
                    sftp::Ssh::transfer(sftp, &input_dir, &default_output_dir).await;
                }
                None => {
                    error!(
                        "Failed to connect to server. Host: {} ID: {}",
                        server_info.host, server_info.name
                    );
                }
            }
        });
        task_queue.push(task);
    }
    // Wait for all tasks to finish
    for task in task_queue {
        match task.await {
            Ok(_) => {}
            Err(e) => {
                error!("Task error: {:?}", e);
            }
        }
    }
}
