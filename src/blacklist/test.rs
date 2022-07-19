use anyhow::Result;
use r2d2_sqlite::SqliteConnectionManager;

use super::{BlackListDB, Blacklist};

fn new_test_blacklist() -> Result<Box<dyn Blacklist>> {
    let connection_manager = SqliteConnectionManager::memory();
    let blacklist = BlackListDB::new(connection_manager)?;
    Ok(Box::new(blacklist))
}

#[test]
fn not_in_blacklist() -> Result<()> {
    let blacklist = new_test_blacklist()?;
    assert!(!blacklist.blacklisted("unit_id")?);
    Ok(())
}

#[test]
fn add_and_remove_from_blacklist() -> Result<()> {
    let mut blacklist = new_test_blacklist()?;

    blacklist.add_unit("unit_id")?;
    assert!(blacklist.blacklisted("unit_id")?);
    blacklist.remove_unit("unit_id")?;
    assert!(!blacklist.blacklisted("unit_id")?);
    Ok(())
}

#[test]
fn readd_to_blacklist() -> Result<()> {
    let mut blacklist = new_test_blacklist()?;
    blacklist.add_unit("unit_id")?;
    assert!(blacklist.blacklisted("unit_id")?);
    blacklist.remove_unit("unit_id")?;
    assert!(!blacklist.blacklisted("unit_id")?);
    blacklist.add_unit("unit_id")?;
    assert!(blacklist.blacklisted("unit_id")?);
    Ok(())
}

#[test]
fn all_entries() -> Result<()> {
    let mut blacklist = new_test_blacklist()?;
    blacklist.add_unit("unit_id")?;
    assert!(blacklist.blacklisted("unit_id")?);
    blacklist.add_unit("unit_id2")?;
    assert!(blacklist.blacklisted("unit_id2")?);
    assert_eq!(
        blacklist.all_entries()?,
        vec!["unit_id".to_string(), "unit_id2".to_string()]
    );
    Ok(())
}
