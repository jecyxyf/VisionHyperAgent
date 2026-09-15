use std::path::PathBuf;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::{Error, RpcError},
    names::validate_name,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceConfig {
    pub listen_host: String,
    pub port_mode: PortMode,
    pub port: Option<u16>,
    pub port_range: Option<(u16, u16)>,
    pub discovery_enabled: bool,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            listen_host: "0.0.0.0".to_string(),
            port_mode: PortMode::Auto,
            port: None,
            port_range: Some((17600, 17699)),
            discovery_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PortMode {
    Auto,
    Fixed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceRecord {
    pub service_name: String,
    pub created_at: String,
    pub config_json: String,
    pub last_started_at: Option<String>,
    pub last_listen_port: Option<u16>,
    pub conflict_id: String,
    pub state: ServiceRegistryState,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceRegistryState {
    Stopped,
    Running,
    Error,
    NameConflict,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentRecord {
    pub service_name: String,
    pub agent_name: String,
    pub created_at: String,
    pub last_attached_at: Option<String>,
    pub last_stopped_at: Option<String>,
    pub state: AgentRegistryState,
    pub model_config_initialized: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentRegistryState {
    Registered,
    Running,
    Stopped,
    Error,
}

#[derive(Debug, Clone)]
pub struct StoragePaths {
    pub root: PathBuf,
    pub registry_db: PathBuf,
    pub codex_root: PathBuf,
    pub agent_binary: PathBuf,
    pub logs: PathBuf,
}

impl StoragePaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            registry_db: root.join("data/registry.db"),
            codex_root: root.join("data/codex"),
            agent_binary: root.join("agents/codex-app-server"),
            logs: root.join("agents/logs"),
            root,
        }
    }

    pub fn from_environment() -> Result<Self, Error> {
        if let Ok(root) = std::env::var("CODEX_DDS_AGENT_HOME") {
            if !root.trim().is_empty() {
                return Ok(Self::new(root));
            }
        }
        let exe = std::env::current_exe()
            .map_err(|err| Error::internal(format!("failed to resolve executable: {err}")))?;
        let root = exe
            .parent()
            .ok_or_else(|| Error::internal("executable has no parent directory"))?
            .to_path_buf();
        Ok(Self::new(root))
    }

    pub fn prepare(&self) -> Result<(), Error> {
        std::fs::create_dir_all(self.registry_db.parent().unwrap())?;
        std::fs::create_dir_all(&self.codex_root)?;
        std::fs::create_dir_all(self.agent_binary.parent().unwrap())?;
        std::fs::create_dir_all(&self.logs)?;
        Ok(())
    }

    pub fn codex_home(&self, agent_name: &str) -> PathBuf {
        self.codex_root.join(agent_name)
    }

    pub fn agent_log(&self, agent_name: &str) -> PathBuf {
        self.logs.join(agent_name)
    }
}

#[derive(Debug)]
pub struct RegistryStore {
    connection: Connection,
    paths: StoragePaths,
}

impl RegistryStore {
    pub fn open(paths: StoragePaths) -> Result<Self, Error> {
        paths.prepare()?;
        let connection = Connection::open(&paths.registry_db)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        Self::initialize(&connection)?;
        Ok(Self { connection, paths })
    }

    pub fn paths(&self) -> &StoragePaths {
        &self.paths
    }

    fn initialize(connection: &Connection) -> Result<(), Error> {
        connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS services (
                service_name TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                config_json TEXT NOT NULL,
                last_started_at TEXT,
                last_listen_port INTEGER,
                conflict_id TEXT NOT NULL,
                state TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS agents (
                service_name TEXT NOT NULL,
                agent_name TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_attached_at TEXT,
                last_stopped_at TEXT,
                state TEXT NOT NULL,
                model_config_initialized INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (service_name, agent_name),
                FOREIGN KEY (service_name) REFERENCES services(service_name) ON DELETE CASCADE
            );
            "#,
        )?;
        Ok(())
    }

    pub fn create_service(
        &self,
        service_name: &str,
        agent_name: &str,
        config: &ServiceConfig,
    ) -> Result<(ServiceRecord, AgentRecord), Error> {
        validate_name(service_name)?;
        validate_name(agent_name)?;
        if self.get_service(service_name)?.is_some() {
            return Err(Error::Rpc(RpcError::new(
                "agent_exists",
                "service already exists",
            )));
        }
        let now = Utc::now().to_rfc3339();
        let config_json = serde_json::to_string(config)?;
        let service = ServiceRecord {
            service_name: service_name.to_string(),
            created_at: now.clone(),
            config_json,
            last_started_at: None,
            last_listen_port: None,
            conflict_id: Uuid::new_v4().to_string(),
            state: ServiceRegistryState::Stopped,
        };
        let agent = AgentRecord {
            service_name: service_name.to_string(),
            agent_name: agent_name.to_string(),
            created_at: now,
            last_attached_at: None,
            last_stopped_at: None,
            state: AgentRegistryState::Registered,
            model_config_initialized: false,
        };

        let tx = self.connection.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO services VALUES (?, ?, ?, NULL, NULL, ?, ?)",
            params![
                service.service_name,
                service.created_at,
                service.config_json,
                service.conflict_id,
                state_name(&service.state)
            ],
        )?;
        insert_agent(&tx, &agent)?;
        tx.commit()?;
        Ok((service, agent))
    }

    pub fn get_service(&self, service_name: &str) -> Result<Option<ServiceRecord>, Error> {
        self.connection
            .query_row(
                "SELECT * FROM services WHERE service_name = ?",
                [service_name],
                service_row,
            )
            .optional()
            .map_err(Error::Database)
    }

    pub fn list_services(&self) -> Result<Vec<ServiceRecord>, Error> {
        let mut statement = self
            .connection
            .prepare("SELECT * FROM services ORDER BY created_at, service_name")?;
        let rows = statement
            .query_map([], service_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn set_service_config(
        &self,
        service_name: &str,
        config: &ServiceConfig,
    ) -> Result<(), Error> {
        self.ensure_service(service_name)?;
        let config_json = serde_json::to_string(config)?;
        self.connection.execute(
            "UPDATE services SET config_json = ? WHERE service_name = ?",
            params![config_json, service_name],
        )?;
        Ok(())
    }

    pub fn update_service_runtime(
        &self,
        service_name: &str,
        state: ServiceRegistryState,
        port: Option<u16>,
    ) -> Result<(), Error> {
        self.ensure_service(service_name)?;
        self.connection.execute(
            "UPDATE services SET state = ?, last_started_at = ?, last_listen_port = ? WHERE service_name = ?",
            params![
                state_name(&state),
                Utc::now().to_rfc3339(),
                port,
                service_name
            ],
        )?;
        Ok(())
    }

    pub fn create_agent(&self, service_name: &str, agent_name: &str) -> Result<AgentRecord, Error> {
        self.ensure_service(service_name)?;
        validate_name(agent_name)?;
        if self.get_agent(service_name, agent_name)?.is_some() {
            return Err(Error::Rpc(RpcError::new(
                "agent_exists",
                "agent already exists",
            )));
        }
        let agent = AgentRecord {
            service_name: service_name.to_string(),
            agent_name: agent_name.to_string(),
            created_at: Utc::now().to_rfc3339(),
            last_attached_at: None,
            last_stopped_at: None,
            state: AgentRegistryState::Registered,
            model_config_initialized: false,
        };
        insert_agent(&self.connection, &agent)?;
        Ok(agent)
    }

    pub fn get_agent(
        &self,
        service_name: &str,
        agent_name: &str,
    ) -> Result<Option<AgentRecord>, Error> {
        self.connection
            .query_row(
                "SELECT * FROM agents WHERE service_name = ? AND agent_name = ?",
                params![service_name, agent_name],
                agent_row,
            )
            .optional()
            .map_err(Error::Database)
    }

    pub fn list_agents(&self, service_name: &str) -> Result<Vec<AgentRecord>, Error> {
        self.ensure_service(service_name)?;
        let mut statement = self.connection.prepare(
            "SELECT * FROM agents WHERE service_name = ? ORDER BY created_at, agent_name",
        )?;
        let rows = statement
            .query_map([service_name], agent_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn mark_agent_running(&self, service_name: &str, agent_name: &str) -> Result<(), Error> {
        self.ensure_agent(service_name, agent_name)?;
        self.connection.execute(
            "UPDATE agents SET state = ?, last_attached_at = ?, last_stopped_at = NULL WHERE service_name = ? AND agent_name = ?",
            params![
                agent_state_name(&AgentRegistryState::Running),
                Utc::now().to_rfc3339(),
                service_name,
                agent_name
            ],
        )?;
        Ok(())
    }

    pub fn mark_agent_model_initialized(
        &self,
        service_name: &str,
        agent_name: &str,
    ) -> Result<(), Error> {
        self.ensure_agent(service_name, agent_name)?;
        self.connection.execute(
            "UPDATE agents SET model_config_initialized = 1 WHERE service_name = ? AND agent_name = ?",
            params![service_name, agent_name],
        )?;
        Ok(())
    }

    pub fn mark_agent_stopped(
        &self,
        service_name: &str,
        agent_name: &str,
        state: AgentRegistryState,
    ) -> Result<(), Error> {
        self.ensure_agent(service_name, agent_name)?;
        self.connection.execute(
            "UPDATE agents SET state = ?, last_stopped_at = ? WHERE service_name = ? AND agent_name = ?",
            params![
                agent_state_name(&state),
                Utc::now().to_rfc3339(),
                service_name,
                agent_name
            ],
        )?;
        Ok(())
    }

    pub fn delete_agent(&self, service_name: &str, agent_name: &str) -> Result<(), Error> {
        self.ensure_agent(service_name, agent_name)?;
        self.connection.execute(
            "DELETE FROM agents WHERE service_name = ? AND agent_name = ?",
            params![service_name, agent_name],
        )?;
        remove_dir_if_exists(&self.paths.codex_home(agent_name))?;
        remove_dir_if_exists(&self.paths.agent_log(agent_name))?;
        Ok(())
    }

    fn ensure_service(&self, service_name: &str) -> Result<ServiceRecord, Error> {
        self.get_service(service_name)?
            .ok_or_else(|| Error::Rpc(RpcError::new("service_not_found", "service does not exist")))
    }

    fn ensure_agent(&self, service_name: &str, agent_name: &str) -> Result<AgentRecord, Error> {
        self.get_agent(service_name, agent_name)?
            .ok_or_else(|| Error::Rpc(RpcError::new("agent_not_found", "agent does not exist")))
    }
}

fn remove_dir_if_exists(path: &std::path::Path) -> Result<(), Error> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(Error::Io(err)),
    }
}

fn insert_agent(connection: &Connection, agent: &AgentRecord) -> Result<(), Error> {
    connection.execute(
        "INSERT INTO agents VALUES (?, ?, ?, NULL, NULL, ?, ?)",
        params![
            agent.service_name,
            agent.agent_name,
            agent.created_at,
            agent_state_name(&agent.state),
            agent.model_config_initialized as i32
        ],
    )?;
    Ok(())
}

fn service_row(row: &Row<'_>) -> rusqlite::Result<ServiceRecord> {
    Ok(ServiceRecord {
        service_name: row.get(0)?,
        created_at: row.get(1)?,
        config_json: row.get(2)?,
        last_started_at: row.get(3)?,
        last_listen_port: row.get(4)?,
        conflict_id: row.get(5)?,
        state: match row.get::<_, String>(6)?.as_str() {
            "running" => ServiceRegistryState::Running,
            "error" => ServiceRegistryState::Error,
            "name_conflict" => ServiceRegistryState::NameConflict,
            _ => ServiceRegistryState::Stopped,
        },
    })
}

fn agent_row(row: &Row<'_>) -> rusqlite::Result<AgentRecord> {
    Ok(AgentRecord {
        service_name: row.get(0)?,
        agent_name: row.get(1)?,
        created_at: row.get(2)?,
        last_attached_at: row.get(3)?,
        last_stopped_at: row.get(4)?,
        state: match row.get::<_, String>(5)?.as_str() {
            "running" => AgentRegistryState::Running,
            "stopped" => AgentRegistryState::Stopped,
            "error" => AgentRegistryState::Error,
            _ => AgentRegistryState::Registered,
        },
        model_config_initialized: row.get::<_, i32>(6)? != 0,
    })
}

fn state_name(state: &ServiceRegistryState) -> &'static str {
    match state {
        ServiceRegistryState::Stopped => "stopped",
        ServiceRegistryState::Running => "running",
        ServiceRegistryState::Error => "error",
        ServiceRegistryState::NameConflict => "name_conflict",
    }
}

fn agent_state_name(state: &AgentRegistryState) -> &'static str {
    match state {
        AgentRegistryState::Registered => "registered",
        AgentRegistryState::Running => "running",
        AgentRegistryState::Stopped => "stopped",
        AgentRegistryState::Error => "error",
    }
}

pub fn service_config(record: &ServiceRecord) -> Result<ServiceConfig, Error> {
    Ok(serde_json::from_str(&record.config_json)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_lists_service_and_agent_records() {
        let root = tempfile::tempdir().unwrap();
        let store = RegistryStore::open(StoragePaths::new(root.path())).unwrap();
        store
            .create_service("main-studio", "desktop-a", &ServiceConfig::default())
            .unwrap();
        assert_eq!(store.list_services().unwrap().len(), 1);
        assert_eq!(store.list_agents("main-studio").unwrap().len(), 1);
    }

    #[test]
    fn duplicate_service_and_agent_names_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let store = RegistryStore::open(StoragePaths::new(root.path())).unwrap();
        store
            .create_service("main-studio", "desktop-a", &ServiceConfig::default())
            .unwrap();
        let service_error = store
            .create_service("main-studio", "other", &ServiceConfig::default())
            .unwrap_err();
        assert!(matches!(
            service_error,
            Error::Rpc(RpcError {
                code: "agent_exists",
                ..
            })
        ));
        let agent_error = store.create_agent("main-studio", "desktop-a").unwrap_err();
        assert!(matches!(
            agent_error,
            Error::Rpc(RpcError {
                code: "agent_exists",
                ..
            })
        ));
    }

    #[test]
    fn deleting_agent_removes_registry_and_codex_home() {
        let root = tempfile::tempdir().unwrap();
        let store = RegistryStore::open(StoragePaths::new(root.path())).unwrap();
        store
            .create_service("main-studio", "desktop-a", &ServiceConfig::default())
            .unwrap();
        std::fs::create_dir_all(store.paths().codex_home("desktop-a")).unwrap();
        store.delete_agent("main-studio", "desktop-a").unwrap();
        assert!(store.list_agents("main-studio").unwrap().is_empty());
        assert!(!store.paths().codex_home("desktop-a").exists());
    }
}
