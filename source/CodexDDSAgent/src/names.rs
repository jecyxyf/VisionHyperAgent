use crate::error::Error;

pub fn validate_name(value: &str) -> Result<(), Error> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
    if valid {
        Ok(())
    } else {
        Err(Error::invalid_name(
            "name must contain only ASCII letters, digits, '_' or '-', and be 1..=64 characters",
        ))
    }
}
