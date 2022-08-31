use anyhow::Result;
use r2d2_sqlite::SqliteConnectionManager;
use ustr::Ustr;

use super::{ReviewList, ReviewListDB};

fn new_test_review_list() -> Result<Box<dyn ReviewList>> {
    let connection_manager = SqliteConnectionManager::memory();
    let review_list = ReviewListDB::new(connection_manager)?;
    Ok(Box::new(review_list))
}

#[test]
fn add_and_remove_from_review_list() -> Result<()> {
    let mut review_list = new_test_review_list()?;

    let unit_id = Ustr::from("unit_id");
    let unit_id2 = Ustr::from("unit_id2");
    review_list.add_to_review_list(&unit_id)?;
    review_list.add_to_review_list(&unit_id)?;
    review_list.add_to_review_list(&unit_id2)?;

    let entries = review_list.all_review_list_entries()?;
    assert_eq!(entries.len(), 2);
    assert!(entries.contains(&unit_id));
    assert!(entries.contains(&unit_id2));

    review_list.remove_from_review_list(&unit_id)?;
    let entries = review_list.all_review_list_entries()?;
    assert_eq!(entries.len(), 1);
    assert!(!entries.contains(&unit_id));
    assert!(entries.contains(&unit_id2));
    Ok(())
}
