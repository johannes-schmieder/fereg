//! The sealed boundary witness captures the existing generator, without new input semantics.
#![deny(clippy::too_many_lines)]
#[path = "support/ir_cases.rs"]
mod ir_cases;
const WITNESS: &str =
    include_str!("../../../tests/reference/fe_boundary_response_decimal_oracle_v1.csv");

#[test]
fn boundary_response_witness_inputs_match_the_original_seed155_case_bit_for_bit() {
    let case = ir_cases::case(155, true);
    let response: Vec<_> = case.outcome.iter().map(|&y| f64::from(y == 0.)).collect();
    let boundary = response.iter().filter(|&&u| u == 1.).count();
    let mut rows = 0;
    for (row, line) in WITNESS.lines().skip(1).enumerate() {
        let fields: Vec<_> = line.split(',').collect();
        assert_eq!(fields.len(), 13);
        assert_eq!(fields[0].parse::<usize>().unwrap(), row);
        for effect in 0..3 {
            assert_eq!(
                fields[2 * effect + 1].parse::<u32>().unwrap(),
                case.effects[effect].codes()[row]
            );
            assert_eq!(
                fields[2 * effect + 2].parse::<f64>().unwrap().to_bits(),
                case.effects[effect].slopes()[0][row].to_bits()
            );
            assert_eq!(
                fields[effect + 9].parse::<f64>().unwrap().to_bits(),
                case.columns[effect][row].to_bits()
            );
        }
        let weight = if response[row] == 1. {
            1e-5_f64.powi(2) / boundary as f64
        } else {
            1.
        };
        assert_eq!(
            fields[7].parse::<f64>().unwrap().to_bits(),
            weight.to_bits()
        );
        assert_eq!(
            fields[8].parse::<f64>().unwrap().to_bits(),
            response[row].to_bits()
        );
        assert!(fields[12].parse::<f64>().unwrap().is_finite());
        rows += 1;
    }
    assert_eq!(rows, 79);
}
