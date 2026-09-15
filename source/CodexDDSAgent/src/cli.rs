use clap::{Parser, Subcommand};
use serde_json::json;

use crate::{
    error::Error,
    registry::{PortMode, RegistryStore, ServiceConfig, StoragePaths},
    service::ServiceRuntime,
};

#[derive(Debug, Parser)]
#[command(
    name = "CodexDDSAgent",
    version,
    about = "Local manager for CodexDDSAgent"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(subcommand)]
    Service(ServiceCommand),
    #[command(subcommand)]
    Agent(AgentCommand),
    #[command(subcommand)]
    Config(ConfigCommand),
}

#[derive(Debug, Subcommand)]
enum ServiceCommand {
    Create {
        #[arg(long)]
        service_name: String,
        #[arg(long)]
        agent_name: String,
        #[arg(long = "listen-host", default_value = "0.0.0.0")]
        listen_host: String,
        #[arg(long = "port-mode", default_value = "auto")]
        port_mode: String,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long = "port-range", value_parser = parse_port_range)]
        port_range: Option<(u16, u16)>,
        #[arg(long = "disable-discovery")]
        disable_discovery: bool,
    },
    Start {
        #[arg(long)]
        service_name: String,
    },
    List,
    Shutdown {
        #[arg(long)]
        service_name: String,
    },
}

#[derive(Debug, Subcommand)]
enum AgentCommand {
    List {
        #[arg(long)]
        service_name: String,
    },
    Delete {
        #[arg(long)]
        service_name: String,
        #[arg(long)]
        agent_name: String,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    Get {
        #[arg(long)]
        service_name: String,
    },
    Set {
        #[arg(long)]
        service_name: String,
        values: Vec<String>,
    },
}

pub fn run(args: Cli) -> Result<(), Error> {
    match args.command {
        Command::Service(ServiceCommand::Create {
            service_name,
            agent_name,
            listen_host,
            port_mode,
            port,
            port_range,
            disable_discovery,
        }) => {
            let paths = StoragePaths::from_environment()?;
            let store = RegistryStore::open(paths)?;
            let config = ServiceConfig {
                listen_host,
                port_mode: parse_port_mode(&port_mode)?,
                port,
                port_range,
                discovery_enabled: !disable_discovery,
            };
            validate_service_config(&config)?;
            let (service, agent) = store.create_service(&service_name, &agent_name, &config)?;
            println!(
                "{}",
                json!({
                    "service": service,
                    "initial_agent": agent,
                    "service_started": true
                })
            );
            return run_service(store, &service_name);
        }
        Command::Service(ServiceCommand::Start { service_name }) => {
            let store = RegistryStore::open(StoragePaths::from_environment()?)?;
            if store.get_service(&service_name)?.is_none() {
                return Err(Error::Rpc(crate::error::RpcError::new(
                    "service_not_found",
                    "service does not exist",
                )));
            }
            run_service(store, &service_name)?;
        }
        Command::Service(ServiceCommand::List) => {
            let store = RegistryStore::open(StoragePaths::from_environment()?)?;
            println!("{}", serde_json::to_string_pretty(&store.list_services()?)?);
        }
        Command::Service(ServiceCommand::Shutdown { service_name }) => {
            let store = RegistryStore::open(StoragePaths::from_environment()?)?;
            let service = store.get_service(&service_name)?.ok_or_else(|| {
                Error::Rpc(crate::error::RpcError::new(
                    "service_not_found",
                    "service does not exist",
                ))
            })?;
            let mut stopped = matches!(
                service.state,
                crate::registry::ServiceRegistryState::Stopped
            );
            if !stopped {
                let paths = StoragePaths::from_environment()?;
                let shutdown = paths.service_shutdown_file(&service_name);
                write_atomic(&shutdown, b"")?;
                stopped = wait_for_service_stop(&store, &service_name);
                if !stopped {
                    store.update_service_runtime(
                        &service_name,
                        crate::registry::ServiceRegistryState::Stopped,
                        None,
                    )?;
                    let _ = std::fs::remove_file(&shutdown);
                }
            }
            println!(
                "{}",
                json!({"service_name": service_name, "state": "stopped"})
            );
        }
        Command::Agent(AgentCommand::List { service_name }) => {
            let store = RegistryStore::open(StoragePaths::from_environment()?)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&store.list_agents(&service_name)?)?
            );
        }
        Command::Agent(AgentCommand::Delete {
            service_name,
            agent_name,
            force,
        }) => {
            let store = RegistryStore::open(StoragePaths::from_environment()?)?;
            let agent = store
                .get_agent(&service_name, &agent_name)?
                .ok_or_else(|| {
                    Error::Rpc(crate::error::RpcError::new(
                        "agent_not_found",
                        "agent does not exist",
                    ))
                })?;
            if agent.state == crate::registry::AgentRegistryState::Running && !force {
                return Err(Error::Rpc(crate::error::RpcError::new(
                    "agent_busy",
                    "agent is active; use --force to stop and delete",
                )));
            }
            let service = store.get_service(&service_name)?.ok_or_else(|| {
                Error::Rpc(crate::error::RpcError::new(
                    "service_not_found",
                    "service does not exist",
                ))
            })?;
            let mut deleted = false;
            if force && service.state == crate::registry::ServiceRegistryState::Running {
                let paths = StoragePaths::from_environment()?;
                let commands = paths.service_agent_commands_dir(&service_name);
                let command = commands.join(format!("delete-{agent_name}.json"));
                write_atomic(
                    &command,
                    json!({"action": "delete", "agent_name": agent_name}).to_string(),
                )?;
                deleted = wait_for_agent_delete(&store, &service_name, &agent_name);
                if !deleted {
                    let _ = std::fs::remove_file(&command);
                }
            }
            if !deleted {
                store.delete_agent(&service_name, &agent_name)?;
            }
            println!(
                "{}",
                json!({"service_name": service_name, "agent_name": agent_name, "deleted": true})
            );
        }
        Command::Config(ConfigCommand::Get { service_name }) => {
            let store = RegistryStore::open(StoragePaths::from_environment()?)?;
            let service = store.get_service(&service_name)?.ok_or_else(|| {
                Error::Rpc(crate::error::RpcError::new(
                    "service_not_found",
                    "service does not exist",
                ))
            })?;
            println!(
                "{}",
                serde_json::to_string_pretty(&crate::registry::service_config(&service)?)?
            );
        }
        Command::Config(ConfigCommand::Set {
            service_name,
            values,
        }) => {
            let store = RegistryStore::open(StoragePaths::from_environment()?)?;
            let service = store.get_service(&service_name)?.ok_or_else(|| {
                Error::Rpc(crate::error::RpcError::new(
                    "service_not_found",
                    "service does not exist",
                ))
            })?;
            let mut config = crate::registry::service_config(&service)?;
            for value in values {
                apply_config_value(&mut config, &value)?;
            }
            validate_service_config(&config)?;
            store.set_service_config(&service_name, &config)?;
            println!("{}", serde_json::to_string_pretty(&config)?);
        }
    }
    Ok(())
}

fn run_service(store: RegistryStore, service_name: &str) -> Result<(), Error> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(Error::Io)?;
    runtime.block_on(async {
        let service = ServiceRuntime::start(
            std::sync::Arc::new(std::sync::Mutex::new(store)),
            service_name,
        )
        .await?;
        service.run_until_shutdown().await
    })
}

fn wait_for_service_stop(store: &RegistryStore, service_name: &str) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
        if let Ok(Some(service)) = store.get_service(service_name) {
            if service.state == crate::registry::ServiceRegistryState::Stopped {
                return true;
            }
        }
    }
    false
}

fn wait_for_agent_delete(store: &RegistryStore, service_name: &str, agent_name: &str) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
        if store
            .get_agent(service_name, agent_name)
            .map(|agent| agent.is_none())
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

fn write_atomic(path: &std::path::Path, contents: impl AsRef<[u8]>) -> Result<(), Error> {
    let temporary = path.with_extension("tmp");
    let parent = path
        .parent()
        .ok_or_else(|| Error::internal("control file must have a parent directory"))?;
    std::fs::create_dir_all(parent)?;
    std::fs::write(&temporary, contents)?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}

fn parse_port_range(value: &str) -> Result<(u16, u16), String> {
    let (start, end) = value.split_once('-').ok_or("expected START-END")?;
    let start = start.parse().map_err(|_| "invalid START")?;
    let end = end.parse().map_err(|_| "invalid END")?;
    if start <= end && start != 0 {
        Ok((start, end))
    } else {
        Err("port range must be ascending and not contain 0".to_string())
    }
}

fn parse_port_mode(value: &str) -> Result<PortMode, Error> {
    match value {
        "auto" => Ok(PortMode::Auto),
        "fixed" => Ok(PortMode::Fixed),
        _ => Err(Error::internal("port_mode must be auto or fixed")),
    }
}

fn apply_config_value(config: &mut ServiceConfig, item: &str) -> Result<(), Error> {
    let (key, value) = item.split_once('=').ok_or_else(|| {
        Error::Rpc(crate::error::RpcError::new(
            "invalid_request",
            "config item must be key=value",
        ))
    })?;
    match key {
        "listen_host" => config.listen_host = value.to_string(),
        "port_mode" => config.port_mode = parse_port_mode(value)?,
        "port" => {
            config.port = if value.is_empty() {
                None
            } else {
                Some(value.parse().map_err(|_| Error::internal("invalid port"))?)
            }
        }
        "port_range" => {
            config.port_range = if value.is_empty() {
                None
            } else {
                Some(parse_port_range(value).map_err(Error::internal)?)
            }
        }
        "discovery_enabled" => {
            config.discovery_enabled = value
                .parse()
                .map_err(|_| Error::internal("invalid boolean"))?
        }
        _ => return Err(Error::internal(format!("unknown config key: {key}"))),
    }
    Ok(())
}

fn validate_service_config(config: &ServiceConfig) -> Result<(), Error> {
    if config.listen_host.parse::<std::net::IpAddr>().is_err() && config.listen_host != "0.0.0.0" {
        return Err(Error::Rpc(crate::error::RpcError::new(
            "server_address_invalid",
            "listen_host must be an IP address",
        )));
    }
    if config.port_mode == PortMode::Fixed && config.port.is_none() {
        return Err(Error::Rpc(crate::error::RpcError::new(
            "server_address_invalid",
            "fixed port mode requires port",
        )));
    }
    if let Some((start, end)) = config.port_range {
        if start == 0 || end < start {
            return Err(Error::Rpc(crate::error::RpcError::new(
                "server_address_invalid",
                "invalid port range",
            )));
        }
    }
    Ok(())
}
