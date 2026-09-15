use std::{fs::Permissions, os::unix::fs::PermissionsExt, path::Path};

use crate::error::Error;

pub fn set_owner_only_permissions(path: &Path) -> Result<(), Error> {
    std::fs::set_permissions(path, Permissions::from_mode(0o600)).map_err(Error::Io)
}
