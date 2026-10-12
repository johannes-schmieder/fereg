//! Direct Cartesian contingency witness, independent of histogram moment identities.
#[must_use]
pub fn centered_ratio(a: &[u32], b: &[u32]) -> (u128, u128) {
    assert_eq!(a.len(), b.len());
    let la = *a.iter().max().unwrap() as usize + 1;
    let lb = *b.iter().max().unwrap() as usize + 1;
    let mut cells = vec![vec![0_i128; lb]; la];
    for (&i, &j) in a.iter().zip(b) {
        cells[i as usize][j as usize] += 1;
    }
    let rows: Vec<i128> = cells.iter().map(|r| r.iter().sum()).collect();
    let cols: Vec<i128> = (0..lb).map(|j| cells.iter().map(|r| r[j]).sum()).collect();
    let n = a.len() as i128;
    let mut numerator = 0_u128;
    for (i, &row_count) in rows.iter().enumerate() {
        for (j, &col_count) in cols.iter().enumerate() {
            numerator += (n * cells[i][j] - row_count * col_count)
                .unsigned_abs()
                .pow(2);
        }
    }
    let denominator = n as u128
        * n as u128
        * *rows.iter().min().unwrap() as u128
        * *cols.iter().min().unwrap() as u128;
    (numerator, denominator)
}
