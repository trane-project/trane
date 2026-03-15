//! Contains common utilities used in multiple modules.

use rusqlite::Connection;

/// Returns the weighted average of the scores.
#[must_use]
pub fn weighted_average(values: &[f32], weights: &[f32]) -> f32 {
    // weighted average = (cross product of values and their weights) / (sum of weights)
    let cross_product: f32 = values.iter().zip(weights.iter()).map(|(s, w)| s * *w).sum();
    let weight_sum = weights.iter().sum::<f32>();
    if weight_sum == 0.0 {
        0.0
    } else {
        cross_product / weight_sum
    }
}

/// Returns a connection to the given database path with the correct pragmas set.
pub fn new_connection(db_path: &str) -> Result<Connection, rusqlite::Error> {
    let connection = Connection::open(db_path)?;
    // The following pragma statements are set to improve the read and write performance
    // of SQLite. See the SQLite [docs](https://www.sqlite.org/pragma.html) for more
    // information.
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "OFF")?;
    Ok(connection)
}

#[cfg(test)]
mod test {
    use super::*;

    /// Veriifies the weighted average calculation.
    #[test]
    fn test_weighted_average() {
        // Valid rewards and weights.
        let rewards = vec![1.0, 2.0, 3.0];
        let weights = vec![0.2, 0.3, 0.5];
        let average = weighted_average(&rewards, &weights);
        assert_eq!(average, 2.3);

        // Empty weights result in a zero average.
        let rewards: Vec<f32> = vec![];
        let weights: Vec<f32> = vec![];
        let average = weighted_average(&rewards, &weights);
        assert_eq!(average, 0.0);

        // All zero weights result in a zero average.
        let rewards = vec![1.0, 2.0, 3.0];
        let weights = vec![0.0, 0.0, 0.0];
        let average = weighted_average(&rewards, &weights);
        assert_eq!(average, 0.0);
    }
}
